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
/// Segments a circle of this radius needs to read as a circle. A settled third-person ring is
/// 1.7 px across; forty segments of it were forty degenerate quads, and the doubled underlay
/// turned that waste into a real cost. The chord error at the cap (48 segments on the widest
/// bloomed sniper ring) is a quarter of a pixel.
fn arc_segments(radius: f32) -> u32 {
    ((radius * 640.0).ceil() as u32).clamp(12, 48)
}

/// Where the gun's own state draws: just outside the live dispersion ring, never inside a
/// hairline one. The loading arc and the ready ring share it, so the red arc drains and the cyan
/// circle closes on the SAME line — one gun, one place on screen.
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
/// not do this — 0.014..0.030 clip measured through each view is
///
/// | view | band |
/// |---|---|
/// | third person (62 deg) | 8.4 .. 18.0 mrad |
/// | sniper, 8 deg step (x7.8) | 0.98 .. 2.10 mrad |
/// | sniper, 3 deg step (x20.7) | 0.37 .. 0.79 mrad |
///
/// so under zoom the marker stayed lit through the whole exponential tail of the turret's fine
/// lay, most nagging exactly where aiming is most deliberate. In third person the floors below
/// carry the band (the ring itself is tiny there), which is why this reproduces today's
/// third-person feel while fixing every magnified view.
const SEPARATION_FADE_LOW_RING: f32 = 0.75;
const SEPARATION_FADE_HIGH_RING: f32 = 1.60;
/// Floors for degenerate rings (an unseeded predictor, a hairline settled circle) so the band
/// can never collapse to zero width and pin a marker on screen.
const SEPARATION_FADE_LOW_FLOOR: f32 = 0.010;
const SEPARATION_FADE_HIGH_FLOOR: f32 = 0.022;

/// Hairline floor: only a guard against a degenerate (zero) circle, never a size the ring is
/// pushed up to. At the third-person 62 degree view a settled 2.9 mrad gun is 0.0048 clip — 1.7
/// pixels at 720p — so the previous 0.008 floor drew that gun at 4.8 mrad, a 67% lie of exactly
/// the kind the old 0.025 floor was deleted for. The circle is angular truth or it is nothing.
const RING_MIN_RADIUS: f32 = 0.0035;

/// A continuous circle outline: short segments between consecutive points, not floating dots.
/// The visible size IS the server's dispersion, projected through the actual view — so it is a
/// SNIPER instrument by arithmetic: 2.9 mrad is 15 px at the 8 degree step and 40 px at maximum
/// zoom, against 1.7 px in third person. What third person reads from it is bloom (10 px at the
/// 17 mrad ceiling) and the settled brightening, not fine convergence. A dark underlay on BOTH
/// sides keeps the hairline readable on bright straw and dark shade alike.
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
    let radius = radius.clamp(RING_MIN_RADIUS, 0.35);
    let color = if converged { RETICLE_RING_CONVERGED } else { RETICLE_RING };
    // Underlay first: a dark twin on EACH side, so the hairline is outlined against whatever it
    // crosses. One outer twin left the inner edge bare — on pale straw the circle lost its lower
    // half against the ground it was drawn over.
    let segments = arc_segments(radius);
    for offset in [0.0018, -0.0018] {
        let twin = radius + offset;
        if twin > 0.0 {
            push_arc(
                vertices,
                center,
                twin,
                0.0,
                std::f32::consts::TAU,
                segments,
                aspect,
                RETICLE_RING_OUTLINE,
            );
        }
    }
    push_arc(vertices, center, radius, 0.0, std::f32::consts::TAU, segments, aspect, color);
}

/// The remaining reload as a RED arc that DRAINS clockwise from the top: full circle right after
/// firing, gone the instant the gun is ready. Red is the state, not an alarm — the trigger does
/// nothing while this draws, and [`push_ready_ring`] closes the same line when it does.
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
    let segments =
        (remaining * arc_segments(gun_state_radius(ring_radius)) as f32).ceil().max(2.0) as u32;
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

/// A refused fire click: one heavy red ring that SLAMS INWARD across the gun's own line — the
/// visual twin of the UiReject knock, so a swallowed shot is SEEN as refused, never wondered
/// about.
///
/// It has to survive being drawn over a red loading arc, because "still reloading" is the
/// refusal a player meets most. Colour cannot carry that (both are red, and denial-red is the
/// right red), so MOTION does: it starts well outside the arc, crosses it, and collapses past
/// it to the marker in a third of a second, at double stroke. A pulse that merely sat on the
/// arc's radius read as the arc briefly thickening.
pub(super) fn push_denied_flash(
    vertices: &mut Vec<renderer_api::HudVertex>,
    center: [f32; 2],
    ring_radius: f32,
    age_s: f32,
    aspect: f32,
) {
    const DENIED_FLASH_S: f32 = 0.32;
    if !(0.0..DENIED_FLASH_S).contains(&age_s) {
        return;
    }
    let t = age_s / DENIED_FLASH_S;
    // Sweep from outside the gun's own line to well inside it.
    let line = gun_state_radius(ring_radius);
    let radius = line * (1.9 - t * 1.55);
    let alpha = (1.0 - t) * 0.9;
    let color = [0.92, 0.28, 0.22, alpha];
    // Two concentric strokes: the arc primitive draws one hairline, and a hairline is what the
    // eye loses against the loading arc.
    for stroke in [0.0, 0.0035] {
        let radius = (radius + stroke).max(0.002);
        push_arc(
            vertices,
            center,
            radius,
            0.0,
            std::f32::consts::TAU,
            arc_segments(radius),
            aspect,
            color,
        );
    }
}

