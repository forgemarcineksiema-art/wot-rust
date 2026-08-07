//! The dual-Kawase bloom ladder (`docs/art-direction-policy.md` rule 6): a mip chain of
//! Rgba16Float targets downsampled from the resolved HDR frame and tent-upsampled back,
//! composited over the sharp frame in the central post pass at the profile's small weight.
//! Threshold-free — energy decides what glows. Depth is a quality-tier knob
//! (`LightingQuality::bloom_mips`; 0 skips the chain and the post pass reads a black pixel).

use std::cell::RefCell;

use crate::shader_library::compose_shader;

use super::post::HDR_FORMAT;

fn bloom_shader_source() -> String {
    compose_shader(&[include_str!("../shaders/bloom.wgsl")])
}

/// One ladder level: the texture at `1/2^(level+1)` of the frame, its view, and the bind group
/// that reads it as a source.
struct BloomLevel {
    view: wgpu::TextureView,
    read_bind: wgpu::BindGroup,
}

pub(crate) struct BloomTargets {
    width: u32,
    height: u32,
    levels: Vec<BloomLevel>,
    /// Bind group reading the resolved HDR frame (the ladder's source).
    hdr_read_bind: wgpu::BindGroup,
    /// A 1x1 black fallback so the post pass's bloom binding is always valid when the ladder
    /// is off (weight 0 / bloom_mips 0).
    pub black_view: wgpu::TextureView,
}

pub(crate) struct BloomResources {
    pub down_pipeline: wgpu::RenderPipeline,
    pub up_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    pub sampler: wgpu::Sampler,
    /// Ladder depth from the quality tier; 0 disables the chain entirely.
    pub mips: u32,
    pub targets: RefCell<Option<BloomTargets>>,
}

impl BloomResources {
    pub fn new(device: &wgpu::Device, mips: u32) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom_bgl"),
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
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloom_shader"),
            source: wgpu::ShaderSource::Wgsl(bloom_shader_source().into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bloom_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let make_pipeline = |label: &str, entry: &str, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR_FORMAT,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let down_pipeline = make_pipeline("bloom_down", "fs_down", None);
        // Upsample ACCUMULATES onto the finer level: additive blend, alpha untouched.
        let up_pipeline = make_pipeline(
            "bloom_up",
            "fs_up",
            Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::REPLACE,
            }),
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("bloom_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            down_pipeline,
            up_pipeline,
            bind_group_layout,
            sampler,
            mips,
            targets: RefCell::new(None),
        }
    }

    /// (Re)build the ladder for the output size; also rebuilt when the HDR resolve view changes
    /// (the caller passes it fresh each ensure).
    pub fn ensure_targets(
        &self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        hdr_resolve_view: &wgpu::TextureView,
    ) {
        if self.mips == 0 {
            return;
        }
        {
            let current = self.targets.borrow();
            if let Some(t) = current.as_ref()
                && t.width == width
                && t.height == height
            {
                return;
            }
        }
        let make_read_bind = |view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bloom_read_bg"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            })
        };
        let mut levels = Vec::new();
        for level in 0..self.mips {
            let w = (width >> (level + 1)).max(1);
            let h = (height >> (level + 1)).max(1);
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("bloom_mip"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HDR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let read_bind = make_read_bind(&view);
            levels.push(BloomLevel { view, read_bind });
        }
        let black = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bloom_black"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let black_view = black.create_view(&wgpu::TextureViewDescriptor::default());
        *self.targets.borrow_mut() = Some(BloomTargets {
            width,
            height,
            levels,
            hdr_read_bind: make_read_bind(hdr_resolve_view),
            black_view,
        });
    }

    /// Encode the ladder: HDR resolve -> mip0 -> ... -> mipN (downsample), then mipN -> ... ->
    /// mip0 (tent upsample, additive). The post pass then reads mip0.
    /// The whole ladder is ONE measured span: the first rung opens it, the last closes it, and
    /// the rungs between write no timestamps of their own. Timing each rung separately would
    /// report a dozen numbers nobody can act on — the actionable question is what the bloom chain
    /// costs, not what its fourth downsample costs.
    pub fn encode(
        &self,
        recorder: &mut crate::pass_recorder::PassRecorder<'_>,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if self.mips == 0 {
            return;
        }
        let targets = self.targets.borrow();
        let Some(t) = targets.as_ref() else {
            return;
        };
        // A one-level ladder runs a single rung, so that rung has to carry both ends of the span
        // rather than opening one nothing closes.
        let rungs = t.levels.len() * 2 - 1;
        let mut rung = 0usize;
        let span_for = |index: usize| {
            if rungs == 1 {
                crate::pass_recorder::TimestampSpan::Whole
            } else if index == 0 {
                crate::pass_recorder::TimestampSpan::Start
            } else if index + 1 == rungs {
                crate::pass_recorder::TimestampSpan::End
            } else {
                crate::pass_recorder::TimestampSpan::Inside
            }
        };
        // Down the ladder.
        for (index, level) in t.levels.iter().enumerate() {
            let span = span_for(rung);
            rung += 1;
            let mut pass = begin_rung(
                recorder,
                encoder,
                &level.view,
                wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                span,
            );
            pass.set_pipeline(&self.down_pipeline);
            let src = if index == 0 { &t.hdr_read_bind } else { &t.levels[index - 1].read_bind };
            pass.set_bind_group(0, src, &[]);
            pass.draw(0..3, 0..1);
        }
        // Back up, accumulating.
        for index in (1..t.levels.len()).rev() {
            let span = span_for(rung);
            rung += 1;
            let mut pass =
                begin_rung(recorder, encoder, &t.levels[index - 1].view, wgpu::LoadOp::Load, span);
            pass.set_pipeline(&self.up_pipeline);
            pass.set_bind_group(0, &t.levels[index].read_bind, &[]);
            pass.draw(0..3, 0..1);
        }
    }

    /// The view the post pass composites (mip0), or the black fallback when the ladder is off.
    pub fn output_view(&self) -> Option<wgpu::TextureView> {
        let targets = self.targets.borrow();
        let t = targets.as_ref()?;
        Some(if self.mips > 0 && !t.levels.is_empty() {
            t.levels[0].view.clone()
        } else {
            t.black_view.clone()
        })
    }
}

/// One rung of the bloom ladder, opened through the recorder like every other pass.
fn begin_rung(
    recorder: &mut crate::pass_recorder::PassRecorder<'_>,
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
    span: crate::pass_recorder::TimestampSpan,
) -> wgpu::RenderPass<'static> {
    recorder
        .begin_span(
            encoder,
            crate::frame_graph::PassId::Bloom,
            span,
            &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
            })],
            None,
        )
        .forget_lifetime()
}
