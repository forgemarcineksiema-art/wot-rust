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
        include_str!("../shaders/shadow.wgsl"),
    ])
}

const SHADOW_VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 1] =
    wgpu::vertex_attr_array![0 => Float32x3];
const SHADOW_INSTANCE_ATTRIBUTES: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x4];

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
    pub bind_group: std::cell::RefCell<wgpu::BindGroup>,
    pub pipeline_scene: wgpu::RenderPipeline,
    pub pipeline_vehicle: wgpu::RenderPipeline,
    /// The far-cascade occluder pipeline: scene vertex stride through `vs_far`. The fleet has no
    /// far pipeline on purpose â€” at the far map's texel size a tank's shadow does not resolve.
    pub pipeline_scene_far: wgpu::RenderPipeline,
    pub params: SunShadowParams,
    pub far_params: SunShadowParams,
    /// 2 = near + far cascades; 1 = the single near box (`WOT_SHADOW_CASCADES=1`).
    pub cascade_count: u32,
    pub depth_bias: f32,
    pub normal_offset: f32,
    pub strength: f32,
    shadow_sampler: wgpu::Sampler,
    ao_sampler: wgpu::Sampler,
}

impl ShadowResources {
    pub fn new(
        device: &wgpu::Device,
        shadow_bgl: &wgpu::BindGroupLayout,
        camera_bgl: &wgpu::BindGroupLayout,
        initial_ao_view: &wgpu::TextureView,
        resolution: u32,
        cascade_count: u32,
    ) -> Self {
        // The caller resolved the resolution per adapter (`quality::resolve_lighting_quality`);
        // clamp to the device limit last so a capped device gets a smaller map â€” with the
        // texel-derived PCF step and normal offset shrinking with it â€” never a failed texture.
        let params = SunShadowParams {
            resolution: resolution.min(device.limits().max_texture_dimension_2d),
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
        let bind_group = super::env_group::build_environment_bind_group(
            device,
            shadow_bgl,
            &depth_view,
            &far_depth_view,
            &shadow_sampler,
            initial_ao_view,
            &ao_sampler,
        );
        let pipeline_scene = build_shadow_pipeline(
            device,
            camera_bgl,
            std::mem::size_of::<renderer_api::SceneVertex>() as u64,
            "vs_main",
            "shadow_pipeline_scene",
        );
        let pipeline_vehicle = build_shadow_pipeline(
            device,
            camera_bgl,
            std::mem::size_of::<renderer_api::VehicleVertex>() as u64,
            "vs_main",
            "shadow_pipeline_vehicle",
        );
        let pipeline_scene_far = build_shadow_pipeline(
            device,
            camera_bgl,
            std::mem::size_of::<renderer_api::SceneVertex>() as u64,
            "vs_far",
            "shadow_pipeline_scene_far",
        );
        // A small constant depth bias plus a normal offset scaled to the texel footprint kills acne
        // without peter-panning; strength 1 = full shadow (0 is the no-shadow capability fallback).
        // The bias is NDC over the 2*depth_radius span â€” 0.0008 * 160 m = ~13 cm of world slack,
        // tight enough that wheel-scale detail keeps its contact shadow.
        Self {
            depth_view,
            far_depth_view,
            bind_group: std::cell::RefCell::new(bind_group),
            pipeline_scene,
            pipeline_vehicle,
            pipeline_scene_far,
            params,
            far_params,
            cascade_count: cascade_count.clamp(1, 2),
            depth_bias: 0.0008,
            normal_offset: params.texel_world_size() * 1.5,
            strength: 1.0,
            shadow_sampler,
            ao_sampler,
        }
    }

    /// Re-point the group-2 environment bind group at a (re)created SSAO target.
    pub fn rebind_ao(
        &self,
        device: &wgpu::Device,
        shadow_bgl: &wgpu::BindGroupLayout,
        ao_view: &wgpu::TextureView,
    ) {
        *self.bind_group.borrow_mut() = super::env_group::build_environment_bind_group(
            device,
            shadow_bgl,
            &self.depth_view,
            &self.far_depth_view,
            &self.shadow_sampler,
            ao_view,
            &self.ao_sampler,
        );
    }

    /// The packed `shadow_params` the shaders read: texel UV step, depth bias, strength, normal
    /// offset.
    pub fn shader_params(&self) -> [f32; 4] {
        [self.params.texel_uv_size(), self.depth_bias, self.strength, self.normal_offset]
    }

    /// The packed `cascade_params` the shaders read: far texel UV step, far normal offset,
    /// cascade count, containment margin. A single-cascade setup packs margin 0, so the near
    /// box's valid region is exactly the pre-cascade `[0, 1]` UV â€” byte-for-byte the old lookup.
    pub fn cascade_shader_params(&self) -> [f32; 4] {
        [
            self.far_params.texel_uv_size(),
            self.far_params.texel_world_size() * 1.5,
            self.cascade_count as f32,
            if self.cascade_count >= 2 { CASCADE_MARGIN_UV } else { 0.0 },
        ]
    }
}

/// Depth-only occluder pipeline: transforms position by the selected cascade's light matrix
/// (`entry_point` picks `vs_main` for the near box, `vs_far` for the far cascade) and writes
/// depth. `vertex_stride` selects the caster format (scene vs vehicle); both lead with `position`,
/// so the one vertex shader serves both. Single-sampled (the shadow map is 1x), camera uniform at
/// group 0.
fn build_shadow_pipeline(
    device: &wgpu::Device,
    camera_bgl: &wgpu::BindGroupLayout,
    vertex_stride: u64,
    entry_point: &str,
    label: &str,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shadow_shader"),
        source: wgpu::ShaderSource::Wgsl(shadow_shader_source().into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("shadow_pipeline_layout"),
        bind_group_layouts: &[Some(camera_bgl)],
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
                    attributes: &SHADOW_VERTEX_ATTRIBUTES,
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
            // same buffer), whose sun-facing surface IS its front face â€” front-culling it would drop
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
            // varies fastest across a texel) get the most push, flat decks almost none â€” the
            // classic peter-pan-free acne fix for an open receiver that also casts.
            bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.5, clamp: 0.0 },
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: None,
        multiview_mask: None,
        cache: None,
    })
}
