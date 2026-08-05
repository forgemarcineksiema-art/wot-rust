//! The reticle contact sheet: every sight state side by side, each drawn over pale straw and
//! over deep shade. The vertex locks say what draws; this says whether a player can read it.
//! `cargo run -p client --example probe -- reticle_strip`

use std::fs::File;
use std::io::BufWriter;

use client::demo_reticle_strip;
use renderer_api::{Camera, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (1600u32, 900u32);
    let aspect = width as f32 / height as f32;

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[])?;
    let (font_w, font_h, font_coverage) = client::hud_font_atlas();
    renderer.set_hud_font_atlas(&ctx, font_w, font_h, font_coverage);

    // The sheet paints its own ground in every cell, so the scene behind it is never seen; the
    // camera only has to be valid.
    let camera = Camera::default();
    let projection = renderer_api::CameraProjectionPolicy::webgpu_default();
    let view_proj = view_projection_matrix(
        &camera,
        aspect,
        projection.near_plane_m(),
        projection.far_plane_m(),
    );

    renderer.set_hud(&ctx, &demo_reticle_strip(aspect));
    renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
    let pixels = target.read_rgba8(&ctx)?;
    let path = "target/reticle_strip.png";
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    println!("wrote {path} ({width}x{height})");
    Ok(())
}
