//! Measures how SHARP a cast shadow's edge is, as a number.
//!
//! Built because eyeballing it produced a wrong answer. Comparing two renders that differed in
//! shadow-map resolution suggested resolution drove the softness; shrinking the focus box by the
//! same texel factor then changed nothing visible, which cannot both be true if the penumbra is
//! simply `3 × texel_world_size`. A wrong model survived because the instrument was a human
//! looking at two pictures framed differently.
//!
//! The metric is the **steepest luminance step** across the shadow boundary, sampled along rows
//! inside an authored ground box that the tank's shadow edge crosses. A hard edge concentrates
//! the whole light-to-dark drop into one or two pixels and scores high; a blurred one spreads the
//! same drop over many and scores low. Total variation would NOT work — it is the same either way
//! — which is exactly the trap that makes "it looks blurrier" so unreliable.
//!
//! `edge_width_px` divides the drop by the steepest step: roughly how many pixels the transition
//! occupies, which is the number to compare across settings.
//!
//! Sweep it (each knob alone — that is the whole point):
//! ```text
//! WOT_SHADOW_RES=2048 WOT_SHADOW_FOCUS=64 cargo run -p client --example probe -- shadow_probe
//! WOT_SHADOW_RES=8192 WOT_SHADOW_FOCUS=64 cargo run -p client --example probe -- shadow_probe
//! WOT_SHADOW_RES=2048 WOT_SHADOW_FOCUS=16 cargo run -p client --example probe -- shadow_probe
//! ```

use client::{REVIEWED_MAPS, render_review_views, review_views_for};

const WIDTH: u32 = 960;
const HEIGHT: u32 = 540;

/// The ground box the tank's cast shadow edge runs through in `prokhorovka_contact_backlit`.
/// Below the hull, left of it, clear of the grass tufts that would add their own edges.
const SHADOW_BOX: [f32; 4] = [0.10, 0.770, 0.42, 0.795];

fn srgb_to_linear(byte: u8) -> f32 {
    let c = byte as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let map = REVIEWED_MAPS[0];
    let battlefield = map_forge::battlefield(map);
    let views = review_views_for(map, &battlefield);
    let index = views
        .iter()
        .position(|view| view.name == "prokhorovka_contact_backlit")
        .ok_or("the backlit readability view is missing from the review set")?;
    let frames = render_review_views(map, &views, WIDTH, HEIGHT)?;
    let pixels = &frames[index];

    let x0 = (SHADOW_BOX[0] * WIDTH as f32) as usize;
    let x1 = (SHADOW_BOX[2] * WIDTH as f32) as usize;
    let y0 = (SHADOW_BOX[1] * HEIGHT as f32) as usize;
    let y1 = (SHADOW_BOX[3] * HEIGHT as f32) as usize;

    // Collapse the box to a 1-D profile by averaging DOWN each column. Grass cards are
    // alpha-cutout, so their edges are the hardest thing in any outdoor frame and a raw
    // max-gradient just reports a blade every time — measured, and it did: the number refused to
    // move across a 4x resolution sweep. Blades are high-frequency and uncorrelated between rows;
    // a roughly vertical shadow boundary is not. Averaging kills the first and keeps the second.
    let mut profile = vec![0.0f32; x1 - x0];
    for y in y0..y1 {
        let row = y * WIDTH as usize * 4;
        for (i, slot) in profile.iter_mut().enumerate() {
            let p = row + (x0 + i) * 4;
            *slot += 0.2126 * srgb_to_linear(pixels[p])
                + 0.7152 * srgb_to_linear(pixels[p + 1])
                + 0.0722 * srgb_to_linear(pixels[p + 2]);
        }
    }
    let rows = (y1 - y0) as f32;
    for slot in profile.iter_mut() {
        *slot /= rows;
    }

    let lit = profile.iter().copied().fold(f32::MIN, f32::max);
    let shade = profile.iter().copied().fold(f32::MAX, f32::min);
    let drop = lit - shade;
    // The 10-90% transition: how many columns the profile takes to cross the boundary. This is
    // the penumbra in pixels, and it is what "the shadow looks soft" actually means.
    let low = shade + drop * 0.1;
    let high = shade + drop * 0.9;
    let first_high = profile.iter().position(|v| *v >= high);
    let first_low = profile.iter().position(|v| *v <= low);
    let edge_width_px = match (first_high, first_low) {
        (Some(a), Some(b)) => (a as f32 - b as f32).abs(),
        _ => f32::INFINITY,
    };
    let steepest = profile.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);

    let res = std::env::var("WOT_SHADOW_RES").unwrap_or_else(|_| "tier".to_string());
    let focus = std::env::var("WOT_SHADOW_FOCUS").unwrap_or_else(|_| "default(64)".to_string());
    println!(
        "res {res} / focus {focus} m: steepest step {steepest:.4}, lit {lit:.3}, shade {shade:.3}, \
         drop {drop:.3}, edge width {edge_width_px:.1} px"
    );
    Ok(())
}
