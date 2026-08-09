//! GPU resources for the focused sun shadow map (`docs/shadow-policy.md`): the depth target, the
//! comparison sampler + bind group both main pipelines sample at group 2, and the depth-only
//! occluder pipeline. The light matrix itself is backend-neutral (`renderer_api::sun_shadow`).

use renderer_api::SunShadowParams;

use crate::scene_resources::SceneInstance;

const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// The near box's containment margin in shadow-map UV: fragments inside it take the crisp near
/// cascade, outside it fall through to the far cascade. Keeps the handoff off the very edge of
/// the near map, where the 3Ă—3 PCF would read clamped texels.
const CASCADE_MARGIN_UV: f32 = 0.02;

pub fn shadow_shader_source() -> String {
    crate::shader_library::compose_shader(&[
        crate::shader_library::CAMERA_COMMON_WGSL,
        // The wind field, so a swaying canopy's SHADOW sways with it: the cutout caster
        // displaces its vertices exactly as the colour pass does. Without it the leaves would
        // move over a shadow nailed to the ground — the one place the mismatch is unmissable.
        crate::shader_library::NOISE_COMMON_WGSL,
        crate::shader_library::MEADOW_COMMON_WGSL,
        include_str!("../shaders/shadow.wgsl"),
    ])
}

const SHADOW_VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 1] =
    wgpu::vertex_attr_array![0 => Float32x3];
/// The SCENE caster reads position AND the UV lane (Flora 2.0, FL-2), so the depth fragment
/// can cut a leaf's shadow to its mask. Explicit offsets: uv sits at byte 52 of SceneVertex
/// (pinned by `renderer_api/tests/scene_vertex_lanes.rs`) — `vertex_attr_array!` would pack
/// it right after position and read garbage.
const SHADOW_SCENE_VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 3] = [
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
    // The wind lane (byte 48), so a caster bends with the same gust its lit copy does.
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32, offset: 48, shader_location: 11 },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 52,
        shader_location: 12,
    },
];
const SHADOW_INSTANCE_ATTRIBUTES: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x4,
        10 => Float32x4, 13 => Uint32];

/// The legal range for a scene's near-box half-size, shared by the `WOT_SHADOW_FOCUS` dev knob
/// and the per-scene override. Below 4 m the box stops containing a vehicle; past 256 m the
/// texel is coarser than the far cascade's and the near map buys nothing.
pub(crate) const FOCUS_RADIUS_RANGE_M: std::ops::RangeInclusive<f32> = 4.0..=256.0;

/// The focused sun shadow map: depth target, the group-2 environment bind group (shadow map +
/// SSAO target), the depth-only occluder pipelines, and the tuning that drives the light matrix
/// and PCF in the shaders. The bind group is rebuilt whenever the SSAO target resizes.
///
/// Two occluder pipelines share one `shadow.wgsl` `vs_main` (both formats lead with `position`),
/// differing only in vertex stride: `pipeline_scene` for the static world buffer (terrain +
/// buildings + trees) and the dynamic mesh, `pipeline_vehicle` for the running fleet. The whole
/// world casts, not just vehicles, so buildings ground on the field and hillsides self-shadow
/// under a raking sun.
pub(crate) struct ShadowResources {
    pub depth_view: wgpu::TextureView,
    pub far_depth_view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
    pub pipeline_scene: wgpu::RenderPipeline,
    pub pipeline_vehicle: wgpu::RenderPipeline,
    /// The far-cascade occluder pipeline: scene vertex stride through `vs_far`. The fleet has no
    /// far pipeline on purpose — at the far map's texel size a tank's shadow does not resolve.
    pub pipeline_scene_far: wgpu::RenderPipeline,
    /// The DEFAULT cascades: the battlefield's 64 m half-box (or the `WOT_SHADOW_FOCUS` dev
    /// override). A scene may narrow the near box per frame — see [`Self::cascades`].
    pub params: SunShadowParams,
    pub far_params: SunShadowParams,
    /// 2 = near + far cascades; 1 = the single near box (`WOT_SHADOW_CASCADES=1`).
    pub cascade_count: u32,
    pub depth_bias: f32,
    pub strength: f32,
    shadow_sampler: wgpu::Sampler,
    ao_sampler: wgpu::Sampler,
    /// The baked cloud-coverage tile + repeat sampler (group-2 bindings 5–6, `cloud_map.rs`).
    cloud_view: wgpu::TextureView,
    cloud_sampler: wgpu::Sampler,
}

