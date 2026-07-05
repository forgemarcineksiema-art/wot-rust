//! The battle readouts: health and reload bars, the sniper zoom label, the FPS diagnostic and
//! the speed readout. Split from `hud.rs` (model + assembly order) for the reviewability budget.

use renderer_api::HudVertex;

use super::primitives::push_bar;
use super::{BattleHudModel, health_color};

/// Battle-clock color for the final minute — the same alert orange as a running reload.
pub(crate) const CLOCK_CLOSING_COLOR: [f32; 4] = [0.86, 0.55, 0.20, 0.95];

pub(crate) fn push_battle_readouts(
    vertices: &mut Vec<HudVertex>,
    model: &BattleHudModel,
    aspect: f32,
) {
    let vitals = model.vitals;
    let hp_frac = (vitals.hit_points as f32 / vitals.max_hit_points.max(1) as f32).clamp(0.0, 1.0);
    push_bar(vertices, [-0.95, 0.9], [0.17, 0.018], hp_frac, health_color(hp_frac));
    crate::hud::number::push_number(
        vertices,
        vitals.hit_points.min(9_999),
        -0.61,
        0.95,
        0.055,
        aspect,
        crate::hud::number::HP_COLOR,
    );

    let ready =
        (1.0 - vitals.reload_remaining_s / vitals.reload_seconds.max(0.001)).clamp(0.0, 1.0);
    let reload_color =
        if ready >= 1.0 { [0.55, 0.85, 0.96, 0.95] } else { [0.86, 0.55, 0.20, 0.92] };
    push_bar(vertices, [-0.16, -0.9], [0.16, 0.016], ready, reload_color);
    crate::hud::number::push_number(
        vertices,
        vitals.reload_remaining_s.ceil().clamp(0.0, 99.0) as u32,
        0.06,
        -0.76,
        0.065,
        aspect,
        crate::hud::number::RELOAD_TIME_COLOR,
    );

    // Sniper magnification readout, WT-style "X6.9", just under the reticle center so the
    // eye reads it without leaving the sight. Third person draws nothing.
    if let Some(zoom) = model.zoom_factor {
        let label =
            format!("{}{:.1}", crate::ui_strings::battle::ZOOM_PREFIX, zoom.clamp(0.0, 99.9));
        crate::hud::font::push_text(
            vertices,
            &label,
            -0.03,
            -0.16,
            0.05,
            aspect,
            crate::hud::number::ZOOM_COLOR,
        );
    }

    // Battle clock, top-center M:SS. Steady readout most of the match; the last minute warms to
    // the alert orange so the closing squeeze reads at a glance.
    if let Some(remaining_s) = model.battle_clock_remaining_s {
        let total = remaining_s.max(0.0).ceil() as u32;
        let label = format!("{}:{:02}", total / 60, total % 60);
        let color = if total <= 60 { CLOCK_CLOSING_COLOR } else { crate::hud::number::UNIT_COLOR };
        let width = crate::hud::font::text_width(&label, 0.055, aspect);
        crate::hud::font::push_text(vertices, &label, -width * 0.5, 0.93, 0.055, aspect, color);
    }

    if model.fps > 0.0 {
        crate::hud::number::push_number(
            vertices,
            model.fps.round() as u32,
            0.97,
            0.97,
            0.05,
            aspect,
            crate::hud::number::FPS_COLOR,
        );
    }

    if model.speed_kmh >= 0.5 {
        crate::hud::number::push_number(
            vertices,
            model.speed_kmh.round().clamp(0.0, 999.0) as u32,
            -0.78,
            -0.76,
            0.065,
            aspect,
            crate::hud::number::SPEED_COLOR,
        );
        // Unit sits just right of the value's anchor, dimmer and a touch smaller for hierarchy.
        crate::hud::font::push_text(
            vertices,
            crate::ui_strings::battle::SPEED_UNIT,
            -0.765,
            -0.764,
            0.045,
            aspect,
            crate::hud::number::UNIT_COLOR,
        );
    }
}
