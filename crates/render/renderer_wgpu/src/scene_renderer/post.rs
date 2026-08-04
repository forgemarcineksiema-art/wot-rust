//! The HDR image-formation chain (`docs/art-direction-policy.md` rule 7): the world renders
//! linear HDR radiance into an internal `Rgba16Float` target (MSAA-resolved when the renderer
//! runs multisampled); one fullscreen post pass then applies the display transform (exposure ->
//! ACES-lite -> profile grade, shared with the CPU mirror `SceneLighting::grade_reference`) and
//! writes the final picture to the caller's sRGB target. Bloom and vignette slot in here later
//! — energy first, curve second, grade third.

use std::cell::RefCell;

use crate::GpuContext;
use crate::shader_library::{CAMERA_COMMON_WGSL, LIGHTING_COMMON_WGSL, compose_shader};

/// The linear-light intermediate format: renderable, blendable and multisample-capable in core
/// WebGPU, wide enough that a hot sun or muzzle flash survives to the tone curve instead of
/// clipping at 1.0 in the framebuffer.
pub(crate) const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The formed-picture intermediate the FXAA pass reads: plain (non-sRGB) 8-bit Unorm carrying
/// the post pass's sRGB-ENCODED output verbatim. Plain on purpose, twice over: the dither in
/// `post.wgsl` straddles exactly this texture's quantization grid, and FXAA wants its input
/// luma perceptual — a hardware-decoding sRGB view would hand it linear values instead.
pub(crate) const LDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn post_shader_source() -> String {
    compose_shader(&[
        CAMERA_COMMON_WGSL,
        LIGHTING_COMMON_WGSL,
        include_str!("../shaders/post.wgsl"),
    ])
}

fn fxaa_shader_source() -> String {
    compose_shader(&[
        CAMERA_COMMON_WGSL,
        LIGHTING_COMMON_WGSL,
        include_str!("../shaders/fxaa.wgsl"),
    ])
}

/// The framebuffer-sized HDR chain, rebuilt when the output size or sample count changes
/// (the SSAO ensure-targets pattern).
pub(crate) struct HdrTargets {
    width: u32,
    height: u32,
    sample_count: u32,
    /// The scene pass's colour attachment: MSAA when the renderer is multisampled.
    pub color_view: wgpu::TextureView,
    /// The single-sample resolved HDR frame the post pass reads. Equal to `color_view`'s
    /// texture when the renderer runs single-sampled.
    pub resolve_view: wgpu::TextureView,
    /// True when the scene pass needs a resolve attachment (sample_count > 1).
    pub multisampled: bool,
    /// The formed LDR picture (post pass output, sRGB-encoded bytes) the FXAA pass reads.
    pub ldr_view: wgpu::TextureView,
    /// The resolved HDR texture, kept for the diagnostic readback.
    resolve_texture: wgpu::Texture,
    /// Water-refraction grab: a single-sample HDR texture the opaque pass resolves into so the
    /// water pass can sample the scene BEHIND it. `Some` only when refraction is active (discrete
    /// tier + multisampled); `None` keeps the analytic single-pass water.
    pub grab_view: Option<wgpu::TextureView>,
}

pub(crate) struct PostResources {
    pub pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pub targets: RefCell<Option<HdrTargets>>,
    /// The pass's input bind group (HDR frame + bloom mip); rebuilt whenever either chain's
    /// targets are recreated.
    pub bind_group: RefCell<Option<wgpu::BindGroup>>,
}

