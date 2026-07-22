//! The foliage atlas (Imported Flora 2.0, FL-2): ONE 2D RGBA atlas + sampler bound at group 1
//! of the scene, shadow-caster and SSAO-prepass pipelines. Every scene fragment samples it at
//! the vertex UV lane; texels with alpha under the cutout threshold are discarded — in the
//! color pass AND in the depth passes, so a leaf's shadow is exactly its mask (the honesty
//! rule, in texture form).
//!
//! The default atlas is a single opaque-white texel: procedural content carries uv (0, 0),
//! samples pure white, multiplies by 1.0 and never discards — bit-exact with the pre-atlas
//! render. The importer (FL-3) replaces it with packed foliage regions.

pub(crate) struct FoliageAtlas {
    pub bind_group: wgpu::BindGroup,
    sampler: wgpu::Sampler,
}

/// The group-1 layout every atlas-sampling pipeline shares: one filterable 2D texture and
/// its sampler, fragment-stage only.
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
    /// The startup atlas: one opaque-white texel — the bit-exact no-op.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("foliage_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_group = upload_atlas(device, queue, layout, &sampler, &[255, 255, 255, 255], 1, 1);
        Self { bind_group, sampler }
    }

    /// Replace the atlas with packed foliage content (FL-3's importer output). Tight RGBA8,
    /// sRGB; dimensions are the caller's truth.
    pub fn set(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) {
        self.bind_group = upload_atlas(device, queue, layout, &self.sampler, rgba, width, height);
    }
}

fn upload_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    rgba: &[u8],
    width: u32,
    height: u32,
) -> wgpu::BindGroup {
    assert_eq!(rgba.len(), (width * height * 4) as usize, "tight RGBA8 atlas data");
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("foliage_atlas"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("foliage_atlas_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}
