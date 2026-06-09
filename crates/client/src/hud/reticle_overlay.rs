use renderer_api::HudVertex;

use super::push_quad;
use crate::reticle::{PenetrationHint, ReticleStatus};

pub(crate) const RETICLE_CLEAR: [f32; 4] = [0.86, 0.90, 0.72, 0.64];
pub(crate) const RETICLE_GUN: [f32; 4] = [0.35, 0.80, 1.00, 0.90];
pub(crate) const RETICLE_AIM_CIRCLE: [f32; 4] = [0.86, 0.90, 0.72, 0.30];
/// Amber "X" at the shell's real landing point (gravity + collision), distinct from the
/// plus-shaped aim and gun crosshairs.
pub(crate) const RETICLE_IMPACT: [f32; 4] = [0.98, 0.66, 0.18, 0.92];
const RETICLE_PEN: [f32; 4] = [0.35, 0.85, 0.40, 0.85];
const RETICLE_NO_PEN: [f32; 4] = [0.90, 0.30, 0.25, 0.85];

/// Below this screen-space gap from the aim point the real impact sits inside the crosshair, so
/// drawing it would just clutter the center; above it the drop/collision is worth showing.
const IMPACT_SEPARATION_CLIP: f32 = 0.022;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HudReticle {
    pub aim_clip: [f32; 2],
    pub gun_clip: Option<[f32; 2]>,
    /// Where the shell actually lands (full ballistic + collision trace), if on screen.
    pub impact_clip: Option<[f32; 2]>,
    pub aim_radius_clip: f32,
    pub target_distance_m: Option<f32>,
    pub status: ReticleStatus,
    pub penetration_hint: Option<PenetrationHint>,
}

/// Draw the full aiming overlay: dispersion circle, aim and gun crosshairs, the real-impact
/// marker, target distance, and the penetration indicator.
pub(crate) fn push_reticle(vertices: &mut Vec<HudVertex>, reticle: &HudReticle, aspect: f32) {
    let reticle_color = reticle
        .penetration_hint
        .map_or(RETICLE_CLEAR, |hint| if hint.penetrates { RETICLE_PEN } else { RETICLE_NO_PEN });
    push_aiming_circle(vertices, reticle.aim_clip, reticle.aim_radius_clip, aspect);
    push_crosshair(vertices, reticle.aim_clip, 0.022, 0.0026, aspect, reticle_color);
    if let Some(gun_clip) = reticle.gun_clip {
        push_crosshair(vertices, gun_clip, 0.018, 0.0025, aspect, RETICLE_GUN);
    }
    // Where the shell really lands. Suppressed when it sits inside the crosshair (high muzzle
    // velocity, no drop) and only worth a marker once gravity/collision pull it off the aim point.
    if let Some(impact_clip) = reticle.impact_clip
        && impact_is_separated(reticle.aim_clip, impact_clip, aspect)
    {
        push_impact_marker(vertices, impact_clip, aspect);
    }
    if let Some(distance_m) = reticle.target_distance_m {
        push_target_distance(vertices, reticle.aim_clip, distance_m, aspect);
    }
    if let Some(hint) = reticle.penetration_hint {
        push_pen_indicator(vertices, reticle.aim_clip, hint, aspect);
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
    crate::hud_number::push_number(
        vertices,
        distance_m.round().clamp(0.0, 9_999.0) as u32,
        right_x,
        top_y,
        0.05,
        aspect,
        crate::hud_number::TARGET_DISTANCE_COLOR,
    );
    crate::hud_font::push_text(
        vertices,
        "M",
        right_x + 0.006,
        top_y,
        0.05,
        aspect,
        crate::hud_number::UNIT_COLOR,
    );
}

fn push_aiming_circle(vertices: &mut Vec<HudVertex>, center: [f32; 2], radius: f32, aspect: f32) {
    if radius <= 0.0 {
        return;
    }
    let radius = radius.clamp(0.025, 0.25);
    for i in 0..16 {
        let angle = i as f32 * std::f32::consts::TAU / 16.0;
        let point = [center[0] + angle.cos() * radius / aspect, center[1] + angle.sin() * radius];
        push_quad(vertices, point, [0.0025 / aspect, 0.0025], RETICLE_AIM_CIRCLE);
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

/// Whether the impact point is far enough from the aim point to be worth its own marker.
fn impact_is_separated(aim_clip: [f32; 2], impact_clip: [f32; 2], aspect: f32) -> bool {
    // Compare in screen space (clip x is squished by `aspect`) so the gap reads uniformly.
    let dx = (impact_clip[0] - aim_clip[0]) * aspect;
    let dy = impact_clip[1] - aim_clip[1];
    (dx * dx + dy * dy).sqrt() > IMPACT_SEPARATION_CLIP
}

/// A small amber "X" marking where the shell actually lands.
fn push_impact_marker(vertices: &mut Vec<HudVertex>, center: [f32; 2], aspect: f32) {
    let reach_x = 0.016 / aspect;
    let reach_y = 0.016;
    let half_thick = 0.0028;
    push_segment(
        vertices,
        [center[0] - reach_x, center[1] - reach_y],
        [center[0] + reach_x, center[1] + reach_y],
        half_thick,
        RETICLE_IMPACT,
    );
    push_segment(
        vertices,
        [center[0] - reach_x, center[1] + reach_y],
        [center[0] + reach_x, center[1] - reach_y],
        half_thick,
        RETICLE_IMPACT,
    );
}

/// A thin quad between two clip-space points (the diagonal arms of the impact "X").
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

fn push_pen_indicator(
    vertices: &mut Vec<HudVertex>,
    aim_clip: [f32; 2],
    hint: PenetrationHint,
    aspect: f32,
) {
    let (cx, cy) = (aim_clip[0] + 0.018, aim_clip[1] + 0.018);
    let hw = 0.025 / aspect;
    push_quad(vertices, [cx, cy], [hw, 0.003], [0.0, 0.0, 0.0, 0.55]);
    let max = hint.shell_pen_mm.max(hint.armor_mm).max(1.0);
    let aw = hw * (hint.armor_mm / max).clamp(0.0, 1.0);
    push_quad(vertices, [cx - hw + aw, cy], [aw, 0.003], [0.90, 0.26, 0.22, 0.85]);
    let pw = hw * (hint.shell_pen_mm / max).clamp(0.0, 1.0);
    if pw > aw {
        let ow = pw - aw;
        push_quad(vertices, [cx - hw + aw + ow, cy], [ow, 0.003], [0.30, 0.82, 0.34, 0.85]);
    }
}