impl ShadowResources {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shadow_bgl: &wgpu::BindGroupLayout,
        camera_bgl: &wgpu::BindGroupLayout,
        foliage_bgl: &wgpu::BindGroupLayout,
        initial_ao_view: &wgpu::TextureView,
        resolution: u32,
        cascade_count: u32,
    ) -> Self {
        // The caller resolved the resolution per adapter (`quality::resolve_lighting_quality`);
        // clamp to the device limit last so a capped device gets a smaller map — with the
        // texel-derived PCF step and normal offset shrinking with it — never a failed texture.
        // `WOT_SHADOW_FOCUS=<metres>` — the near box's half-size. A DEV knob, and the reason it
        // exists: resolution and box size both change the world size of a shadow texel, so
        // measuring shadow sharpness needs each of them movable ALONE. Eyeballing two renders
        // that differ in both is how a wrong conclusion gets drawn (it already did once).
        let focus_radius_m = std::env::var("WOT_SHADOW_FOCUS")
            .ok()
            .and_then(|value| value.trim().parse::<f32>().ok())
            .filter(|value| FOCUS_RADIUS_RANGE_M.contains(value))
            .unwrap_or(SunShadowParams::default().focus_radius_m);
        let params = SunShadowParams {
            resolution: resolution.min(device.limits().max_texture_dimension_2d),
            focus_radius_m,
            ..SunShadowParams::default()
        };
        let far_params = params.far_cascade();
        let cascade_map = |label: &str, resolution: u32| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: resolution,
                    height: resolution,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: SHADOW_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            texture.create_view(&wgpu::TextureViewDescriptor::default())
        };
        let depth_view = cascade_map("sun_shadow_map", params.resolution);
        let far_depth_view = cascade_map("sun_shadow_map_far", far_params.resolution);
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let ao_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ssao_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let (cloud_view, cloud_sampler) = super::cloud_map::create_cloud_resources(device, queue);
        let bind_group = super::env_group::build_environment_bind_group(
            device,
            shadow_bgl,
            &depth_view,
            &far_depth_view,
            &shadow_sampler,
            initial_ao_view,
            &ao_sampler,
            &cloud_view,
            &cloud_sampler,
        );
        // Scene casters go through the CUTOUT entries (position + uv, atlas at group 1) so a
        // leaf's shadow is its mask; the fleet keeps the plain depth path — vehicle vertices
        // carry no UV lane and tanks are not made of leaves.
        let pipeline_scene = build_shadow_pipeline(
            device,
            camera_bgl,
            Some(foliage_bgl),
            std::mem::size_of::<renderer_api::SceneVertex>() as u64,
            &SHADOW_SCENE_VERTEX_ATTRIBUTES,
            "vs_main_cutout",
            "fs_depth_cutout",
            "shadow_pipeline_scene",
        );
        let pipeline_vehicle = build_shadow_pipeline(
            device,
            camera_bgl,
            None,
            std::mem::size_of::<renderer_api::VehicleVertex>() as u64,
            &SHADOW_VERTEX_ATTRIBUTES,
            "vs_main",
            "fs_depth",
            "shadow_pipeline_vehicle",
        );
        let pipeline_scene_far = build_shadow_pipeline(
            device,
            camera_bgl,
            Some(foliage_bgl),
            std::mem::size_of::<renderer_api::SceneVertex>() as u64,
            &SHADOW_SCENE_VERTEX_ATTRIBUTES,
            "vs_far_cutout",
            "fs_depth_cutout",
            "shadow_pipeline_scene_far",
        );
        // A small constant depth bias plus a normal offset scaled to the texel footprint kills acne
        // without peter-panning; strength 1 = full shadow (0 is the no-shadow capability fallback).
        // The bias is NDC over the 2*depth_radius span — 0.0008 * 160 m = ~13 cm of world slack,
        // tight enough that wheel-scale detail keeps its contact shadow.
        Self {
            depth_view,
            far_depth_view,
            bind_group,
            pipeline_scene,
            pipeline_vehicle,
            pipeline_scene_far,
            params,
            far_params,
            cascade_count: cascade_count.clamp(1, 2),
            depth_bias: 0.0008,
            strength: 1.0,
            shadow_sampler,
            ao_sampler,
            cloud_view,
            cloud_sampler,
        }
    }

    /// Re-point the group-2 environment bind group at a (re)created SSAO target.
    pub fn rebind_ao(
        &mut self,
        device: &wgpu::Device,
        shadow_bgl: &wgpu::BindGroupLayout,
        ao_view: &wgpu::TextureView,
    ) {
        self.bind_group = super::env_group::build_environment_bind_group(
            device,
            shadow_bgl,
            &self.depth_view,
            &self.far_depth_view,
            &self.shadow_sampler,
            ao_view,
            &self.ao_sampler,
            &self.cloud_view,
            &self.cloud_sampler,
        );
    }

    /// The two cascades for ONE frame, at the scene's near-box half-size (`None` = the default
    /// battlefield box).
    ///
    /// The box size is a property of the SCENE, not of the GPU: a 36 m hangar and a 1000 m
    /// battlefield are handed the same 2048² map, and pointing the battlefield's 128 m box at a
    /// 36 m room spends 92% of its texels on the ground outside the walls. So everything that
    /// depends on the box — both light matrices and both normal offsets — is derived here per
    /// frame rather than frozen at construction. The RESOLUTION stays fixed (one look, one
    /// memory budget); only where the texels are aimed changes.
    pub fn cascades(&self, focus_radius_m: Option<f32>) -> (SunShadowParams, SunShadowParams) {
        let Some(radius) = focus_radius_m else {
            return (self.params, self.far_params);
        };
        let near = SunShadowParams {
            focus_radius_m: radius
                .clamp(*FOCUS_RADIUS_RANGE_M.start(), *FOCUS_RADIUS_RANGE_M.end()),
            ..self.params
        };
        (near, near.far_cascade())
    }

    /// The packed `shadow_params` the shaders read: texel UV step, depth bias, strength, normal
    /// offset. The offset is derived from the FRAME's near box — it is a world distance scaled to
    /// the texel footprint, so a narrowed box must shrink it or a tight scene peter-pans.
    pub fn shader_params(&self, near: SunShadowParams) -> [f32; 4] {
        [near.texel_uv_size(), self.depth_bias, self.strength, near.texel_world_size() * 1.5]
    }

    /// The packed `cascade_params` the shaders read: far texel UV step, far normal offset,
    /// cascade count, containment margin. A single-cascade setup packs margin 0, so the near
    /// box's valid region is exactly the pre-cascade `[0, 1]` UV — byte-for-byte the old lookup.
    pub fn cascade_shader_params(&self, far: SunShadowParams, cascades: u32) -> [f32; 4] {
        [
            far.texel_uv_size(),
            far.texel_world_size() * 1.5,
            cascades as f32,
            if cascades >= 2 { CASCADE_MARGIN_UV } else { 0.0 },
        ]
    }

    /// How many cascades a frame runs, given what the scene asked for.
    ///
    /// A scene may only ever REDUCE the count: `min` with the tier's, so an interior that fits
    /// inside its own near box can drop the far one, and nothing can buy a cascade the adapter
    /// tier did not pay for.
    pub fn frame_cascade_count(&self, requested: Option<u32>) -> u32 {
        requested.unwrap_or(self.cascade_count).clamp(1, self.cascade_count)
    }
}

