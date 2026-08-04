//! The unlit battle-FX pipeline: world-space soft quads (muzzle flash, smoke, dirt, sparks,
//! tracers) drawn inside the scene pass after the lit geometry, depth-tested against it but
//! never writing depth, blended premultiplied so one pipeline covers both additive glow
//! (alpha 0) and ordinary transparency (full premultiplied color).

use crate::offscreen::DEPTH_FORMAT;

pub fn fx_shader_source() -> String {
    // Teren F3: FX composes the shared camera + lighting fragments so physical media
    // (ruts, smoke, dust, scorch) can breathe the SAME air as the world through the one
    // `apply_fog` implementation — never a private fog fork.
    crate::shader_library::compose_shader(&[
        crate::shader_library::CAMERA_COMMON_WGSL,
        crate::shader_library::LIGHTING_COMMON_WGSL,
        include_str!("shaders/fx.wgsl"),
    ])
}

const FX_ATTRIBUTES: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32, 3 => Float32x4];

/// Premultiplied-alpha blending: `src * 1 + dst * (1 - src.a)`. With premultiplied colors this
/// one state renders the whole FX range — `alpha = 0` degenerates to pure additive.
const FX_BLEND: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
};

/// Build the FX pipeline against the shared scene camera bind-group layout (the FX shader
/// declares only the leading `view_proj` field of the camera uniform, which WGSL permits for a
/// larger bound buffer). Depth compare is `LessEqual` with writes off: effects sit *in* the
/// world (occluded by hulls and terrain) without transparent quads punching holes in each other.
pub(crate) fn build_fx_pipeline(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
    camera_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("fx_shader"),
        source: wgpu::ShaderSource::Wgsl(fx_shader_source().into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("fx_pipeline_layout"),
        bind_group_layouts: &[Some(camera_bgl)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("fx_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<renderer_api::FxVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &FX_ATTRIBUTES,
            }],
        },
        // No culling: billboards face the camera by construction, but stretched tracer quads and
        // ground-oriented dust rings are legitimately viewed from either side.
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
                blend: Some(FX_BLEND),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
