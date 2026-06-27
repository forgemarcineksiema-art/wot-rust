use std::fs::File;
use std::io::BufWriter;

use renderer_wgpu::{GpuContext, OffscreenTarget, clear_color};

/// Render a frame offscreen (no window) and write it to a PNG, so the renderer can be
/// inspected headlessly. Usage: `cargo run -p renderer_wgpu --example screenshot -- out.png`
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| "target/screenshot.png".to_string());
    let width = 640u32;
    let height = 360u32;

    let ctx = GpuContext::headless()?;
    println!("adapter: {:?}", ctx.adapter.get_info());

    let target = OffscreenTarget::new(&ctx, width, height)?;
    clear_color(&ctx, &target, [0.45, 0.62, 0.85, 1.0])?;
    let pixels = target.read_rgba8(&ctx)?;

    let file = File::create(&path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    println!("wrote {path} ({width}x{height})");
    Ok(())
}