/// Depth-only occluder pipeline: transforms position by the selected cascade's light matrix
/// (`entry_point` picks `vs_main` for the near box, `vs_far` for the far cascade) and writes
/// depth. `vertex_stride` selects the caster format (scene vs vehicle); both lead with `position`,
/// so the one vertex shader serves both. Single-sampled (the shadow map is 1x), camera uniform at
/// group 0.
#[allow(clippy::too_many_arguments)]
fn build_shadow_pipeline(
    device: &wgpu::Device,
    camera_bgl: &wgpu::BindGroupLayout,
    foliage_bgl: Option<&wgpu::BindGroupLayout>,
    vertex_stride: u64,
    vertex_attributes: &'static [wgpu::VertexAttribute],
    entry_point: &str,
    fragment_entry: &str,
    label: &str,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shadow_shader"),
        source: wgpu::ShaderSource::Wgsl(shadow_shader_source().into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("shadow_pipeline_layout"),
        bind_group_layouts: &[Some(camera_bgl), foliage_bgl],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some(entry_point),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: vertex_stride,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: vertex_attributes,
                },
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SceneInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &SHADOW_INSTANCE_ATTRIBUTES,
                },
            ],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            // No culling: the static world is an open heightmap (buildings/trees are baked into the
            // same buffer), whose sun-facing surface IS its front face — front-culling it would drop
            // exactly the casters we want (hills self-shadowing, roofs onto walls). Acne is held off
            // instead by a slope-scaled hardware depth bias plus the shader's normal offset, which
            // together behave on both the open ground and the closed hulls.
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: SHADOW_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            // Slope-scaled: grazing hillsides (where the sun rakes along the surface and depth
            // varies fastest across a texel) get the most push, flat decks almost none — the
            // classic peter-pan-free acne fix for an open receiver that also casts.
            bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.5, clamp: 0.0 },
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(fragment_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[],
        }),
        multiview_mask: None,
        cache: None,
    })
}
