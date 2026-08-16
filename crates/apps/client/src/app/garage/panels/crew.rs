//! Left crew column: the five roles, presentational. The shared proficiency dial is GONE
//! (Hala v4 W1, user decision 2026-08-14): proficiency is pinned to 1.0 in `game_core`, so
//! a control for it would be a knob wired to nothing — the roster stays because the crew is
//! part of the machine's story, not because it is a stat.

use renderer_api::HudVertex;

use crate::app::garage::GarageState;
use crate::app::garage::layout::*;
use crate::hud::font::{push_icon, push_text};
use crate::hud::icons::HudIcon;
use crate::hud::{push_hairline, push_panel};

pub(in crate::app::garage) fn draw(v: &mut Vec<HudVertex>, _state: &GarageState, aspect: f32) {
    push_panel(v, [CREW_X, 0.46], [CREW_HALF_X + 0.02, 0.34], CHAMFER_PANEL, aspect, PANEL);
    let left = CREW_X - CREW_HALF_X;
    push_text(v, crate::ui_strings::garage::CREW, left, 0.80, 0.04, aspect, TEXT_DIM);
    push_hairline(v, left, CREW_X + CREW_HALF_X, 0.755, HAIRLINE);

    for (i, role) in game_core::Crew::roles().into_iter().enumerate() {
        let y = CREW_TOP - i as f32 * CREW_PITCH;
        push_icon(v, HudIcon::Crew, left, y + 0.035, 0.06, aspect, ICON);
        push_text(v, role.label(), left + 0.075, y + 0.022, 0.038, aspect, TEXT);
    }
}
