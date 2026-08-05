//! The individual reticle glyphs: dispersion ring, reload arc, crosshair, blocked form, gun and
//! impact markers. `reticle_overlay` owns which of them draw in which mode; this module only
//! knows how each mark looks.

use renderer_api::HudVertex;

use super::primitives::{push_arc, push_quad, push_segment};
use super::reticle_overlay::{
    RETICLE_BLOCKED, RETICLE_GUN, RETICLE_IMPACT, RETICLE_LOADED, RETICLE_RELOAD, RETICLE_RING,
    RETICLE_RING_CONVERGED, RETICLE_RING_OUTLINE,
};

/// Floor for the reload arc / flash radius: even a fully settled hairline ring leaves the arc
/// readable (it hugs the ring whenever the ring is larger).
const RELOAD_ARC_MIN_RADIUS: f32 = 0.040;
/// Kept for the denied flash's base scale.
const RELOAD_ARC_RADIUS: f32 = 0.055;

/// Where the gun's own state draws: just outside the live dispersion ring, never inside a
/// hairline one. The loading arc and the loaded ring share it, so the red arc drains and the
/// green circle closes on the SAME line — one gun, one place on screen.
fn gun_state_radius(ring_radius: f32) -> f32 {
    (ring_radius + 0.008).max(RELOAD_ARC_MIN_RADIUS)
}

/// The secondary-marker fade band (impact X, gun marker), as a fraction of the LIVE dispersion
/// ring: hidden below the low edge, fully drawn above the high one. A BAND, not a threshold —
/// a zero-width cutoff made the markers flicker on/off every frame while the turret settled
/// across it.
///
/// Measured against the ring — that is, in milliradians — because that is the only honest scale
/// for "different enough from the crosshair to matter": inside its own dispersion cone the gun
/// cannot tell the two points apart, so neither should the sight. Fixed clip constants could
/// not do this: 0.014..0.030 clip is 7..15 mrad in the third-person view but barely 1..2 mrad
/// under 6.9x sniper zoom, so the marker stayed lit through the whole exponential tail of the
/// turret's fine lay — most nagging exactly where aiming is most deliberate. These fractions
/// reproduce the old band at the third-person settled ring and fix every other view.
const SEPARATION_FADE_LOW_RING: f32 = 0.75;
const SEPARATION_FADE_HIGH_RING: f32 = 1.60;
/// Floors for degenerate rings (an unseeded predictor, a hairline settled circle) so the band
/// can never collapse to zero width and pin a marker on screen.
const SEPARATION_FADE_LOW_FLOOR: f32 = 0.010;
const SEPARATION_FADE_HIGH_FLOOR: f32 = 0.022;

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
    // Underlay first: one slightly larger dark twin behind the ring reads as an outline.
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

/// The remaining reload as a RED arc that DRAINS clockwise from the top: full circle right after
/// firing, gone the instant the gun is ready. Red is the state, not an alarm — the trigger does
/// nothing while this draws, and [`push_ready_ring`] closes the same line in green when it does.
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
    push_arc(
        vertices,
        center,
        gun_state_radius(ring_radius),
        start,
        sweep,
        segments,
        aspect,
        RETICLE_RELOAD,
    );
}

/// Seconds the loaded ring holds at full strength before it starts dissolving.
pub(crate) const READY_RING_HOLD_S: f32 = 0.55;
/// Seconds the loaded ring lives in total (hold + dissolve).
pub(crate) const READY_RING_TTL_S: f32 = 0.95;

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

/// The loaded gun: the drained arc CLOSES into one full green circle on the same line, holds,
/// then dissolves into silence. No expansion, no second glyph — the state simply finished, and
/// the whole engagement rhythm times itself against that closing.
///
/// This replaced an expanding blue flash. That flash existed only because "ready" had no colour
/// of its own (a loaded gun drew nothing), so the moment needed an event glyph to be seen at
/// all; once red/green carry the state, the event is the colour change itself.
pub(super) fn push_ready_ring(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    age_s: f32,
    ring_radius: f32,
    aspect: f32,
) {
    if !(0.0..READY_RING_TTL_S).contains(&age_s) {
        return;
    }
    // Full strength through the hold, then a linear dissolve to nothing.
    let dissolve = (READY_RING_TTL_S - READY_RING_HOLD_S).max(1.0e-3);
    let fade = ((READY_RING_TTL_S - age_s) / dissolve).clamp(0.0, 1.0);
    let mut color = RETICLE_LOADED;
    color[3] *= fade;
    push_arc(
        vertices,
        center,
        gun_state_radius(ring_radius),
        0.0,
        std::f32::consts::TAU,
        40,
        aspect,
        color,
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

/// Alpha of a secondary marker by its separation from the aim, measured against the live
/// dispersion ring: hidden inside the gun's own cone, full once it is clearly outside.
pub(super) fn impact_separation_alpha(
    aim_clip: [f32; 2],
    marker_clip: [f32; 2],
    ring_radius: f32,
    aspect: f32,
) -> f32 {
    let dx = (marker_clip[0] - aim_clip[0]) * aspect;
    let dy = marker_clip[1] - aim_clip[1];
    let separation = (dx * dx + dy * dy).sqrt();
    let low = (ring_radius * SEPARATION_FADE_LOW_RING).max(SEPARATION_FADE_LOW_FLOOR);
    let high = (ring_radius * SEPARATION_FADE_HIGH_RING).max(SEPARATION_FADE_HIGH_FLOOR);
    ((separation - low) / (high - low).max(1.0e-4)).clamp(0.0, 1.0)
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

/// The hollow gun marker: a small DIAMOND outline where the barrel points at target range,
/// dimming as it merges with the central marker.
///
/// A diamond, not a circle, because at this sight every circle already means one thing — the
/// dispersion of this gun (the ring, the loading arc, the loaded ring, the denial pulse all
/// speak it). A second small circle carrying an unrelated meaning was a homonym: sitting on the
/// ring it read as a knot in it rather than as the barrel.
pub(super) fn push_gun_marker(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    aspect: f32,
    alpha: f32,
) {
    let mut color = RETICLE_GUN;
    color[3] *= alpha;
    const REACH: f32 = 0.013;
    let corners = [[0.0, REACH], [REACH, 0.0], [0.0, -REACH], [-REACH, 0.0_f32]];
    let point = |c: [f32; 2]| [center[0] + c[0] / aspect, center[1] + c[1]];
    for index in 0..corners.len() {
        push_segment(
            vertices,
            point(corners[index]),
            point(corners[(index + 1) % corners.len()]),
            0.0022,
            color,
        );
    }
}
