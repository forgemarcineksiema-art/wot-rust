//! One sRGB foliage atlas shared by scene color, shadow and SSAO depth pipelines.
//! The startup chain is a bit-exact opaque-white 1x1 no-op for procedural UV (0, 0).
//!
//! Route 2 (2026-09-02, trees as data): the same bind group carries the BARK pair — an
//! albedo and a tangent-normal tile, sampled triplanar in world space by every bark
//! fragment (`surface_role::BARK`) through a REPEAT sampler. The startup default is one
//! texel of the authored trunk tone and one flat normal, so an unbound path keeps the
//! pre-texture look instead of a white trunk.

use renderer_api::{Rgba8MipChain, Rgba8MipLevel};

pub(crate) struct FoliageAtlas {
    pub bind_group: wgpu::BindGroup,
    /// The atlas pages last set, kept so a bark set can rebuild the bind group (and the
    /// other way round) without the caller re-sending both.
    atlas: (Rgba8MipChain, Option<Rgba8MipChain>),
    bark: Option<(Rgba8MipChain, Rgba8MipChain)>,
}

pub(crate) fn build_foliage_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("foliage_atlas_bgl"),
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
            // The tangent-normal page (hero-flora): bark relief for the near rung. Always
            // BOUND — an absent page binds a 1x1 flat normal, so the shader has one code path
            // and no branch on a uniform nobody would be able to see change.
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // The bark pair (route 2): albedo (sRGB) and tangent normals, world-triplanar.
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // A REPEAT sampler: bark tiles along a trunk, the atlas never wraps.
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// The startup bark texel: the authored trunk tone (scene_build's `TRUNK_TONE`, linear
/// (0.30, 0.22, 0.14)) in sRGB, so an unbound bark reads as it did before textures.
const DEFAULT_BARK_SRGB: [u8; 4] = [149, 130, 105, 255];

impl FoliageAtlas {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout) -> Self {
        let chain = Rgba8MipChain::new(vec![Rgba8MipLevel::new(1, 1, vec![255, 255, 255, 255])], 0);
        let bind_group = upload_foliage_atlas(device, queue, layout, &chain, None, None);
        Self { bind_group, atlas: (chain, None), bark: None }
    }

    pub fn set(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        chain: &Rgba8MipChain,
        normals: Option<&Rgba8MipChain>,
    ) {
        self.atlas = (chain.clone(), normals.cloned());
        self.rebuild(device, queue, layout);
    }

    pub fn set_bark(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        albedo: &Rgba8MipChain,
        normals: &Rgba8MipChain,
    ) {
        self.bark = Some((albedo.clone(), normals.clone()));
        self.rebuild(device, queue, layout);
    }

    fn rebuild(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) {
        self.bind_group = upload_foliage_atlas(
            device,
            queue,
            layout,
            &self.atlas.0,
            self.atlas.1.as_ref(),
            self.bark.as_ref().map(|(albedo, normals)| (albedo, normals)),
        );
    }
}

fn upload_foliage_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    chain: &Rgba8MipChain,
    normals: Option<&Rgba8MipChain>,
    bark: Option<(&Rgba8MipChain, &Rgba8MipChain)>,
) -> wgpu::BindGroup {
    let base = &chain.levels()[0];
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("foliage_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        lod_min_clamp: 0.0,
        lod_max_clamp: chain.max_sampled_level() as f32,
        // Leaf cards live at grazing angles (a crown seen from a tank is all oblique
        // quads); isotropic mips smear them into streaks. 8x anisotropy is the standard
        // foliage fix and costs a rounding error on this generation of hardware.
        anisotropy_clamp: 8,
        ..Default::default()
    });
    let upload_page = |label: &str, chain: &Rgba8MipChain, srgb: bool| {
        let base = &chain.levels()[0];
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: base.width(),
                height: base.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: chain.levels().len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Normals are VECTORS: decoding them through the sRGB curve would bend every
            // slope toward the light and read as a different surface.
            format: if srgb {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            },
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (mip_level, mip) in chain.levels().iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: mip_level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                mip.rgba(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(mip.width() * 4),
                    rows_per_image: Some(mip.height()),
                },
                wgpu::Extent3d {
                    width: mip.width(),
                    height: mip.height(),
                    depth_or_array_layers: 1,
                },
            );
        }
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    };
    // A flat 1x1 (128, 128, 255) stands in when nothing shipped normals: sampling it is a
    // cached fetch that decodes to the geometric normal, which IS the pre-normal look.
    let flat_normal =
        Rgba8MipChain::new(vec![Rgba8MipLevel::new(1, 1, vec![128, 128, 255, 255])], 0);
    let normal_view = upload_page("foliage_normal_atlas", normals.unwrap_or(&flat_normal), false);
    let default_bark =
        Rgba8MipChain::new(vec![Rgba8MipLevel::new(1, 1, DEFAULT_BARK_SRGB.to_vec())], 0);
    let (bark_albedo, bark_normal) = bark.unwrap_or((&default_bark, &flat_normal));
    let bark_albedo_view = upload_page("bark_albedo", bark_albedo, true);
    let bark_normal_view = upload_page("bark_normal", bark_normal, false);
    let bark_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("bark_sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        lod_min_clamp: 0.0,
        lod_max_clamp: bark_albedo.max_sampled_level().max(bark_normal.max_sampled_level()) as f32,
        anisotropy_clamp: 8,
        ..Default::default()
    });
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("foliage_atlas"),
        size: wgpu::Extent3d {
            width: base.width(),
            height: base.height(),
            depth_or_array_layers: 1,
        },
        mip_level_count: chain.levels().len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (mip_level, mip) in chain.levels().iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip_level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            mip.rgba(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(mip.width() * 4),
                rows_per_image: Some(mip.height()),
            },
            wgpu::Extent3d { width: mip.width(), height: mip.height(), depth_or_array_layers: 1 },
        );
    }
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("foliage_atlas_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&normal_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&bark_albedo_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&bark_normal_view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(&bark_sampler),
            },
        ],
    })
}