/// The loaded gun: the drained arc CLOSES into one full circle on the same line, holds, then
/// dissolves into silence. No expansion, no second glyph — the state simply finished, and the
/// whole engagement rhythm times itself against that closing.
///
/// The no-expansion rule is the durable half of an older decision: this ring replaced an
/// expanding blue FLASH, which existed only because "ready" had no colour of its own (a loaded
/// gun drew nothing), so the moment needed an event glyph to be seen at all. Once the arc's own
/// line carries the state, the event is the state finishing, and no second glyph is needed. The
/// hue moved on separately (see [`RETICLE_LOADED`]) — what mattered about that decision was the
/// glyph, and the glyph is unchanged.
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
    let radius = gun_state_radius(ring_radius);
    push_arc(
        vertices,
        center,
        radius,
        0.0,
        std::f32::consts::TAU,
        arc_segments(radius),
        aspect,
        color,
    );
}

/// The central marker: four arms around an OPEN centre, plus the aim dot the caller adds.
///
/// The gap is where the aiming circle lives. Through the 62 degree third-person view the ring
/// runs from 0.0035 clip settled to 0.028 at the 17 mrad bloom ceiling, while solid arms used to
/// run from the centre out to 0.020 — ink straight through the whole useful range of the one
/// glyph that reports the gun. Now the arms start outside it, and bloom crosses them only in its
/// top third, where the circle is already 4-9 px across and impossible to miss.
pub(super) fn push_crosshair(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    inner: f32,
    reach: f32,
    thick: f32,
    aspect: f32,
    color: [f32; 4],
) {
    // Dark backing first, a hair larger on every side: the marker has to hold on pale straw the
    // same way the ring does. The ring got its outline and the glyph the player actually aims
    // with did not.
    push_arms(vertices, center, inner, reach, thick + MARKER_OUTLINE, aspect, RETICLE_RING_OUTLINE);
    push_arms(vertices, center, inner, reach, thick, aspect, color);
}

/// Half-thickness added to the marker's dark backing on each side.
const MARKER_OUTLINE: f32 = 0.0014;

fn push_arms(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    inner: f32,
    reach: f32,
    thick: f32,
    aspect: f32,
    color: [f32; 4],
) {
    let (inner, reach) = (inner.max(0.0), reach.max(inner));
    let half = (reach - inner) * 0.5;
    let mid = inner + half;
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0_f32)] {
        push_quad(
            vertices,
            [center[0] + dx * mid / aspect, center[1] + dy * mid],
            [(half * dx.abs() + thick * dy.abs()) / aspect, half * dy.abs() + thick * dx.abs()],
            color,
        );
    }
}

/// The BLOCKED form: the SAME four arms, blown outward off the aim point and greyed, with no dot
/// left in the middle — the marker has come apart. It must stay unmistakable against the live
/// crosshair, which now also has an open centre (the ring lives there); reading them apart is
/// the distance the arms have flown, so this form starts outside where that one ends.
pub(super) fn push_blocked_marker(vertices: &mut Vec<HudVertex>, center: [f32; 2], aspect: f32) {
    push_arms(
        vertices,
        center,
        0.020,
        0.036,
        0.0028 + MARKER_OUTLINE,
        aspect,
        RETICLE_RING_OUTLINE,
    );
    push_arms(vertices, center, 0.020, 0.036, 0.0028, aspect, RETICLE_BLOCKED);
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

/// A hairline amber leader from the crosshair to the real impact, so a refusal reads as one
/// statement instead of two marks. It stops short at both ends: at the aim it clears the marker
/// and the dispersion ring, and at the impact it stops before the X, so neither glyph is
/// overdrawn by the line that points at it.
pub(super) fn push_impact_leader(
    vertices: &mut Vec<HudVertex>,
    aim_clip: [f32; 2],
    impact_clip: [f32; 2],
    alpha: f32,
) {
    // Dimmer than the X it points at: the answer is the mark, this only carries the eye there.
    let mut color = RETICLE_IMPACT;
    color[3] *= alpha * 0.45;
    let span = [impact_clip[0] - aim_clip[0], impact_clip[1] - aim_clip[1]];
    let length = (span[0] * span[0] + span[1] * span[1]).sqrt();
    // Below this the X is already inside the marker's own space and the line would be a smudge.
    const CLEAR_OF_MARKER: f32 = 0.040;
    const CLEAR_OF_X: f32 = 0.022;
    if length <= CLEAR_OF_MARKER + CLEAR_OF_X {
        return;
    }
    let unit = [span[0] / length, span[1] / length];
    push_segment(
        vertices,
        [aim_clip[0] + unit[0] * CLEAR_OF_MARKER, aim_clip[1] + unit[1] * CLEAR_OF_MARKER],
        [impact_clip[0] - unit[0] * CLEAR_OF_X, impact_clip[1] - unit[1] * CLEAR_OF_X],
        0.0012,
        color,
    );
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
