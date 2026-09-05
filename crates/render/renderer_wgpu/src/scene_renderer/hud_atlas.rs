//! HUD texture resources: the glyph/coverage atlas (R8) and the material sheet (RGBA8) that the
//! one HUD bind group carries, with the samplers each wants and the placeholders that keep the
//! group valid before either upload. Kept beside `SceneRenderer` so the texture plumbing does
//! not crowd the renderer's construction and draw code.
//!
//! Two textures, one bind group: the group is rebuilt whenever either texture is replaced, from
//! the views the renderer keeps — so uploading the sheet does not lose the atlas, and vice versa.

use crate::GpuContext;
use crate::scene_pipeline::build_hud_font_bind_group_layout;

/// What the HUD pass binds at group 0, and the two views it was last built from.
pub(super) struct HudTextures {
    pub(super) layout: wgpu::BindGroupLayout,
    /// Clamped and linear: glyph coverage must not bleed across the atlas's shelves.
    pub(super) atlas_sampler: wgpu::Sampler,
    /// Repeating and linear: a plate's tile wraps every `TILE_UNITS` of local coordinate.
    pub(super) sheet_sampler: wgpu::Sampler,
    pub(super) atlas_view: wgpu::TextureView,
    pub(super) sheet_view: wgpu::TextureView,
    pub(super) bind_group: wgpu::BindGroup,
}

impl super::SceneRenderer {
    /// Upload the HUD glyph/coverage atlas: a single-channel (R8) image of `width`x`height` where
    /// each texel's red channel is text coverage in `[0, 255]`. Rebuilds the bind group so the next
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
        self.hud_textures.atlas_view =
            upload_atlas_r8(&ctx.device, &ctx.queue, "hud_font_atlas", width, height, coverage);
        self.hud_textures.rebuild_hud_bind_group(&ctx.device);
    }

    /// Upload the HUD material sheet (interface program F2): an RGBA8 image of `width`x`height`,
    /// the tiles a plate is cut from and what the `SHEET` style samples directly. Linear, not
    /// sRGB — a tile is a modulation map, not a picture. `rgba` must be exactly
    /// `width * height * 4` bytes.
    pub fn set_hud_material_sheet(
        &mut self,
        ctx: &GpuContext,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) {
        if width == 0 || height == 0 || rgba.len() != (width as usize * height as usize * 4) {
            return;
        }
        self.hud_textures.sheet_view =
            upload_sheet_rgba8(&ctx.device, &ctx.queue, "hud_material_sheet", width, height, rgba);
        self.hud_textures.rebuild_hud_bind_group(&ctx.device);
    }
}

impl HudTextures {
    /// The layout, the two samplers, and a bind group over a 1x1 opaque atlas and a 1x1 neutral
    /// sheet. The placeholders keep the group valid before any upload; solid HUD verts never read
    /// either (uv sentinel, `SOLID` style), so they only matter until the real uploads run.
    pub(super) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let layout = build_hud_font_bind_group_layout(device);
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("hud_font_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let sheet_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("hud_sheet_sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let atlas_view = upload_atlas_r8(device, queue, "hud_font_atlas", 1, 1, &[255]);
        // Neutral: the shader doubles a tile, so one half leaves a plate's colour untouched.
        let sheet_view =
            upload_sheet_rgba8(device, queue, "hud_material_sheet", 1, 1, &[128, 128, 128, 255]);
        let bind_group = build_hud_bind_group(
            device,
            &layout,
            &atlas_sampler,
            &sheet_sampler,
            &atlas_view,
            &sheet_view,
        );
        Self { layout, atlas_sampler, sheet_sampler, atlas_view, sheet_view, bind_group }
    }

    fn rebuild_hud_bind_group(&mut self, device: &wgpu::Device) {
        self.bind_group = build_hud_bind_group(
            device,
            &self.layout,
            &self.atlas_sampler,
            &self.sheet_sampler,
            &self.atlas_view,
            &self.sheet_view,
        );
    }
}

fn build_hud_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    atlas_sampler: &wgpu::Sampler,
    sheet_sampler: &wgpu::Sampler,
    atlas_view: &wgpu::TextureView,
    sheet_view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("hud_font_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(atlas_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(atlas_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(sheet_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(sheet_sampler),
            },
        ],
    })
}

/// Create an R8 texture, upload `coverage`, and return its view. Used for both the 1x1
/// placeholder and the real glyph atlas, so the texture setup lives in one place.
fn upload_atlas_r8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    width: u32,
    height: u32,
    coverage: &[u8],
) -> wgpu::TextureView {
    upload_2d(device, queue, label, width, height, wgpu::TextureFormat::R8Unorm, 1, coverage)
}

/// Create an RGBA8 (linear) texture, upload `rgba`, and return its view.
fn upload_sheet_rgba8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> wgpu::TextureView {
    upload_2d(device, queue, label, width, height, wgpu::TextureFormat::Rgba8Unorm, 4, rgba)
}

#[allow(clippy::too_many_arguments)]
fn upload_2d(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    bytes_per_texel: u32,
    data: &[u8],
) -> wgpu::TextureView {
    let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
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
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * bytes_per_texel),
            rows_per_image: Some(height),
        },
        size,
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
