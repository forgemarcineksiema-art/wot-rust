use std::fs::File;
use std::io::BufWriter;

use client::{render_review_views, review_views_for};
use terrain::MapId;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

/// Render a map's canonical review views offscreen — every look its blueprint declares, plus the
/// identity frames that say what the map is. The views come from `client::review_views_for` and
/// go through `client::render_review_views`, the same one path the `look_goldens` harness uses,
/// so what a human reviews here is exactly what the goldens lock — structurally, not by
/// convention.
///
/// One PNG per view under `target/`. Defaults to Prokhorovka; pass a map slug for another:
/// `cargo run -p client --example prokhorovka_views -- bystra-valley`
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let slug = std::env::args().nth(1);
    let map = match &slug {
        Some(slug) => MapId::from_slug(slug)
            .ok_or_else(|| format!("unknown map slug {slug:?}; try one of {:?}", MapId::ALL))?,
        None => MapId::ProkhorovkaHill252_2,
    };

    let battlefield = map_forge::battlefield(map);
    let views = review_views_for(map, &battlefield);
    let frames = render_review_views(map, &views, WIDTH, HEIGHT)?;

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
