//! The HUD golden instrument's frames at 1080p, for eyes (interface program F8): every
//! `HudState` in every size class over its frozen battlefield frame, to `target/hud_states/`,
//! and the per-element vertex census of every state (F9). `cargo run -p client --example probe
//! -- hud_states`. The goldens lock the same frames at 960x540.

use std::fs::File;
use std::io::BufWriter;

use client::{HudSizeClass, HudState};

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (1920u32, 1080u32);
    let views = client::hud_review_views();
    let frames = client::render_hud_review_views(&views, width, height)?;
    std::fs::create_dir_all("target/hud_states")?;
    for (view, pixels) in views.iter().zip(&frames) {
        let path = format!("target/hud_states/{}.png", view.name);
        let file = File::create(&path)?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(pixels)?;
        println!("wrote {path}");
    }

    println!("\nHUD CENSUS (vertices per element, {}x{}):", width, height);
    let aspect = width as f32 / height as f32;
    let mut busiest = (HudState::ThirdPersonIdle, 0usize);
    for state in HudState::ALL {
        let census = client::hud_state_census(state, aspect);
        let total: usize = census.iter().map(|(_, n)| n).sum();
        let emitted =
            client::hud_state_vertices(state, HudSizeClass::Standard, width, height).len();
        println!("  {:<18} {:>6} vertices ({} emitted)", state.name(), total, emitted);
        for (id, n) in census {
            println!("      {:<16} {:>6}", format!("{id:?}"), n);
        }
        if emitted > busiest.1 {
            busiest = (state, emitted);
        }
    }
    println!("  busiest: {:?} at {} of 16384", busiest.0, busiest.1);
    Ok(())
}
