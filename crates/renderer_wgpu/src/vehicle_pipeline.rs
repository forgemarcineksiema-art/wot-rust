//! The PBR-lite vehicle render pipeline: a separate path from the scene pipeline that consumes
//! [`renderer_api::VehicleVertex`] (tangent frame, uv, material id, tint mask) plus a per-instance
//! model/tint, feeding the normal-mapped vehicle shader. Terrain and simple meshes stay on the
//! lighter scene pipeline; this keeps the heavier vertex format off everything that doesn't need it.

use crate::offscreen::DEPTH_FORMAT;
use crate::scene_resources::SceneInstance;

pub fn vehicle_shader_source() -> &'static str {
    include_str!("shaders/vehicle.wgsl")
}

const VEHICLE_VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
    0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 3 => Float32x2, 4 => Uint32, 5 => Float32];
const VEHICLE_INSTANCE_ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x4, 10 => Float32x4];

/// Build the PBR-lite vehicle pipeline (camera uniform at group 0, depth-tested back-face-culled
/// triangles) plus its camera bind-group layout. Mirrors the scene pipeline's rigging so the two
/// can share camera uploads and render targets.
pub fn build_vehicle_pipeline(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("vehicle_shader"),
        source: wgpu::ShaderSource::Wgsl(vehicle_shader_source().into()),
    });
    let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("vehicle_camera_bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("vehicle_pipeline_layout"),
        bind_group_layouts: &[Some(&camera_bgl)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("vehicle_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<renderer_api::VehicleVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &VEHICLE_VERTEX_ATTRIBUTES,
                },
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SceneInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &VEHICLE_INSTANCE_ATTRIBUTES,
                },
            ],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
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
    });
    (pipeline, camera_bgl)
}
