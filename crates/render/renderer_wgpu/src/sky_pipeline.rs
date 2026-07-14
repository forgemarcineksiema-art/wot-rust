//! The gradient-sky background pipeline: a single fullscreen triangle (no vertex buffer) shaded from
//! the per-pixel view ray into a zenith->horizon gradient plus a sun disc/haze. Drawn first in the
//! scene pass, behind the geometry (depth compare Always, no depth write), reusing the shared scene
//! camera bind group. See `docs/atmosphere-policy.md` phase 2.

use crate::offscreen::DEPTH_FORMAT;
use crate::shader_library::{CAMERA_COMMON_WGSL, LIGHTING_COMMON_WGSL, compose_shader};

pub fn sky_shader_source() -> String {
    compose_shader(&[CAMERA_COMMON_WGSL, LIGHTING_COMMON_WGSL, include_str!("shaders/sky.wgsl")])
}

/// Build the gradient-sky pipeline against the shared scene camera bind-group layout (group 0).
/// It draws three vertices generated in the shader, so it needs no vertex or instance buffers.
pub(crate) fn build_sky_pipeline(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
    camera_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("sky_shader"),
        source: wgpu::ShaderSource::Wgsl(sky_shader_source().into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("sky_pipeline_layout"),
        bind_group_layouts: &[Some(camera_bgl)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("sky_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        // A plain triangle list; the covering triangle is CCW but nothing depends on its winding, so
        // culling is off to stay robust against a flipped projection.
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        // Behind everything: compare Always so it always shades, write off so it never occludes the
        // geometry that draws over it next.
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            // LessEqual against the cleared far plane (the sky quad sits at z = w): only
            // pixels no opaque draw claimed survive — see draw_world_opaque's ordering note.
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
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
