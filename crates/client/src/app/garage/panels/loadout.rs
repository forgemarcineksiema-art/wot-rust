//! Bottom loadout strip: six module slots (click cycles the option) then the gun's ammo slots
//! (click selects). Swappable modules read bright; single-option slots read dim. Each module
//! slot also shows a one-line key stat below the icon so the player sees what's installed.

use renderer_api::HudVertex;

use crate::app::garage::GarageState;
use crate::app::garage::draft::FitSlot;
use crate::app::garage::layout::*;
use crate::hud::push_quad;
use crate::hud_font::{push_icon, push_text, text_width};

pub(in crate::app::garage) fn draw(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    push_quad(v, [-0.03, LOADOUT_Y], [0.52, SLOT_HALF[1] + 0.02], PANEL);

    for (i, slot) in FitSlot::ALL.into_iter().enumerate() {
        let c = module_slot_center(i);
        let bg = if state.rejected_slot() == Some(slot) {
            REJECTED
        } else if state.focused_slot() == slot {
            SLOT_FOCUSED
        } else {
            SLOT
        };
        push_quad(v, c, SLOT_HALF, bg);
        let tint = if state.draft().has_choice(slot) { ICON } else { ICON_DIM };
        push_icon(v, slot_icon(slot), c[0] - 0.03, c[1] + 0.03, 0.06, aspect, tint);

        let summary = state.draft().current_module_summary(slot);
        let w = text_width(&summary, 0.022, aspect);
        push_text(v, &summary, c[0] - w / 2.0, c[1] - 0.028, 0.022, aspect, TEXT_DIM);
    }

    let selected = state.draft().ammo_index();
    for i in 0..state.draft().ammo_options().len() {
        let c = ammo_slot_center(i);
        push_quad(v, c, SLOT_HALF, if i == selected { SLOT_SELECTED } else { SLOT });
        push_icon(v, ammo_icon(i), c[0] - 0.03, c[1] + 0.03, 0.06, aspect, ICON);
    }
}
