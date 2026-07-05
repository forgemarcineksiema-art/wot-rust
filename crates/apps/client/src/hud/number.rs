//! Numeric HUD readouts. Digits are drawn with the real glyph font (`hud_font`); this module keeps
//! the shared readout colors and a thin right-aligned integer helper so existing call sites and
//! their layout (right edge at `right_x`, top at `top_y`) stay unchanged.
use renderer_api::HudVertex;

use crate::hud::theme::{color as theme, tagged};

// Readout tints alias the art-direction tokens (`hud/theme.rs`): primary values read warm
// off-white like stenciled instrument markings, diagnostics stay soft, units stay dim. Each
// readout carries a unique alpha tag (`theme::tagged`) because the HUD tests identify features
// by exact vertex-color equality — shared RGB, distinct bytes.
pub(crate) const FPS_COLOR: [f32; 4] = tagged(theme::READOUT_SOFT, 0.75);
pub(crate) const SPEED_COLOR: [f32; 4] = tagged(theme::READOUT, 0.94);
pub(crate) const HP_COLOR: [f32; 4] = tagged(theme::READOUT, 0.95);
pub(crate) const RELOAD_TIME_COLOR: [f32; 4] = tagged(theme::ACCENT, 0.94);
pub(crate) const TARGET_DISTANCE_COLOR: [f32; 4] = tagged(theme::READOUT, 0.93);
/// Dim secondary tint for unit labels (KM/H, M) so they read as context next to the bright value.
pub(crate) const UNIT_COLOR: [f32; 4] = theme::UNIT;
/// Sniper magnification readout ("X6.9") under the reticle.
pub(crate) const ZOOM_COLOR: [f32; 4] = tagged(theme::READOUT_SOFT, 0.74);

/// Number of decimal digits in `n` (1 for zero); used to budget layout width for readouts.
pub(crate) fn digit_count(mut n: u32) -> u32 {
    if n == 0 {
        return 1;
    }
    let mut count = 0;
    while n > 0 {
        n /= 10;
        count += 1;
    }
    count
}

/// Draw `value` right-aligned with its right edge at `right_x` and top at `top_y`, em height
/// `height` clip units. X extents stay square via `aspect`.
pub(crate) fn push_number(
    vertices: &mut Vec<HudVertex>,
    value: u32,
    right_x: f32,
    top_y: f32,
    height: f32,
    aspect: f32,
    color: [f32; 4],
) {
    crate::hud::font::push_text_right(
        vertices,
        &value.to_string(),
        right_x,
        top_y,
        height,
        aspect,
        color,
    );
}
