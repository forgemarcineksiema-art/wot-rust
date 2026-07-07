use renderer_api::RenderError;

use crate::scene_target::{SceneRenderTarget, store_op_for_target};
use crate::{CameraUniform, GpuContext, encode_camera_uniform};

impl super::SceneRenderer {
    pub fn render(
        &self,
        ctx: &GpuContext,
        target: SceneRenderTarget<'_>,
        view_proj: [[f32; 4]; 4],
        camera_pos: [f32; 3],
    ) -> Result<(), RenderError> {
        if target.sample_count != self.sample_count {
            return Err(RenderError::new(format!(
                "scene renderer sample count {} does not match render target sample count {}",
                self.sample_count, target.sample_count
            )));
        }
        // Focus the sun-shadow box on the subject (falls back to the camera, which still covers the
        // near action), build its texel-snapped light matrix, and pack it into the shared uniform.
        let focus = self.shadow_focus.unwrap_or(camera_pos);
        let light_view_proj = renderer_api::sun_light_view_projection(
            self.scene_lighting.key_direction,
            focus,
            self.shadow.params,
        );
        // The projection's Y scale (P[1][1]) survives in the view-projection's second row, whose
        // rotation part is unit length — recovered here so SSAO can convert world radii to pixels.
        let proj_y_scale = (view_proj[0][1] * view_proj[0][1]
            + view_proj[1][1] * view_proj[1][1]
            + view_proj[2][1] * view_proj[2][1])
            .sqrt();
        let camera = CameraUniform::from_scene(
            view_proj,
            renderer_api::view_projection_inverse(view_proj),
            camera_pos,
            &self.scene_lighting,
            crate::FramePassParams {
                light_view_proj,
                shadow_params: self.shadow.shader_params(),
                ssao_params: [self.ssao.near, self.ssao.far, self.ssao.strength, proj_y_scale],
                time_s: self.scene_time_s,
            },
        );
        ctx.queue.write_buffer(&self.camera_buffer, 0, &encode_camera_uniform(&camera)?);

        if self.ssao.ensure_targets(&ctx.device, target.width, target.height) {
            let targets = self.ssao.targets.borrow();
            let blur_view = &targets.as_ref().expect("ssao targets just created").blur_view;
            self.shadow.rebind_ao(&ctx.device, &self.shadow_bgl, blur_view);
        }

        let mut encoder =
            ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        self.encode_shadow_pass(&mut encoder);
        if self.ssao.strength > 0.0 {
            self.encode_ssao_prepass(&mut encoder);
            self.ssao.encode_ao_passes(&mut encoder, &self.camera_bind_group);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.color_view,
                    resolve_target: target.resolve_target,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.sky),
                        store: store_op_for_target(target.resolve_target),
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            // Gradient-sky background first (depth compare Always, no write), so the geometry drawn
            // next overwrites it wherever it wins the depth test and the sky shows through elsewhere.
            if self.draw_sky {
                pass.set_pipeline(&self.sky_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &*self.shadow.bind_group.borrow(), &[]);
            pass.set_vertex_buffer(1, self.identity_instance.slice(..));
            if self.terrain_index_count > 0 {
                pass.set_vertex_buffer(0, self.terrain_vertices.slice(..));
                pass.set_index_buffer(self.terrain_indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.terrain_index_count, 0, 0..1);
            }
            if self.dynamic_index_count > 0 {
                pass.set_vertex_buffer(0, self.dynamic_vertices.slice(..));
                pass.set_index_buffer(self.dynamic_indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.dynamic_index_count, 0, 0..1);
            }
            if self.frame_instance_count > 0 {
                pass.set_vertex_buffer(1, self.frame_instances.slice(..));
                for draw in &self.frame_draws {
                    let Some(mesh) = self.static_meshes.get(draw.mesh) else {
                        self.skipped_mesh_draws
                            .set(self.skipped_mesh_draws.get().saturating_add(1));
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
            if self.vehicle_instance_count > 0 {
                pass.set_pipeline(&self.vehicle_pipeline);
                pass.set_bind_group(0, &self.vehicle_camera_bind_group, &[]);
                pass.set_bind_group(2, &*self.shadow.bind_group.borrow(), &[]);
                pass.set_vertex_buffer(1, self.vehicle_instances.slice(..));
                for draw in &self.vehicle_draws {
                    pass.set_bind_group(1, self.vehicle_materials.bind_group(draw.material), &[]);
                    let Some(mesh) = self.vehicle_meshes.get(draw.mesh) else {
                        self.skipped_mesh_draws
                            .set(self.skipped_mesh_draws.get().saturating_add(1));
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
            // The river surface after every lit opaque: depth-tested (banks and hulls above
            // the waterline occlude it) but drawn over whatever sits below the plane — a
            // wading hull's submerged running gear correctly reads through the water. Never
            // writes depth, so the FX splashes drawn next still composite over the surface.
            if self.water_index_count > 0
                && let Some((water_vertices, water_indices)) = self.water_buffers.as_ref()
            {
                pass.set_pipeline(&self.water_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_vertex_buffer(0, water_vertices.slice(..));
                pass.set_index_buffer(water_indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.water_index_count, 0, 0..1);
            }
            // Battle FX draw after every lit opaque (correct depth occlusion) and before the
            // HUD overlay; the pipeline reuses the scene camera bind group for view_proj.
            if self.fx_vertex_count > 0 {
                pass.set_pipeline(&self.fx_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_vertex_buffer(0, self.fx_vertices.slice(..));
                pass.draw(0..self.fx_vertex_count, 0..1);
            }
            if self.hud_vertex_count > 0 {
                pass.set_pipeline(&self.hud_pipeline);
                pass.set_bind_group(0, &self.hud_font_bind_group, &[]);
                pass.set_vertex_buffer(0, self.hud_vertices.slice(..));
                pass.draw(0..self.hud_vertex_count, 0..1);
            }
        }
        ctx.queue.submit(Some(encoder.finish()));
        Ok(())
    }
}
