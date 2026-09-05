//! The battle readouts: health and reload bars, the sniper zoom label, the FPS diagnostic and
//! the speed readout. Split from `hud.rs` (model + assembly order) for the reviewability budget.

use renderer_api::HudVertex;

use super::BattleHudModel;

/// The alert orange of the final-minute readouts (the p95 frame time) — the same as a running reload.
pub(crate) const CLOCK_CLOSING_COLOR: [f32; 4] = [0.86, 0.55, 0.20, 0.95];
/// The dev-build badge's own tone — deliberately NOT the clock's alert orange, so the clock
/// tests (which count their colour) never see the badge and vice versa.
const DEV_BADGE_COLOR: [f32; 4] = [0.90, 0.30, 0.25, 0.9];

pub(crate) fn push_battle_readouts(
    vertices: &mut Vec<HudVertex>,
    model: &BattleHudModel,
    aspect: f32,
) {
    // The hit points live in the damage panel (H4, `damage_panel.rs`) — the bar and the number
    // that floated top-left are its first row.

    // The reload lives at the reticle alone (arc + seconds): the old bottom-center bar drew a
    // SECOND loading indicator that split the eye between two progress displays for one gun.

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

    // The battle clock lives in the top bar now (H1, `top_bar.rs`) — under glass, in the
    // middle, with the frags and the pools beside it.

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
    // F9: the p95 frame interval under the FPS counter — "it drops sometimes" as a number.
    // Green territory is <= 20 ms; the tone flips to the alert orange when the worst frames
    // stretch past 25 ms (a visible stutter at 60 Hz).
    if model.frame_p95_ms > 0.0 {
        let label = format!("{:.0} ms", model.frame_p95_ms);
        let color = if model.frame_p95_ms > 25.0 {
            CLOCK_CLOSING_COLOR
        } else {
            crate::hud::number::UNIT_COLOR
        };
        let width = crate::hud::font::text_width(&label, 0.032, aspect);
        crate::hud::font::push_text(vertices, &label, 0.97 - width, 0.935, 0.032, aspect, color);
    }
    // A dev build announces itself next to the FPS counter (Płynność 2.0 / F8): every
    // performance verdict ever given on a debug binary was measured against the wrong game —
    // the profile is deliberately slower (opt-level 1) for compile speed. Unmissable, in the
    // alert tone, only in non-release builds.
    if cfg!(debug_assertions) {
        let label = "DEV BUILD - uzyj --release";
        let width = crate::hud::font::text_width(label, 0.035, aspect);
        crate::hud::font::push_text(
            vertices,
            label,
            0.97 - width,
            0.90,
            0.035,
            aspect,
            DEV_BADGE_COLOR,
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
