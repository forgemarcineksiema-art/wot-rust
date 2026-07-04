use renderer_api::RenderError;

use crate::GpuContext;
use crate::msaa::{default_sample_count, validate_msaa_support};
use crate::scene_target::{SceneRenderTarget, store_op_for_target};

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

fn padded_bytes_per_row(width: u32) -> u32 {
    (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
}

/// An offscreen RGBA8 color target with a matching depth buffer and a mappable
/// readback buffer, so a frame can be rendered without a window and saved as a PNG.
pub struct OffscreenTarget {
    pub width: u32,
    pub height: u32,
    sample_count: u32,
    color: wgpu::Texture,
    pub color_view: wgpu::TextureView,
    msaa_color_view: Option<wgpu::TextureView>,
    pub depth_view: wgpu::TextureView,
    readback: wgpu::Buffer,
}

impl OffscreenTarget {
    pub fn new(ctx: &GpuContext, width: u32, height: u32) -> Result<Self, RenderError> {
        Self::new_with_sample_count(ctx, width, height, default_sample_count())
    }

    pub fn new_with_sample_count(
        ctx: &GpuContext,
        width: u32,
        height: u32,
        sample_count: u32,
    ) -> Result<Self, RenderError> {
        validate_msaa_support(ctx, COLOR_FORMAT, DEPTH_FORMAT, sample_count)?;
        let width = width.max(1);
        let height = height.max(1);
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let color = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen_color"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let msaa_color_view = if sample_count > 1 {
            Some(
                ctx.device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("offscreen_msaa_color"),
                        size,
                        mip_level_count: 1,
                        sample_count,
                        dimension: wgpu::TextureDimension::D2,
                        format: COLOR_FORMAT,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    })
                    .create_view(&wgpu::TextureViewDescriptor::default()),
            )
        } else {
            None
        };
        let depth = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen_depth"),
            size,
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("offscreen_readback"),
            size: u64::from(padded_bytes_per_row(width)) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Ok(Self {
            width,
            height,
            sample_count,
            color_view: color.create_view(&wgpu::TextureViewDescriptor::default()),
            msaa_color_view,
            depth_view: depth.create_view(&wgpu::TextureViewDescriptor::default()),
            color,
            readback,
        })
    }

    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }

    pub fn render_target(&self) -> SceneRenderTarget<'_> {
        SceneRenderTarget {
            color_view: self.msaa_color_view.as_ref().unwrap_or(&self.color_view),
            resolve_target: self.msaa_color_view.as_ref().map(|_| &self.color_view),
            depth_view: &self.depth_view,
            sample_count: self.sample_count,
            width: self.width,
            height: self.height,
        }
    }

    /// Copy the rendered color texture back into CPU memory as tightly-packed RGBA8.
    pub fn read_rgba8(&self, ctx: &GpuContext) -> Result<Vec<u8>, RenderError> {
        let bpr = padded_bytes_per_row(self.width);
        let mut encoder =
            ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
        );
        ctx.queue.submit(Some(encoder.finish()));

        self.readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        ctx.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| RenderError::new(format!("GPU readback poll failed: {error:?}")))?;

        let mapped = self.readback.slice(..).get_mapped_range();
        let row_bytes = (self.width * 4) as usize;
        let mut pixels = Vec::with_capacity(row_bytes * self.height as usize);
        for row in 0..self.height as usize {
            let start = row * bpr as usize;
            pixels.extend_from_slice(&mapped[start..start + row_bytes]);
        }
        drop(mapped);
        self.readback.unmap();
        Ok(pixels)
    }
}

/// Clear the target to a solid color (smoke test for the GPU path).
pub fn clear_color(
    ctx: &GpuContext,
    target: &OffscreenTarget,
    color: [f64; 4],
) -> Result<(), RenderError> {
    let target_view = target.render_target();
    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target_view.color_view,
            resolve_target: target_view.resolve_target,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: color[0],
                    g: color[1],
                    b: color[2],
                    a: color[3],
                }),
                store: store_op_for_target(target_view.resolve_target),
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    ctx.queue.submit(Some(encoder.finish()));
    Ok(())
}
