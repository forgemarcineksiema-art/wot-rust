use std::fs::File;
use std::io::BufWriter;

use client::{render_review_views_with_fov, review_views_for};
use terrain::MapId;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

/// Świat 2.0 PR2 (kamera) before/after: render every shipped map's canonical review views at an
/// EXPLICIT vertical FOV, so the 62° → 48° battle-lens verdict compares identical frames. The
/// look goldens stay at the review lens (55°) — this probe is the one place a different FOV is
/// rendered on purpose.
///
///   cargo run -p client --example probe -- fov_probe -- 62
///   cargo run -p client --example probe -- fov_probe -- 48
///
/// One PNG per view under `target/fov<fov>/<view>.png`. Compare the two directories side by side.
pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let fov_arg = match crate::sub_arg(1).as_deref() {
        Some("--") => crate::sub_arg(2),
        other => other.map(str::to_string),
    };
    let fov: f32 = fov_arg
        .as_deref()
        .unwrap_or("48")
        .parse()
        .map_err(|_| "usage: fov_probe -- <vertical fov degrees, e.g. 62 or 48>".to_string())?;
    if !(20.0..=100.0).contains(&fov) {
        return Err(format!("fov {fov}° is outside the sane 20–100° band").into());
    }

    let out_dir = format!("target/fov{}", fov.round() as u32);
    std::fs::create_dir_all(&out_dir)?;

    for &map in MapId::SHIPPED {
        let battlefield = map_forge::battlefield(map);
        let views = review_views_for(map, &battlefield);
        let frames = render_review_views_with_fov(map, &views, WIDTH, HEIGHT, fov)?;
        for (view, pixels) in views.iter().zip(&frames) {
            let path = format!("{out_dir}/{}.png", view.name);
            let file = File::create(&path)?;
            let mut encoder = png::Encoder::new(BufWriter::new(file), WIDTH, HEIGHT);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header()?.write_image_data(pixels)?;
            println!("wrote {path}");
        }
    }
    Ok(())
}
