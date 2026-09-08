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
    // Retain GPU handles, never the decoded upload payloads.
    pages: AtlasPages,
    bark: AtlasPages,
}

struct AtlasPages {
    color: wgpu::TextureView,
    normal: wgpu::TextureView,
    sampler: wgpu::Sampler,
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
            // The bark ARRAY (route 2): one albedo (sRGB) and one tangent-normal layer per
            // species, world-triplanar; the layer rides the vertex's bark role.
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
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
        let pages = upload_pages(device, queue, &chain, None);
        let bark = upload_bark(device, queue, None);
        let bind_group = bind_pages(device, layout, &pages, &bark);
        Self { bind_group, pages, bark }
    }

    pub fn set(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        chain: &Rgba8MipChain,
        normals: Option<&Rgba8MipChain>,
    ) {
        tracing::info!(
            component = "foliage_atlas",
            gpu_payload_bytes = chain_bytes(chain) + normals.map_or(4, chain_bytes),
            retained_cpu_payload_bytes = 0,
            "texture memory"
        );
        self.pages = upload_pages(device, queue, chain, normals);
        self.bind_group = bind_pages(device, layout, &self.pages, &self.bark);
    }

    pub fn set_bark(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        layers: &[(Rgba8MipChain, Rgba8MipChain)],
    ) {
        assert!(!layers.is_empty(), "a bark array needs a layer");
        tracing::info!(
            component = "bark_arrays",
            gpu_payload_bytes =
                layers.iter().map(|(a, n)| chain_bytes(a) + chain_bytes(n)).sum::<usize>(),
            retained_cpu_payload_bytes = 0,
            "texture memory"
        );
        self.bark = upload_bark(device, queue, Some(layers));
        self.bind_group = bind_pages(device, layout, &self.pages, &self.bark);
    }
}

fn chain_bytes(chain: &Rgba8MipChain) -> usize {
    chain.levels().iter().map(|level| level.rgba().len()).sum()
}

/// Upload a stack of same-sized mip chains as one 2D array texture.
fn upload_array(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    chains: &[&Rgba8MipChain],
    srgb: bool,
) -> wgpu::TextureView {
    let base = &chains[0].levels()[0];
    let levels = chains[0].levels().len();
    for chain in chains {
        assert_eq!(chain.levels().len(), levels, "{label}: every layer has the same chain");
        assert_eq!(
            (chain.levels()[0].width(), chain.levels()[0].height()),
            (base.width(), base.height()),
            "{label}: every layer has the same size"
        );
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: base.width(),
            height: base.height(),
            depth_or_array_layers: chains.len() as u32,
        },
        mip_level_count: levels as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: if srgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        },
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (layer, chain) in chains.iter().enumerate() {
        for (mip_level, mip) in chain.levels().iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: mip_level as u32,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: layer as u32 },
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
    }
    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

fn upload_pages(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    chain: &Rgba8MipChain,
    normals: Option<&Rgba8MipChain>,
) -> AtlasPages {
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
    let color = upload_page("foliage_atlas", chain, true);
    AtlasPages { color, normal: normal_view, sampler }
}

fn upload_bark(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bark: Option<&[(Rgba8MipChain, Rgba8MipChain)]>,
) -> AtlasPages {
    let flat_normal =
        Rgba8MipChain::new(vec![Rgba8MipLevel::new(1, 1, vec![128, 128, 255, 255])], 0);
    let default_bark = vec![(
        Rgba8MipChain::new(vec![Rgba8MipLevel::new(1, 1, DEFAULT_BARK_SRGB.to_vec())], 0),
        flat_normal.clone(),
    )];
    let layers: &[(Rgba8MipChain, Rgba8MipChain)] = bark.unwrap_or(&default_bark);
    let albedo_layers: Vec<&Rgba8MipChain> = layers.iter().map(|(albedo, _)| albedo).collect();
    let normal_layers: Vec<&Rgba8MipChain> = layers.iter().map(|(_, normal)| normal).collect();
    let bark_albedo_view = upload_array(device, queue, "bark_albedo", &albedo_layers, true);
    let bark_normal_view = upload_array(device, queue, "bark_normal", &normal_layers, false);
    let bark_max_level = layers
        .iter()
        .map(|(albedo, normal)| albedo.max_sampled_level().max(normal.max_sampled_level()))
        .max()
        .unwrap_or(0);
    let bark_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("bark_sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        lod_min_clamp: 0.0,
        lod_max_clamp: bark_max_level as f32,
        anisotropy_clamp: 8,
        ..Default::default()
    });
    AtlasPages { color: bark_albedo_view, normal: bark_normal_view, sampler: bark_sampler }
}

fn bind_pages(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    pages: &AtlasPages,
    bark: &AtlasPages,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("foliage_atlas_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&pages.color),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&pages.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&pages.normal),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&bark.color),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&bark.normal),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(&bark.sampler),
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_bark_keeps_the_uploaded_leaf_pages_and_vice_versa() {
        let ctx = crate::GpuContext::headless().expect("GPU for atlas ownership lock");
        let layout = build_foliage_bind_group_layout(&ctx.device);
        let mut atlas = FoliageAtlas::new(&ctx.device, &ctx.queue, &layout);
        let chain = Rgba8MipChain::new(vec![Rgba8MipLevel::new(1, 1, vec![17, 31, 127, 255])], 0);
        atlas.set(&ctx.device, &ctx.queue, &layout, &chain, Some(&chain));
        let leaves = (atlas.pages.color.clone(), atlas.pages.normal.clone());
        atlas.set_bark(&ctx.device, &ctx.queue, &layout, &[(chain.clone(), chain.clone())]);
        assert_eq!(atlas.pages.color, leaves.0);
        assert_eq!(atlas.pages.normal, leaves.1);
        let bark = (atlas.bark.color.clone(), atlas.bark.normal.clone());
        atlas.set(&ctx.device, &ctx.queue, &layout, &chain, None);
        assert_eq!(atlas.bark.color, bark.0);
        assert_eq!(atlas.bark.normal, bark.1);
    }
}
