//! The river-surface pipeline: a static mesh on the still-water plane, drawn inside the scene
//! pass after the lit geometry (so wading hulls occlude it correctly above the waterline and
//! blend under it below), depth-tested but never writing depth, alpha-blended. All animation
//! is shader-side from the tick-domain presentation clock — the buffers upload once per scene.

use std::cell::RefCell;

use wgpu::util::DeviceExt;

use crate::offscreen::DEPTH_FORMAT;
use crate::shader_library::{CAMERA_COMMON_WGSL, LIGHTING_COMMON_WGSL, compose_shader};

pub fn water_shader_source() -> String {
    compose_shader(&[CAMERA_COMMON_WGSL, LIGHTING_COMMON_WGSL, include_str!("shaders/water.wgsl")])
}

/// Max screen-UV offset the wave tilt drives when refracting the scene behind the water. Tuned so
/// the bed wobbles clearly without dragging in geometry from far off the fragment (screen-space
/// refraction bleeds if the reach is large).
const REFRACTION_STRENGTH: f32 = 0.85;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct RefractionParams {
    strength: f32,
    _pad: [f32; 3],
}

const WATER_ATTRIBUTES: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x2];

/// Classic transparency: the fragment's alpha (the shore fade) weights the surface over the
/// riverbed beneath.
const WATER_BLEND: wgpu::BlendState = wgpu::BlendState::ALPHA_BLENDING;

/// Build the water pipeline against the shared scene camera bind-group layout (group 0) — the
/// full `Camera` struct including the time uniform the ripple runs on.
pub(crate) fn build_water_pipeline(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
    camera_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("water_shader"),
        source: wgpu::ShaderSource::Wgsl(water_shader_source().into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("water_pipeline_layout"),
        bind_group_layouts: &[Some(camera_bgl)],
        immediate_size: 0,
    });
    build_water_pipeline_variant(device, color_format, sample_count, &layout, &shader, "fs_main")
}

/// Shared pipeline body for both the analytic (`fs_main`) and refraction (`fs_refract`) water
/// entries — identical vertex layout, no-cull primitive, depth-test-no-write, alpha blend.
fn build_water_pipeline_variant(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    fragment_entry: &str,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("water_pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<renderer_api::WaterVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &WATER_ATTRIBUTES,
            }],
        },
        // No culling: the surface is legitimately seen from below the banks' sight lines, and a
        // submerged sniper camera looking up must not see a hole.
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState { count: sample_count, ..Default::default() },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(WATER_BLEND),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// The discrete-tier water refraction: a second pipeline (entry `fs_refract`) that samples a grab
/// of the resolved opaque HDR at group 1, plus the sampler + params buffer + a bind group rebound
/// whenever the grab texture is recreated (resize). Built alongside the analytic pipeline; used
/// only when the tier enables refraction AND the frame is multisampled (see `draw.rs`).
pub(crate) struct WaterRefraction {
    pub pipeline: wgpu::RenderPipeline,
    grab_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params: wgpu::Buffer,
    pub bind_group: RefCell<Option<wgpu::BindGroup>>,
}

impl WaterRefraction {
    pub(crate) fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        camera_bgl: &wgpu::BindGroupLayout,
    ) -> Self {
        let grab_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water_grab_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("water_refraction_shader"),
            source: wgpu::ShaderSource::Wgsl(water_shader_source().into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("water_refraction_pipeline_layout"),
            bind_group_layouts: &[Some(camera_bgl), Some(&grab_bgl)],
            immediate_size: 0,
        });
        let pipeline = build_water_pipeline_variant(
            device,
            color_format,
            sample_count,
            &layout,
            &shader,
            "fs_refract",
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water_grab_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("water_refraction_params"),
            contents: bytemuck::bytes_of(&RefractionParams {
                strength: REFRACTION_STRENGTH,
                _pad: [0.0; 3],
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        Self { pipeline, grab_bgl, sampler, params, bind_group: RefCell::new(None) }
    }

    /// (Re)bind the group-1 resources to the current grab texture — call after the HDR chain is
    /// recreated (resize), exactly like the post pass rebinds its input.
    pub(crate) fn rebuild_bind_group(&self, device: &wgpu::Device, grab_view: &wgpu::TextureView) {
        *self.bind_group.borrow_mut() =
            Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("water_grab_bg"),
                layout: &self.grab_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(grab_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry { binding: 2, resource: self.params.as_entire_binding() },
                ],
            }));
    }
}
