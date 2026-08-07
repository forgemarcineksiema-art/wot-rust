use renderer_api::{Frustum, RenderError};

use crate::scene_target::SceneRenderTarget;
use crate::{CameraUniform, GpuContext, encode_camera_uniform};

/// How far ahead of the chase camera (along its horizontal look) to centre the auto sun-shadow box,
/// so the ~128 m footprint straddles the near/mid combat field the player is looking at rather than
/// the ground behind the camera. Paired with `SunShadowParams::default().focus_radius_m`.
const SHADOW_FORWARD_OFFSET_M: f32 = 40.0;

/// The far cascade's forward offset: its ~576 m footprint is pushed out so the coverage spans the
/// mid/far field ahead of the camera (roughly 200±288 m along the look) — the ground the single
/// near box left flat. Paired with `SunShadowParams::far_cascade`.
const FAR_SHADOW_FORWARD_OFFSET_M: f32 = 200.0;

impl super::SceneRenderer {
    /// Draw a frame: make sure this frame's resources exist, then encode it.
    ///
    /// The two halves are separate methods on purpose. Everything that can CREATE a GPU resource
    /// lives in `prepare_frame`, which takes `&mut self`; encoding takes `&self` and can only
    /// read. Before the split, `render` took `&self` and the lazily-created targets lived behind
    /// `RefCell`s — so every access during encoding was a `borrow()` that could panic at runtime,
    /// and nothing but care stopped a resource being created in the middle of a pass.
    pub fn render(
        &mut self,
        ctx: &GpuContext,
        target: SceneRenderTarget<'_>,
        view_proj: [[f32; 4]; 4],
        camera_pos: [f32; 3],
    ) -> Result<(), RenderError> {
        self.prepare_frame(ctx, target.width, target.height);
        self.encode_frame(ctx, target, view_proj, camera_pos)
    }

    /// Everything this frame needs to exist before a single pass opens: the HDR chain sized to the
    /// target, the SSAO chain, the bloom ladder, and the bind groups that point at them.
    ///
    /// The one place in a frame allowed to create a GPU resource, which is what
    /// `no_gpu_resource_is_created_during_encode` keeps true.
    fn prepare_frame(&mut self, ctx: &GpuContext, width: u32, height: u32) {
        let target = PrepareExtent { width, height };
        if self.ssao.ensure_targets(&ctx.device, target.width, target.height) {
            let blur_view =
                &self.ssao.targets.as_ref().expect("ssao targets just created").blur_view;
            self.shadow.rebind_ao(&ctx.device, &self.shadow_bgl, blur_view);
        }
        let hdr_recreated = self.post.ensure_targets(
            &ctx.device,
            target.width,
            target.height,
            self.sample_count,
            self.refraction,
        );
        let resolve_view =
            self.post.targets.as_ref().expect("post targets just ensured").resolve_view.clone();
        self.bloom.ensure_targets(&ctx.device, target.width, target.height, &resolve_view);
        if hdr_recreated || self.fxaa.bind_group.is_none() {
            let ldr_view =
                self.post.targets.as_ref().expect("post targets just ensured").ldr_view.clone();
            self.fxaa.rebuild_bind_group(&ctx.device, &ldr_view);
        }
        if hdr_recreated || self.post.bind_group.is_none() {
            // The post pass reads the HDR frame and the bloom mip; a black 1x1 stands in when
            // the ladder is off (bloom_mips 0 / weight 0).
            let bloom_view = self.bloom.output_view().unwrap_or_else(|| {
                let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("bloom_black_fallback"),
                    size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                texture.create_view(&wgpu::TextureViewDescriptor::default())
            });
            self.post.rebuild_bind_group(&ctx.device, &bloom_view);
        }

        // Refraction takes the two-pass grab path only when the tier enables it AND the frame is
        // multisampled (the grab is produced by the MSAA resolve). Rebind the water's group-1 grab
        // whenever the HDR chain was recreated.
        if self.frame_switches().refraction {
            let grab =
                self.post.targets.as_ref().expect("post targets just ensured").grab_view.clone();
            if let Some(grab) = grab
                && (hdr_recreated || self.water_refraction.bind_group.is_none())
            {
                self.water_refraction.rebuild_bind_group(&ctx.device, &grab);
            }
        }
    }

