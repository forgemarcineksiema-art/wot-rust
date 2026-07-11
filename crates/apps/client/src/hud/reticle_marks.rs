//! The individual reticle glyphs: dispersion ring, reload arc, crosshair, blocked form, gun and
//! impact markers. `reticle_overlay` owns which of them draw in which mode; this module only
//! knows how each mark looks.

use renderer_api::HudVertex;

use super::primitives::{push_arc, push_quad, push_segment};
use super::reticle_overlay::{
    RETICLE_BLOCKED, RETICLE_GUN, RETICLE_IMPACT, RETICLE_RELOAD, RETICLE_RING,
    RETICLE_RING_CONVERGED, RETICLE_RING_OUTLINE,
};

/// Floor for the reload arc / flash radius: even a fully settled hairline ring leaves the arc
/// readable (it hugs the ring whenever the ring is larger).
const RELOAD_ARC_MIN_RADIUS: f32 = 0.040;
/// Kept for the ready/denied flashes' base scale.
const RELOAD_ARC_RADIUS: f32 = 0.055;

/// The secondary-marker fade band (impact X, gun circle): fully hidden below the low edge,
/// fully drawn above the high one. A BAND, not a threshold — the old single 0.022 cutoff made
/// the gun marker flicker on/off every frame while the turret settled across it.
const SEPARATION_FADE_LOW_CLIP: f32 = 0.014;
const SEPARATION_FADE_HIGH_CLIP: f32 = 0.030;

/// A continuous circle outline: short segments between consecutive points, not floating dots.
/// The ring is HONEST now: no fat minimum clamp — the old 0.025 floor sat a third-person
/// circle permanently on the clamp, ~40% larger than the real dispersion and dead to aiming.
/// A hairline floor only guards degeneracy; the visible size IS the server's dispersion, so
/// watching the circle shrink to its settled hairline is the convergence signal the sight
/// never had. A dark underlay ring keeps it readable on bright straw and dark shade alike.
pub(super) fn push_dispersion_ring(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    radius: f32,
    aspect: f32,
    converged: bool,
) {
    if radius <= 0.0 {
        return;
    }
    let radius = radius.clamp(0.008, 0.35);
    let color = if converged { RETICLE_RING_CONVERGED } else { RETICLE_RING };
    // Underlay first: a slightly offset dark twin on each side reads as an outline.
    push_arc(
        vertices,
        center,
        radius + 0.0018,
        0.0,
        std::f32::consts::TAU,
        40,
        aspect,
        RETICLE_RING_OUTLINE,
    );
    push_arc(vertices, center, radius, 0.0, std::f32::consts::TAU, 40, aspect, color);
}

/// The remaining reload as an arc that DRAINS clockwise from the top: full circle right after
/// firing, gone the instant the gun is ready. Nothing draws when loaded — a ready gun is silence.
pub(super) fn push_reload_arc(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    fraction: f32,
    ring_radius: f32,
    aspect: f32,
) {
    let remaining = (1.0 - fraction).clamp(0.0, 1.0);
    if remaining <= 0.0 {
        return;
    }
    let sweep = remaining * std::f32::consts::TAU;
    let start = std::f32::consts::FRAC_PI_2 - sweep; // ends at 12 o'clock, drains clockwise
    let segments = (remaining * 32.0).ceil().max(2.0) as u32;
    // The arc RIDES the dispersion ring (just outside it): one visual centre for one gun,
    // instead of the old fixed-radius arc fighting the live ring for the eye.
    let radius = (ring_radius + 0.008).max(RELOAD_ARC_MIN_RADIUS);
    push_arc(vertices, center, radius, start, sweep, segments, aspect, RETICLE_RELOAD);
}

/// Seconds the gun-ready flash lives at the reticle.
pub(crate) const READY_FLASH_TTL_S: f32 = 0.4;

/// A refused fire click: one short red pulse ring at the reticle — the visual twin of the
/// UiReject knock, so a swallowed shot is SEEN as refused, never wondered about. Distinct
/// from the blue ready flash by colour and by pulsing inward (denial) vs outward (ready).
pub(super) fn push_denied_flash(
    vertices: &mut Vec<renderer_api::HudVertex>,
    center: [f32; 2],
    age_s: f32,
    aspect: f32,
) {
    const DENIED_FLASH_S: f32 = 0.32;
    if !(0.0..DENIED_FLASH_S).contains(&age_s) {
        return;
    }
    let t = age_s / DENIED_FLASH_S;
    // Collapse inward from the reload-arc radius toward the marker, fading out.
    let radius = RELOAD_ARC_RADIUS * (1.0 - t * 0.55);
    let alpha = (1.0 - t) * 0.85;
    let color = [0.92, 0.28, 0.22, alpha];
    push_arc(vertices, center, radius, 0.0, std::f32::consts::TAU, 40, aspect, color);
}

/// The gun-ready beat: the instant the reload arc drains away, one thin ring expands from the
/// arc's radius and fades. The whole engagement rhythm times itself against this moment — it must
/// be readable without ever looking away from the sight picture.
pub(super) fn push_ready_flash(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    age_s: f32,
    aspect: f32,
) {
    if !(0.0..READY_FLASH_TTL_S).contains(&age_s) {
        return;
    }
    let t = age_s / READY_FLASH_TTL_S;
    let radius = RELOAD_ARC_RADIUS * (1.0 + 0.8 * t);
    let alpha = (1.0 - t) * 0.85;
    // The ready-state blue of the reload bar, so the two cues read as one system.
    push_arc(
        vertices,
        center,
        radius,
        0.0,
        std::f32::consts::TAU,
        40,
        aspect,
        [0.55, 0.85, 0.96, alpha],
    );
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

/// Alpha of a secondary marker by its separation from the aim: hidden below the fade band,
/// full above it. A BAND (0.014..0.030), not the old single 0.022 threshold whose zero-width
/// onset made the gun marker flicker on/off every frame while the turret settled across it.
pub(super) fn impact_separation_alpha(
    aim_clip: [f32; 2],
    marker_clip: [f32; 2],
    aspect: f32,
) -> f32 {
    let dx = (marker_clip[0] - aim_clip[0]) * aspect;
    let dy = marker_clip[1] - aim_clip[1];
    let separation = (dx * dx + dy * dy).sqrt();
    ((separation - SEPARATION_FADE_LOW_CLIP)
        / (SEPARATION_FADE_HIGH_CLIP - SEPARATION_FADE_LOW_CLIP))
        .clamp(0.0, 1.0)
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
