//! The river-surface pipeline: a static mesh on the still-water plane, drawn inside the scene
//! pass after the lit geometry (so wading hulls occlude it correctly above the waterline and
//! blend under it below), depth-tested but never writing depth, alpha-blended. All animation
//! is shader-side from the tick-domain presentation clock — the buffers upload once per scene.

use crate::offscreen::DEPTH_FORMAT;

pub fn water_shader_source() -> &'static str {
    include_str!("shaders/water.wgsl")
}

const WATER_ATTRIBUTES: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32];

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
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("water_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
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
            module: &shader,
            entry_point: Some("fs_main"),
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
