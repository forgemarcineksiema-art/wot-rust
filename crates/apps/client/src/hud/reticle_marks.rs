//! The individual reticle glyphs: dispersion ring, reload arc, crosshair, blocked form, gun and
//! impact markers. `reticle_overlay` owns which of them draw in which mode; this module only
//! knows how each mark looks.

use renderer_api::HudVertex;

use super::primitives::{push_arc, push_quad, push_segment};
use super::reticle_overlay::{
    RETICLE_BLOCKED, RETICLE_GUN, RETICLE_IMPACT, RETICLE_RELOAD, RETICLE_RING,
};

/// Fixed screen radius of the reload arc: inside any realistic dispersion ring, outside the
/// marker, independent of zoom so the eye always finds it in the same place.
const RELOAD_ARC_RADIUS: f32 = 0.055;

/// Below this screen-space gap a secondary marker (impact X, gun circle) sits inside the
/// crosshair; above it the separation is worth its own glyph.
const IMPACT_SEPARATION_CLIP: f32 = 0.022;

/// A continuous circle outline: short segments between consecutive points, not floating dots.
pub(super) fn push_dispersion_ring(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    radius: f32,
    aspect: f32,
) {
    if radius <= 0.0 {
        return;
    }
    let radius = radius.clamp(0.025, 0.25);
    push_arc(vertices, center, radius, 0.0, std::f32::consts::TAU, 40, aspect, RETICLE_RING);
}

/// The remaining reload as an arc that DRAINS clockwise from the top: full circle right after
/// firing, gone the instant the gun is ready. Nothing draws when loaded — a ready gun is silence.
pub(super) fn push_reload_arc(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    fraction: f32,
    aspect: f32,
) {
    let remaining = (1.0 - fraction).clamp(0.0, 1.0);
    if remaining <= 0.0 {
        return;
    }
    let sweep = remaining * std::f32::consts::TAU;
    let start = std::f32::consts::FRAC_PI_2 - sweep; // ends at 12 o'clock, drains clockwise
    let segments = (remaining * 32.0).ceil().max(2.0) as u32;
    push_arc(vertices, center, RELOAD_ARC_RADIUS, start, sweep, segments, aspect, RETICLE_RELOAD);
}

pub(super) fn push_crosshair(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    reach: f32,
    thick: f32,
    aspect: f32,
    color: [f32; 4],
) {
    push_quad(vertices, center, [reach / aspect, thick], color);
    push_quad(vertices, center, [thick / aspect, reach], color);
}

/// The BLOCKED form: the crosshair's four arms pulled apart around an empty center — visibly
/// "broken" at a glance, with no penetration coloring to lie over it.
pub(super) fn push_blocked_marker(vertices: &mut Vec<HudVertex>, center: [f32; 2], aspect: f32) {
    let (inner, outer) = (0.010, 0.026);
    let mid = (inner + outer) * 0.5;
    let half = (outer - inner) * 0.5;
    let thick = 0.0028;
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0_f32)] {
        push_quad(
            vertices,
            [center[0] + dx * mid / aspect, center[1] + dy * mid],
            [(half * dx.abs() + thick * dy.abs()) / aspect, half * dy.abs() + thick * dx.abs()],
            RETICLE_BLOCKED,
        );
    }
}

/// Alpha of a secondary marker by its separation from the aim: 0 at the merge threshold,
/// full at twice the threshold — a fade, not a pop.
pub(super) fn impact_separation_alpha(
    aim_clip: [f32; 2],
    marker_clip: [f32; 2],
    aspect: f32,
) -> f32 {
    let dx = (marker_clip[0] - aim_clip[0]) * aspect;
    let dy = marker_clip[1] - aim_clip[1];
    let separation = (dx * dx + dy * dy).sqrt();
    ((separation - IMPACT_SEPARATION_CLIP) / IMPACT_SEPARATION_CLIP).clamp(0.0, 1.0)
}

/// A small amber "X" marking where the shell actually lands.
pub(super) fn push_impact_marker(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    aspect: f32,
    alpha: f32,
) {
    let mut color = RETICLE_IMPACT;
    color[3] *= alpha;
    let reach_x = 0.016 / aspect;
    let reach_y = 0.016;
    for (sx, sy) in [(1.0, 1.0), (1.0, -1.0_f32)] {
        push_segment(
            vertices,
            [center[0] - reach_x * sx, center[1] - reach_y * sy],
            [center[0] + reach_x * sx, center[1] + reach_y * sy],
            0.0028,
            color,
        );
    }
}

/// The hollow gun marker: a small circle outline at the barrel's converged point, dimming as it
/// merges with the central marker.
pub(super) fn push_gun_marker(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    aspect: f32,
    alpha: f32,
) {
    let mut color = RETICLE_GUN;
    color[3] *= alpha;
    push_arc(vertices, center, 0.012, 0.0, std::f32::consts::TAU, 16, aspect, color);
}
