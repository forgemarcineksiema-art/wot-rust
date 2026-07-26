use std::fs::File;
use std::io::BufWriter;

use client::{prokhorovka_review_views, render_review_views};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

/// Render the Prokhorovka steppe offscreen in its three times of day — the hazy noon, the
/// golden evening whose low western sun the shadow cascades rake long, and the dry lead
/// overcast — from the hill panorama and a mid-field vantage. The views are the canonical
/// review set (`client::prokhorovka_review_views`) and they go through
/// `client::render_review_views`, the same one path the `look_goldens` harness uses, so what a
/// human reviews here is exactly what the goldens lock — structurally, not by convention. One
/// PNG per view under `target/`: `cargo run -p client --example prokhorovka_views`
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
    let views = prokhorovka_review_views(&battlefield);
    let frames = render_review_views(terrain::MapId::ProkhorovkaHill252_2, &views, WIDTH, HEIGHT)?;

    for (view, pixels) in views.iter().zip(&frames) {
        let path = format!("target/{}.png", view.name);
        let file = File::create(&path)?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), WIDTH, HEIGHT);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(pixels)?;
        println!("wrote {path}");
    }
    Ok(())
}