impl PostResources {
    pub fn new(device: &wgpu::Device, camera_bgl: &wgpu::BindGroupLayout) -> Self {
        // The post pass forms the picture into the LDR intermediate; the FXAA pass owns the
        // caller's output format.
        let output_format = LDR_FORMAT;
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post_shader"),
            source: wgpu::ShaderSource::Wgsl(post_shader_source().into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("post_pipeline_layout"),
            bind_group_layouts: &[Some(camera_bgl), Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("post_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            // The post pass owns the whole output frame: no depth attachment, single-sampled.
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("post_bloom_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            pipeline,
            bind_group_layout,
            sampler,
            targets: RefCell::new(None),
            bind_group: RefCell::new(None),
        }
    }

    /// Rebuild the pass's input bind group (called whenever the HDR or bloom targets change).
    pub fn rebuild_bind_group(&self, device: &wgpu::Device, bloom_view: &wgpu::TextureView) {
        let targets = self.targets.borrow();
        let Some(t) = targets.as_ref() else {
            return;
        };
        *self.bind_group.borrow_mut() =
            Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("post_bg"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&t.resolve_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(bloom_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            }));
    }

    /// Make sure the HDR chain matches the output size/sample count; rebuilds lazily on change.
    /// Returns true when the targets were (re)created — the input bind group must follow.
    pub fn ensure_targets(
        &self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        sample_count: u32,
        refraction: bool,
    ) -> bool {
        // Refraction needs an MSAA-resolved grab of the opaque scene; single-sampled frames keep
        // the analytic water, so a grab there would be dead memory.
        let want_grab = refraction && sample_count > 1;
        let current = self.targets.borrow();
        if let Some(t) = current.as_ref()
            && t.width == width
            && t.height == height
            && t.sample_count == sample_count
            && t.grab_view.is_some() == want_grab
        {
            return false;
        }
        drop(current);

        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let resolve_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hdr_resolve"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let resolve_view = resolve_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let ldr_view = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("ldr_formed"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: LDR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        let multisampled = sample_count > 1;
        let color_view = if multisampled {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("hdr_msaa_color"),
                    size,
                    mip_level_count: 1,
                    sample_count,
                    dimension: wgpu::TextureDimension::D2,
                    format: HDR_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        } else {
            resolve_texture.create_view(&wgpu::TextureViewDescriptor::default())
        };
        let grab_view = want_grab.then(|| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("hdr_refraction_grab"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: HDR_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        });
        *self.targets.borrow_mut() = Some(HdrTargets {
            width,
            height,
            sample_count,
            color_view,
            resolve_view,
            multisampled,
            ldr_view,
            resolve_texture,
            grab_view,
        });
        true
    }
}

/// The FXAA pass: reads the formed LDR picture and writes the caller's sRGB target — the one
/// anti-aliasing every player gets (the one-look policy ships 1x MSAA on every adapter, so
/// without this pass the shipped game had NO anti-aliasing at all). See `fxaa.wgsl` for why
/// filtering happens on the ENCODED picture.
pub(crate) struct FxaaResources {
    pub pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pub bind_group: RefCell<Option<wgpu::BindGroup>>,
}

impl FxaaResources {
    pub fn new(
        device: &wgpu::Device,
        output_format: wgpu::TextureFormat,
        camera_bgl: &wgpu::BindGroupLayout,
    ) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fxaa_bgl"),
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
            label: Some("fxaa_shader"),
            source: wgpu::ShaderSource::Wgsl(fxaa_shader_source().into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fxaa_pipeline_layout"),
            bind_group_layouts: &[Some(camera_bgl), Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fxaa_pipeline"),
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
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("fxaa_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self { pipeline, bind_group_layout, sampler, bind_group: RefCell::new(None) }
    }

    /// Rebuild the pass's input bind group (called whenever the LDR intermediate is recreated).
    pub fn rebuild_bind_group(&self, device: &wgpu::Device, ldr_view: &wgpu::TextureView) {
        *self.bind_group.borrow_mut() =
            Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("fxaa_bg"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(ldr_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            }));
    }
}

impl super::SceneRenderer {
    /// Diagnostic readback of the resolved linear-HDR frame as raw RGBA16F bytes (row-major,
    /// tightly packed). This is the image-formation lock's window into the pipeline: values
    /// above 1.0 must survive to the post pass instead of clipping in an 8-bit framebuffer.
    pub fn read_hdr_rgba16(&self, ctx: &GpuContext) -> Option<Vec<u8>> {
        let targets = self.post.targets.borrow();
        let targets = targets.as_ref()?;
        let bytes_per_row_unpadded = targets.width * 8;
        let bpr = bytes_per_row_unpadded.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hdr_readback"),
            size: u64::from(bpr) * u64::from(targets.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder =
            ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &targets.resolve_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(targets.height),
                },
            },
            wgpu::Extent3d {
                width: targets.width,
                height: targets.height,
                depth_or_array_layers: 1,
            },
        );
        ctx.queue.submit(Some(encoder.finish()));
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        ctx.device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        let mapped = buffer.slice(..).get_mapped_range();
        let row_bytes = bytes_per_row_unpadded as usize;
        let mut out = Vec::with_capacity(row_bytes * targets.height as usize);
        for row in 0..targets.height as usize {
            let start = row * bpr as usize;
            out.extend_from_slice(&mapped[start..start + row_bytes]);
        }
        drop(mapped);
        buffer.unmap();
        Some(out)
    }
}
