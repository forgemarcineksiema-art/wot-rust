//! GPU resources for the focused sun shadow map (`docs/shadow-policy.md`): the depth target, the
//! comparison sampler + bind group both main pipelines sample at group 2, and the depth-only
//! occluder pipeline. The light matrix itself is backend-neutral (`renderer_api::sun_shadow`).

use renderer_api::SunShadowParams;

use crate::scene_resources::SceneInstance;

const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub fn shadow_shader_source() -> &'static str {
    include_str!("../shaders/shadow.wgsl")
}

const SHADOW_VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 1] =
    wgpu::vertex_attr_array![0 => Float32x3];
const SHADOW_INSTANCE_ATTRIBUTES: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x4];

/// The focused sun shadow map: depth target, the group-2 environment bind group (shadow map +
/// SSAO target), the depth-only occluder pipeline, and the tuning that drives the light matrix
/// and PCF in the shaders. The bind group is rebuilt whenever the SSAO target resizes.
pub(crate) struct ShadowResources {
    pub depth_view: wgpu::TextureView,
    pub bind_group: std::cell::RefCell<wgpu::BindGroup>,
    pub pipeline: wgpu::RenderPipeline,
    pub params: SunShadowParams,
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
    ) -> Self {
        let params = SunShadowParams::default();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sun_shadow_map"),
            size: wgpu::Extent3d {
                width: params.resolution,
                height: params.resolution,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
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
            &shadow_sampler,
            initial_ao_view,
            &ao_sampler,
        );
        let pipeline = build_shadow_pipeline(device, camera_bgl);
        // A small constant depth bias plus a normal offset scaled to the texel footprint kills acne
        // without peter-panning; strength 1 = full shadow (0 is the no-shadow capability fallback).
        Self {
            depth_view,
            bind_group: std::cell::RefCell::new(bind_group),
            pipeline,
            params,
            depth_bias: 0.0015,
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
}

/// Depth-only occluder pipeline: transforms position by `camera.light_view_proj * model` and writes
/// depth. Single-sampled (the shadow map is 1x), camera uniform at group 0.
fn build_shadow_pipeline(
    device: &wgpu::Device,
    camera_bgl: &wgpu::BindGroupLayout,
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
        label: Some("shadow_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<renderer_api::VehicleVertex>() as u64,
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
            // Cull front faces in the shadow pass: shadow-casting from back faces reduces acne on lit
            // surfaces (a common peter-panning/acne trade for solid closed occluders like hulls).
            cull_mode: Some(wgpu::Face::Front),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: SHADOW_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: None,
        multiview_mask: None,
        cache: None,
    })
}
