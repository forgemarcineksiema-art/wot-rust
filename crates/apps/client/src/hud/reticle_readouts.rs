//! Text and pulse readouts anchored to the reticle (split from `reticle_overlay.rs` for the
//! reviewability budget): the target distance, the sniper-only pen/armor millimeters, and the
//! landed-hit confirm ticks.

use renderer_api::HudVertex;

use super::reticle::PenetrationHint;
use super::reticle_overlay::{RETICLE_NO_PEN, RETICLE_PEN, push_segment};

/// A just-landed own hit, echoed at the reticle as a brief four-tick pulse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HitConfirm {
    pub age_s: f32,
    pub penetrated: bool,
    pub ricocheted: bool,
}

/// Seconds the hit-confirm ticks stay on screen.
pub(crate) const HIT_CONFIRM_TTL_S: f32 = 0.45;

/// The landed-hit echo: four diagonal ticks flare around the marker and fade over
/// [`HIT_CONFIRM_TTL_S`]. Color carries the result — bright green pen, amber ricochet, gray
/// bounce — so the shooter reads the outcome at the reticle before any floating number.
pub(super) fn push_hit_confirm(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    confirm: HitConfirm,
    aspect: f32,
) {
    let life = 1.0 - (confirm.age_s / HIT_CONFIRM_TTL_S).clamp(0.0, 1.0);
    if life <= 0.0 {
        return;
    }
    let mut color = if confirm.penetrated {
        [0.45, 1.0, 0.50, 0.95]
    } else if confirm.ricocheted {
        [0.98, 0.72, 0.25, 0.95]
    } else {
        [0.75, 0.72, 0.68, 0.95]
    };
    color[3] *= life;
    // Ticks drift slightly outward as they fade — a flare, not a static stamp.
    let (inner, outer) = (0.030 + 0.010 * (1.0 - life), 0.046 + 0.010 * (1.0 - life));
    for (sx, sy) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0_f32)] {
        push_segment(
            vertices,
            [center[0] + inner * sx / aspect, center[1] + inner * sy],
            [center[0] + outer * sx / aspect, center[1] + outer * sy],
            0.0026,
            color,
        );
    }
}

/// Sniper-only mm readout under the distance: the shell penetration (verdict color) against
/// the effective armor under the marker (dim red).
pub(super) fn push_pen_numbers(
    vertices: &mut Vec<HudVertex>,
    aim_clip: [f32; 2],
    hint: PenetrationHint,
    aspect: f32,
) {
    let right_x = (aim_clip[0] + 0.18).clamp(-0.88, 0.96);
    let top_y = (aim_clip[1] - 0.115).clamp(-0.75, 0.85);
    let pen_color = if hint.penetrates { RETICLE_PEN } else { RETICLE_NO_PEN };
    crate::hud::number::push_number(
        vertices,
        hint.shell_pen_mm.round().clamp(0.0, 9_999.0) as u32,
        right_x,
        top_y,
        0.038,
        aspect,
        pen_color,
    );
    crate::hud::font::push_text(
        vertices,
        "/",
        right_x + 0.004,
        top_y,
        0.038,
        aspect,
        crate::hud::number::UNIT_COLOR,
    );
    crate::hud::number::push_number(
        vertices,
        hint.armor_mm.round().clamp(0.0, 9_999.0) as u32,
        right_x + 0.058,
        top_y,
        0.038,
        aspect,
        [0.85, 0.42, 0.38, 0.80],
    );
}

pub(super) fn push_target_distance(
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
        crate::ui_strings::battle::DISTANCE_UNIT,
        right_x + 0.006,
        top_y,
        0.05,
        aspect,
        crate::hud::number::UNIT_COLOR,
    );
}