    fn encode_frame(
        &self,
        ctx: &GpuContext,
        target: SceneRenderTarget<'_>,
        view_proj: [[f32; 4]; 4],
        camera_pos: [f32; 3],
    ) -> Result<(), RenderError> {
        // The graph decides what this frame encodes. The switches are read off the renderer once
        // and then ASKED, so a pass cannot be gated by one rule here and described by another
        // there.
        let switches = self.frame_switches();
        let refraction_active = switches.refraction;
        // Focus the sun-shadow box on the subject. Studio shots set an explicit focus (the tank); the
        // battlefield leaves it unset, so the box is pushed forward along the view so its coverage
        // lands on the field the chase camera looks at, not the empty ground behind it. Then build
        // the texel-snapped light matrix and pack it into the shared uniform.
        let (near_cascade, far_cascade) = self.shadow.cascades(self.shadow_focus_radius_m);
        let focus = self.shadow_focus.unwrap_or_else(|| {
            renderer_api::forward_shadow_focus(camera_pos, view_proj, SHADOW_FORWARD_OFFSET_M)
        });
        let light_view_proj = renderer_api::sun_light_view_projection(
            self.scene_lighting.key_direction,
            focus,
            near_cascade,
        );
        // The far cascade rides the same sun, centred further out along the look. A studio shot's
        // explicit focus centres both boxes on the subject — the far box just covers more floor.
        let far_focus = self.shadow_focus.unwrap_or_else(|| {
            renderer_api::forward_shadow_focus(camera_pos, view_proj, FAR_SHADOW_FORWARD_OFFSET_M)
        });
        let light_view_proj_far = renderer_api::sun_light_view_projection(
            self.scene_lighting.key_direction,
            far_focus,
            far_cascade,
        );
        // The projection's Y scale (P[1][1]) survives in the view-projection's second row, whose
        // rotation part is unit length — recovered so SSAO can convert world radii to pixels,
        // and (Jedna Trawa P4b) so the grass bands know this frame's magnification.
        let proj_y_scale = renderer_api::projection_y_scale(&view_proj);
        let band_scale = renderer_api::grass_zoom_band_scale(proj_y_scale);
        let camera = CameraUniform::from_scene(
            view_proj,
            renderer_api::view_projection_inverse(view_proj),
            camera_pos,
            &self.scene_lighting,
            crate::FramePassParams {
                light_view_proj,
                light_view_proj_far,
                shadow_params: self.shadow.shader_params(near_cascade),
                cascade_params: self.shadow.cascade_shader_params(far_cascade),
                ssao_params: [self.ssao.near, self.ssao.far, self.ssao.strength, proj_y_scale],
                inv_render_size: [
                    1.0 / target.width.max(1) as f32,
                    1.0 / target.height.max(1) as f32,
                ],
                cloud_shadows_enabled: self.cloud_shadows_enabled,
                shader_detail: self.shader_detail,
                crushers: self.grass_crusher_slots(camera_pos),
                bloom_enabled: self.bloom.mips > 0,
                time_s: self.scene_time_s,
                rain_intensity: self.rain_intensity,
                wetness: self.wetness,
                weather_params: [
                    self.cloud_offset[0],
                    self.cloud_offset[1],
                    self.puddle_fill,
                    self.rain_phase_s,
                ],
            },
        );
        ctx.queue.write_buffer(&self.camera_buffer, 0, &encode_camera_uniform(&camera)?);

        // Per-pass visibility: each pass draws only the terrain chunks its own frustum sees —
        // the camera passes cull by the view frustum, the shadow passes by their light boxes.
        let camera_frustum = Frustum::from_view_proj(&view_proj);
        let light_frustum = Frustum::from_view_proj(&light_view_proj);
        let light_frustum_far = Frustum::from_view_proj(&light_view_proj_far);

        let mut encoder =
            ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        // Every pass below is opened through the recorder, which owns the label and the timestamp
        // writes. While the profiler is Disabled — the shipped path — it emits the same descriptor
        // this code emitted before it existed.
        let mut recorder = crate::pass_recorder::PassRecorder::new(&self.profiler);
        self.encode_shadow_pass(&mut recorder, &mut encoder, &light_frustum);
        self.encode_far_shadow_pass(&mut recorder, &mut encoder, &light_frustum_far);
        if switches.encodes(crate::frame_graph::PassId::SsaoPrepass) {
            self.encode_ssao_prepass(&mut recorder, &mut encoder, &camera_frustum);
            self.ssao.encode_ao_passes(&mut recorder, &mut encoder, &self.camera_bind_group);
        }
        // The world renders linear HDR into the internal Rgba16Float chain; the caller's target
        // receives only the post pass's display-transformed picture (plus the HUD).
        let hdr = self.post.targets.as_ref().expect("post targets just ensured");
        if refraction_active {
            // Refraction path (two passes). Pass 1 — the lit opaque world, RESOLVED into the grab
            // texture so the water can read the scene behind it; the MSAA colour and depth are kept
            // (Store) for pass 2 to load. Pass 2 — the water sampling that grab, then FX and rain,
            // over the kept colour, resolving into the final HDR the post pass reads.
            let grab_view = hdr.grab_view.as_ref().expect("grab present when refraction active");
            {
                let mut pass = recorder.begin(
                    &mut encoder,
                    crate::frame_graph::PassId::SceneOpaque,
                    &[Some(wgpu::RenderPassColorAttachment {
                        view: &hdr.color_view,
                        resolve_target: Some(grab_view),
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(self.sky),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &hdr.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                );
                self.draw_world_opaque(&mut pass, &camera_frustum, camera_pos, band_scale);
            }
            {
                let grab_bg = self.water_refraction.bind_group.as_ref();
                let mut pass = recorder.begin(
                    &mut encoder,
                    crate::frame_graph::PassId::SceneWater,
                    &[Some(wgpu::RenderPassColorAttachment {
                        view: &hdr.color_view,
                        resolve_target: Some(&hdr.resolve_view),
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Discard,
                        },
                    })],
                    Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &hdr.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Discard,
                        }),
                        stencil_ops: None,
                    }),
                );
                self.draw_water_surface(&mut pass, grab_bg);
                self.draw_overlay_fx(&mut pass);
            }
        } else {
            // Single-pass path (analytic water) — the integrated/software route, byte-for-byte the
            // pre-refraction frame: opaque world, analytic water inline, then FX and rain.
            let mut pass = recorder.begin(
                &mut encoder,
                crate::frame_graph::PassId::Scene,
                &[Some(wgpu::RenderPassColorAttachment {
                    view: &hdr.color_view,
                    resolve_target: hdr.multisampled.then_some(&hdr.resolve_view),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.sky),
                        store: if hdr.multisampled {
                            wgpu::StoreOp::Discard
                        } else {
                            wgpu::StoreOp::Store
                        },
                    },
                })],
                Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &hdr.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
            );
            self.draw_world_opaque(&mut pass, &camera_frustum, camera_pos, band_scale);
            self.draw_water_surface(&mut pass, None);
            self.draw_overlay_fx(&mut pass);
        }
        // The bloom ladder blurs the resolved HDR frame down and back up before the post pass
        // composites it (rule 6); skipped entirely at weight 0 or bloom_mips 0.
        if switches.encodes(crate::frame_graph::PassId::Bloom) {
            self.bloom.encode(&mut recorder, &mut encoder);
        }
        // The post pass: one fullscreen triangle applies the display transform (exposure ->
        // ACES -> grade -> dither) to the resolved HDR frame and writes the ENCODED picture
        // into the LDR intermediate — the single place the picture is formed.
        {
            let mut pass = recorder.begin(
                &mut encoder,
                crate::frame_graph::PassId::Post,
                &[Some(wgpu::RenderPassColorAttachment {
                    view: &hdr.ldr_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                None,
            );
            pass.set_pipeline(&self.post.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(
                1,
                self.post.bind_group.as_ref().expect("post bind group just ensured"),
                &[],
            );
            pass.draw(0..3, 0..1);
        }
        // The FXAA pass reads that formed picture and writes the caller's target — the one
        // anti-aliasing every player gets (canonical ships 1x MSAA, so this is the shipped
        // game's only AA). The HUD draws after it, un-graded and never softened: the UI reads
        // the battle, it is not part of the painting.
        {
            let output_view = target.output_view;
            let mut pass = recorder.begin(
                &mut encoder,
                crate::frame_graph::PassId::Fxaa,
                &[Some(wgpu::RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                None,
            );
            pass.set_pipeline(&self.fxaa.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(
                1,
                self.fxaa.bind_group.as_ref().expect("fxaa bind group just ensured"),
                &[],
            );
            pass.draw(0..3, 0..1);
            if self.hud_vertex_count > 0 {
                pass.set_pipeline(&self.hud_pipeline);
                pass.set_bind_group(0, &self.hud_font_bind_group, &[]);
                pass.set_vertex_buffer(0, self.hud_vertices.slice(..));
                pass.draw(0..self.hud_vertex_count, 0..1);
            }
        }
        // Copy this frame's timestamps out on the SAME encoder, before submit: a resolve on a
        // later encoder would read a query set the GPU may not have finished writing. A no-op
        // while the profiler is Disabled.
        recorder.resolve(&mut encoder);
        // The counts, unlike the timings, are always there to take.
        self.frame_counts.set(recorder.counts());
        self.frame_pass_order.set(recorder.order());
        ctx.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    /// The opaque world into the current pass: gradient sky, the scene pipeline (terrain, dynamic
    /// props, static frame meshes), the battlefield ground, then vehicles. Shared by the single
    /// pass (analytic water) and the refraction opaque pass.
    fn draw_world_opaque(
        &self,
        pass: &mut crate::pass_recorder::CountedPass<'_, '_>,
        camera_frustum: &Frustum,
        eye: [f32; 3],
        band_scale: f32,
    ) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_bind_group(1, &self.foliage_atlas.bind_group, &[]);
        pass.set_bind_group(2, &self.shadow.bind_group, &[]);
        pass.set_vertex_buffer(1, self.identity_instance.slice(..));
        if self.terrain_index_count > 0 {
            pass.set_vertex_buffer(0, self.terrain_vertices.slice(..));
            pass.set_index_buffer(self.terrain_indices.slice(..), wgpu::IndexFormat::Uint32);
            self.draw_visible_terrain(pass, camera_frustum);
        }
        // The dressing slot (Żywy Step P2): color pass ONLY — never the cascades, never the
        // SSAO prepass — and distance-cut past its own collapse band, which the scope
        // stretches (Jedna Trawa P4b) exactly as far as the shader stands far tufts.
        self.draw_visible_dressing(pass, camera_frustum, eye, band_scale);
        if self.dynamic_index_count > 0 {
            pass.set_vertex_buffer(0, self.dynamic_vertices.slice(..));
            pass.set_index_buffer(self.dynamic_indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.dynamic_index_count, 0, 0..1);
        }
        if self.frame_instance_count > 0 {
            pass.set_vertex_buffer(1, self.frame_instances.slice(..));
            for draw in &self.frame_draws {
                let Some(mesh) = self.static_meshes.get(draw.mesh) else {
                    self.skipped_mesh_draws.set(self.skipped_mesh_draws.get().saturating_add(1));
                    debug_assert!(
                        false,
                        "render frame references unregistered mesh handle {}",
                        draw.mesh.0
                    );
                    continue;
                };
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(
                    0..mesh.index_count,
                    0,
                    draw.instance_start..draw.instance_start + draw.instance_count,
                );
            }
        }
        // The battlefield ground: its own pipeline (splat layers + macro normals at group 1), the
        // same camera/shadow groups, chunk-culled by the camera frustum. Drawn AFTER every
        // scene-pipeline draw on purpose: the scene pipeline's layout carries a hole (None) at
        // group 1, and leaving a foreign bind group parked in a hole corrupts the group-2 shadow
        // sampling on some backends — the ground must never leave its material group behind for a
        // holed layout to trip over.
        if let Some(binding) = self.ground.binding.as_ref() {
            pass.set_pipeline(&self.ground.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(1, &binding.bind_group, &[]);
            pass.set_bind_group(2, &self.shadow.bind_group, &[]);
            pass.set_vertex_buffer(1, self.identity_instance.slice(..));
            self.draw_visible_ground(pass, camera_frustum);
        }
        if self.vehicle_instance_count > 0 {
            pass.set_pipeline(&self.vehicle_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow.bind_group, &[]);
            pass.set_vertex_buffer(1, self.vehicle_instances.slice(..));
            for draw in &self.vehicle_draws {
                pass.set_bind_group(1, self.vehicle_materials.bind_group(draw.material), &[]);
                let Some(mesh) = self.vehicle_meshes.get(draw.mesh) else {
                    self.skipped_mesh_draws.set(self.skipped_mesh_draws.get().saturating_add(1));
                    debug_assert!(
                        false,
                        "vehicle render frame references unregistered mesh handle {}",
                        draw.mesh.0
                    );
                    continue;
                };
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(
                    0..mesh.index_count,
                    0,
                    draw.instance_start..draw.instance_start + draw.instance_count,
                );
            }
        }
        // Gradient sky LAST, at the far plane behind a LessEqual depth test (F2): before this
        // it was drawn FIRST with compare Always — the heaviest per-pixel shader in the frame
        // (the FBM cloud sheet) paid for every pixel, including the ~half the terrain then
        // overwrote. Drawn last, early-Z kills every covered pixel and the sky pays only for
        // the visible dome.
        if self.draw_sky {
            pass.set_pipeline(&self.sky_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }

    /// The river surface into the current pass. `grab` supplied (refraction path) selects the
    /// refraction pipeline sampling the opaque grab at group 1; `None` uses the analytic pipeline.
    /// Depth-tested (banks and hulls above the waterline occlude it) but never depth-writing, so a
    /// wading hull's submerged running gear reads through and the FX splashes composite over it.
    fn draw_water_surface(
        &self,
        pass: &mut crate::pass_recorder::CountedPass<'_, '_>,
        grab: Option<&wgpu::BindGroup>,
    ) {
        if self.water_index_count > 0
            && let Some((water_vertices, water_indices)) = self.water_buffers.as_ref()
        {
            match grab {
                Some(grab_bg) => {
                    pass.set_pipeline(&self.water_refraction.pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    pass.set_bind_group(1, grab_bg, &[]);
                }
                None => {
                    pass.set_pipeline(&self.water_pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                }
            }
            pass.set_vertex_buffer(0, water_vertices.slice(..));
            pass.set_index_buffer(water_indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.water_index_count, 0, 0..1);
        }
    }

    /// Battle FX then rain into the current pass, over the water and opaque world before the HUD:
    /// FX reuse the scene camera group for view_proj; rain is stateless vertex-shader streaks.
    fn draw_overlay_fx(&self, pass: &mut crate::pass_recorder::CountedPass<'_, '_>) {
        if self.fx_vertex_count > 0 {
            pass.set_pipeline(&self.fx_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.fx_vertices.slice(..));
            pass.draw(0..self.fx_vertex_count, 0..1);
        }
        let rain_streaks =
            (crate::rain_pipeline::RAIN_MAX_STREAKS as f32 * self.rain_intensity) as u32;
        if rain_streaks > 0 {
            pass.set_pipeline(&self.rain_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.draw(0..6, 0..rain_streaks);
        }
    }
}

/// The two fields `prepare_frame` needs from the caller's target, so the moved body can keep
/// reading `target.width` and `target.height` and the diff stays a move rather than a rewrite.
struct PrepareExtent {
    width: u32,
    height: u32,
}
