//! Text and pulse readouts anchored to the reticle (split from `reticle_overlay.rs` for the
//! reviewability budget): the target distance, the sniper-only pen/armor millimeters, and the
//! landed-hit confirm ticks.

use renderer_api::HudVertex;

use super::primitives::push_segment;
use super::reticle::PenetrationHint;
use super::reticle_overlay::{RETICLE_NO_PEN, RETICLE_PEN};

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

/// Where the readout column stands: to the right of the aim and below it, clear of the aiming
/// circle whatever it is doing.
///
/// The offsets used to be fixed at 0.18/0.055. Two ways that failed: a bloomed ring reaches 0.35
/// and simply swallowed the numbers, and the fixed spot sat at clip radius 0.325 — right on the
/// 0.30 ring where incoming-hit arcs draw, so the range and the "you are being shot from there"
/// arc overprinted each other. The column now starts closer in (radius ~0.20, comfortably inside
/// the hit ring) and steps outward only as far as the circle forces it.
fn readout_anchor(
    aim_clip: [f32; 2],
    ring_radius: f32,
    row: f32,
    value_width: f32,
    aspect: f32,
) -> [f32; 2] {
    // The numbers are RIGHT-aligned on this anchor, so their own width has to be part of the
    // clearance or the leftmost digit is the one the circle swallows.
    let clear_of_ring = (ring_radius + 0.02) / aspect.max(0.01) + value_width;
    // 0.135 puts the column at clip radius ~0.25: clear of the marker, and a comfortable 20 px
    // inside the 0.30 ring the incoming-hit arcs draw on.
    let x = (aim_clip[0] + (0.135f32).max(clear_of_ring)).clamp(-0.88, 0.96);
    let y = (aim_clip[1] - row.max(ring_radius + row * 0.3)).clamp(-0.75, 0.85);
    [x, y]
}

/// Width of an `n`-digit value at `height`, measured through the real font metrics without
/// formatting a string every frame.
fn digits_width(value: u32, height: f32, aspect: f32) -> f32 {
    const RULER: [&str; 5] = ["0", "0", "00", "000", "0000"];
    let digits = crate::hud::number::digit_count(value).min(4) as usize;
    crate::hud::font::text_width(RULER[digits], height, aspect)
}

/// Sniper-only mm readout under the distance: the shell penetration (verdict color) against
/// the effective armor under the marker (dim red).
pub(super) fn push_pen_numbers(
    vertices: &mut Vec<HudVertex>,
    aim_clip: [f32; 2],
    ring_radius: f32,
    hint: PenetrationHint,
    aspect: f32,
) {
    let pen_mm = hint.shell_pen_mm.round().clamp(0.0, 9_999.0) as u32;
    let [right_x, top_y] =
        readout_anchor(aim_clip, ring_radius, 0.105, digits_width(pen_mm, 0.038, aspect), aspect);
    let pen_color = if hint.penetrates { RETICLE_PEN } else { RETICLE_NO_PEN };
    crate::hud::number::push_number(vertices, pen_mm, right_x, top_y, 0.038, aspect, pen_color);
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

/// Metres to whatever eats the round, drawn under the range in the broken marker's own grey.
///
/// The row is the one the penetration millimetres use, and the two can never collide: a shot that
/// does not arrive reaches no armour, so there are no millimetres to print. Sharing the row is
/// deliberate rather than convenient — this line and that one answer the same question ("what
/// happens at the other end of this shot"), and giving the refusal its own third row would push
/// the column into the incoming-hit arcs.
///
/// It is a smaller, dimmer number than the range on purpose: the range is what the player asked
/// for, this is why they cannot have it.
pub(super) fn push_block_distance(
    vertices: &mut Vec<HudVertex>,
    aim_clip: [f32; 2],
    ring_radius: f32,
    distance_m: f32,
    aspect: f32,
) {
    const HEIGHT: f32 = 0.038;
    let metres = distance_m.round().clamp(0.0, 9_999.0) as u32;
    let [right_x, top_y] =
        readout_anchor(aim_clip, ring_radius, 0.105, digits_width(metres, HEIGHT, aspect), aspect);
    crate::hud::number::push_number(
        vertices,
        metres,
        right_x,
        top_y,
        HEIGHT,
        aspect,
        crate::hud::reticle_overlay::RETICLE_BLOCKED,
    );
    crate::hud::font::push_text(
        vertices,
        crate::ui_strings::battle::DISTANCE_UNIT,
        right_x + 0.006,
        top_y,
        HEIGHT,
        aspect,
        crate::hud::number::UNIT_COLOR,
    );
}

pub(super) fn push_target_distance(
    vertices: &mut Vec<HudVertex>,
    aim_clip: [f32; 2],
    ring_radius: f32,
    distance_m: f32,
    aspect: f32,
) {
    let metres = distance_m.round().clamp(0.0, 9_999.0) as u32;
    let [right_x, top_y] =
        readout_anchor(aim_clip, ring_radius, 0.05, digits_width(metres, 0.05, aspect), aspect);
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
