//! HUD font/coverage atlas resources: the bind-group layout, sampler, and the R8 texture upload
//! shared by the 1x1 placeholder and the real glyph atlas. Kept beside `SceneRenderer` so the
//! texture plumbing does not crowd the renderer's construction and draw code.

use crate::GpuContext;
use crate::scene_pipeline::build_hud_font_bind_group_layout;

impl super::SceneRenderer {
    /// Upload the HUD glyph/coverage atlas: a single-channel (R8) image of `width`x`height` where
    /// each texel's red channel is text coverage in `[0, 255]`. Replaces the bind group so the next
    /// HUD draw samples it. `coverage` must be exactly `width * height` bytes.
    pub fn set_hud_font_atlas(
        &mut self,
        ctx: &GpuContext,
        width: u32,
        height: u32,
        coverage: &[u8],
    ) {
        if width == 0 || height == 0 || coverage.len() != (width as usize * height as usize) {
            return;
        }
        self.hud_font_bind_group = build_hud_font_bind_group(
            &ctx.device,
            &ctx.queue,
            &self.hud_font_bgl,
            &self.hud_font_sampler,
            width,
            height,
            coverage,
        );
    }
}

/// Build the atlas bind-group layout, a linear sampler, and a 1x1 opaque placeholder bind group.
/// The placeholder keeps the bind group valid before any text atlas is uploaded; solid HUD verts
/// never read it (uv sentinel), so it only matters until `set_hud_font_atlas` runs.
pub(super) fn create_hud_font_resources(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::BindGroupLayout, wgpu::Sampler, wgpu::BindGroup) {
    let layout = build_hud_font_bind_group_layout(device);
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("hud_font_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    let bind_group = build_hud_font_bind_group(device, queue, &layout, &sampler, 1, 1, &[255]);
    (layout, sampler, bind_group)
}

/// Create the R8 atlas texture, upload `coverage`, and bind it with `sampler`. Used for both the
/// 1x1 placeholder and the real glyph atlas, so the texture/bind-group setup lives in one place.
fn build_hud_font_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width: u32,
    height: u32,
    coverage: &[u8],
) -> wgpu::BindGroup {
    let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hud_font_atlas"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
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
        coverage,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width),
            rows_per_image: Some(height),
        },
        size,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("hud_font_bg"),
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
