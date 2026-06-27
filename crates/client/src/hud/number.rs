//! Numeric HUD readouts. Digits are drawn with the real glyph font (`hud_font`); this module keeps
//! the shared readout colors and a thin right-aligned integer helper so existing call sites and
//! their layout (right edge at `right_x`, top at `top_y`) stay unchanged.
use renderer_api::HudVertex;

pub(crate) const FPS_COLOR: [f32; 4] = [0.72, 1.0, 0.78, 0.9];
pub(crate) const SPEED_COLOR: [f32; 4] = [0.78, 0.88, 1.00, 0.92];
pub(crate) const HP_COLOR: [f32; 4] = [0.86, 1.00, 0.88, 0.95];
pub(crate) const RELOAD_TIME_COLOR: [f32; 4] = [0.92, 0.78, 0.48, 0.94];
pub(crate) const TARGET_DISTANCE_COLOR: [f32; 4] = [0.82, 0.92, 1.00, 0.82];
/// Dim secondary tint for unit labels (KM/H, M) so they read as context next to the bright value.
pub(crate) const UNIT_COLOR: [f32; 4] = [0.72, 0.80, 0.74, 0.66];

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
