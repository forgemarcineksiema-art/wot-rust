//! The aiming overlay, rebuilt around one rule: ONE marker owns the center. The old overlay
//! stacked three crosshairs (aim, gun, impact) plus a millimeter bar there; in a fight nobody
//! can parse four glyphs at the screen center. Now a single marker carries everything by color
//! and shape — neutral (no target), green (penetrates), red (bounces), and a broken red form
//! when the shot is BLOCKED (terrain/ally/arc) — inside a continuous dispersion ring, with the
//! reload draining as an arc where the eyes already are. The amber X still appears when the
//! real ballistic landing point separates from the aim (shell drop / collision): that is the
//! one moment a second glyph carries real information.

use renderer_api::HudVertex;

use super::push_quad;
use crate::hud::reticle::{PenetrationHint, ReticleStatus};

pub(crate) const RETICLE_NEUTRAL: [f32; 4] = [0.88, 0.90, 0.84, 0.85];
pub(crate) const RETICLE_PEN: [f32; 4] = [0.35, 0.85, 0.40, 0.92];
pub(crate) const RETICLE_NO_PEN: [f32; 4] = [0.90, 0.30, 0.25, 0.92];
/// The BLOCKED form: the shot will not reach the aim point (terrain, ally, gun arc).
pub(crate) const RETICLE_BLOCKED: [f32; 4] = [0.88, 0.28, 0.22, 0.95];
/// Continuous dispersion ring — quiet on purpose; it frames, the marker speaks.
pub(crate) const RETICLE_RING: [f32; 4] = [0.86, 0.90, 0.72, 0.38];
/// Amber "X" at the shell's real landing point (gravity + collision).
pub(crate) const RETICLE_IMPACT: [f32; 4] = [0.98, 0.66, 0.18, 0.92];
/// The reload arc draining clockwise around the marker while the gun is loading.
pub(crate) const RETICLE_RELOAD: [f32; 4] = [0.92, 0.62, 0.20, 0.88];

/// Fixed screen radius of the reload arc: inside any realistic dispersion ring, outside the
/// marker, independent of zoom so the eye always finds it in the same place.
const RELOAD_ARC_RADIUS: f32 = 0.055;

/// Below this screen-space gap the real impact sits inside the crosshair; above it the
/// drop/collision is worth its own marker.
const IMPACT_SEPARATION_CLIP: f32 = 0.022;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HudReticle {
    pub aim_clip: [f32; 2],
    /// Where the shell actually lands (full ballistic + collision trace), if on screen.
    pub impact_clip: Option<[f32; 2]>,
    pub aim_radius_clip: f32,
    pub target_distance_m: Option<f32>,
    pub status: ReticleStatus,
    pub penetration_hint: Option<PenetrationHint>,
    /// Reload progress in `[0, 1]` (1 = ready). Below 1 the reticle arc shows what remains.
    pub reload_fraction: f32,
}

/// Draw the aiming overlay: dispersion ring, reload arc, the single center marker, the
/// real-impact marker when separated, and the target distance.
pub(crate) fn push_reticle(vertices: &mut Vec<HudVertex>, reticle: &HudReticle, aspect: f32) {
    push_dispersion_ring(vertices, reticle.aim_clip, reticle.aim_radius_clip, aspect);
    push_reload_arc(vertices, reticle.aim_clip, reticle.reload_fraction, aspect);

    match reticle.status {
        ReticleStatus::Blocked => push_blocked_marker(vertices, reticle.aim_clip, aspect),
        ReticleStatus::Clear => {
            let color = reticle.penetration_hint.map_or(RETICLE_NEUTRAL, |hint| {
                if hint.penetrates { RETICLE_PEN } else { RETICLE_NO_PEN }
            });
            push_crosshair(vertices, reticle.aim_clip, 0.020, 0.0026, aspect, color);
            push_quad(vertices, reticle.aim_clip, [0.0016 / aspect, 0.0016], color);
        }
    }

    if let Some(impact_clip) = reticle.impact_clip
        && impact_is_separated(reticle.aim_clip, impact_clip, aspect)
    {
        push_impact_marker(vertices, impact_clip, aspect);
    }
    if let Some(distance_m) = reticle.target_distance_m {
        push_target_distance(vertices, reticle.aim_clip, distance_m, aspect);
    }
}

