//! One sRGB foliage atlas shared by scene color, shadow and SSAO depth pipelines.
//! The startup chain is a bit-exact opaque-white 1x1 no-op for procedural UV (0, 0).

use renderer_api::{Rgba8MipChain, Rgba8MipLevel};

pub(crate) struct FoliageAtlas {
    pub bind_group: wgpu::BindGroup,
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
        ],
    })
}

impl FoliageAtlas {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout) -> Self {
        let chain = Rgba8MipChain::new(vec![Rgba8MipLevel::new(1, 1, vec![255, 255, 255, 255])], 0);
        Self { bind_group: upload_foliage_atlas(device, queue, layout, &chain) }
    }

    pub fn set(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        chain: &Rgba8MipChain,
    ) {
        self.bind_group = upload_foliage_atlas(device, queue, layout, chain);
    }
}

fn upload_foliage_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    chain: &Rgba8MipChain,
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
        ],
    })
}