/// A continuous circle outline: short segments between consecutive points, not floating dots.
fn push_dispersion_ring(vertices: &mut Vec<HudVertex>, center: [f32; 2], radius: f32, aspect: f32) {
    if radius <= 0.0 {
        return;
    }
    let radius = radius.clamp(0.025, 0.25);
    push_arc(vertices, center, radius, 0.0, std::f32::consts::TAU, 40, aspect, RETICLE_RING);
}

/// The remaining reload as an arc that DRAINS clockwise from the top: full circle right after
/// firing, gone the instant the gun is ready. Nothing draws when loaded — a ready gun is silence.
fn push_reload_arc(vertices: &mut Vec<HudVertex>, center: [f32; 2], fraction: f32, aspect: f32) {
    let remaining = (1.0 - fraction).clamp(0.0, 1.0);
    if remaining <= 0.0 {
        return;
    }
    let sweep = remaining * std::f32::consts::TAU;
    let start = std::f32::consts::FRAC_PI_2 - sweep; // ends at 12 o'clock, drains clockwise
    let segments = (remaining * 32.0).ceil().max(2.0) as u32;
    push_arc(vertices, center, RELOAD_ARC_RADIUS, start, sweep, segments, aspect, RETICLE_RELOAD);
}

#[allow(clippy::too_many_arguments)]
fn push_arc(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    radius: f32,
    start_rad: f32,
    sweep_rad: f32,
    segments: u32,
    aspect: f32,
    color: [f32; 4],
) {
    let point =
        |angle: f32| [center[0] + angle.cos() * radius / aspect, center[1] + angle.sin() * radius];
    for index in 0..segments {
        let a = start_rad + sweep_rad * (index as f32 / segments as f32);
        let b = start_rad + sweep_rad * ((index + 1) as f32 / segments as f32);
        push_segment(vertices, point(a), point(b), 0.0018, color);
    }
}

fn push_crosshair(
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
/// "broken" at a glance, in warning red, with no penetration coloring to lie over it.
fn push_blocked_marker(vertices: &mut Vec<HudVertex>, center: [f32; 2], aspect: f32) {
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

/// Whether the impact point is far enough from the aim point to be worth its own marker.
fn impact_is_separated(aim_clip: [f32; 2], impact_clip: [f32; 2], aspect: f32) -> bool {
    let dx = (impact_clip[0] - aim_clip[0]) * aspect;
    let dy = impact_clip[1] - aim_clip[1];
    (dx * dx + dy * dy).sqrt() > IMPACT_SEPARATION_CLIP
}

/// A small amber "X" marking where the shell actually lands.
fn push_impact_marker(vertices: &mut Vec<HudVertex>, center: [f32; 2], aspect: f32) {
    let reach_x = 0.016 / aspect;
    let reach_y = 0.016;
    for (sx, sy) in [(1.0, 1.0), (1.0, -1.0_f32)] {
        push_segment(
            vertices,
            [center[0] - reach_x * sx, center[1] - reach_y * sy],
            [center[0] + reach_x * sx, center[1] + reach_y * sy],
            0.0028,
            RETICLE_IMPACT,
        );
    }
}

fn push_target_distance(
    vertices: &mut Vec<HudVertex>,
    aim_clip: [f32; 2],
    distance_m: f32,
    aspect: f32,
) {
    let right_x = (aim_clip[0] + 0.18).clamp(-0.88, 0.96);
    let top_y = (aim_clip[1] - 0.055).clamp(-0.70, 0.90);
    crate::hud::number::push_number(
        vertices,
        distance_m.round().clamp(0.0, 9_999.0) as u32,
        right_x,
        top_y,
        0.05,
        aspect,
        crate::hud::number::TARGET_DISTANCE_COLOR,
    );
    crate::hud::font::push_text(
        vertices,
        "M",
        right_x + 0.006,
        top_y,
        0.05,
        aspect,
        crate::hud::number::UNIT_COLOR,
    );
}

/// A thin quad between two clip-space points (arc segments, impact "X" arms).
fn push_segment(
    vertices: &mut Vec<HudVertex>,
    a: [f32; 2],
    b: [f32; 2],
    half_thick: f32,
    color: [f32; 4],
) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt().max(1.0e-6);
    let (nx, ny) = (-dy / len * half_thick, dx / len * half_thick);
    let corners = [
        [a[0] + nx, a[1] + ny],
        [b[0] + nx, b[1] + ny],
        [b[0] - nx, b[1] - ny],
        [a[0] + nx, a[1] + ny],
        [b[0] - nx, b[1] - ny],
        [a[0] - nx, a[1] - ny],
    ];
    for position in corners {
        vertices.push(HudVertex::new(position, color));
    }
}
