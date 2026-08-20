//! Bottom loadout strip: six module slots (click cycles the option) then the gun's ammo slots
//! (click selects the loaded round; the − / + zones under each slot edit how many rounds of it
//! fill the rack, within the vehicle's authored capacity). Swappable modules read bright;
//! single-option slots read dim. Each module slot also shows a one-line key stat below the icon
//! so the player sees what's installed.

use renderer_api::HudVertex;

use crate::app::garage::GarageState;
use crate::app::garage::draft::FitSlot;
use crate::app::garage::layout::*;
use crate::hud::font::{push_icon, push_text, text_width};
use crate::hud::push_panel;

pub(in crate::app::garage) fn draw(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    push_panel(v, [-0.03, LOADOUT_Y], [0.52, SLOT_HALF[1] + 0.02], CHAMFER_PANEL, aspect, PANEL);

    for (i, slot) in FitSlot::ALL.into_iter().enumerate() {
        let c = module_slot_center(i);
        let bg = if state.rejected_slot() == Some(slot) {
            REJECTED
        } else if state.focused_slot() == slot {
            SLOT_FOCUSED
        } else {
            SLOT
        };
        push_panel(v, c, SLOT_HALF, CHAMFER_SLOT, aspect, bg);
        let tint = if state.draft().has_choice(slot) { ICON } else { ICON_DIM };
        push_icon(v, slot_icon(slot), c[0] - 0.03, c[1] + 0.03, 0.06, aspect, tint);

        let summary = state.draft().current_module_summary(slot);
        let w = text_width(&summary, 0.022, aspect);
        push_text(v, &summary, c[0] - w / 2.0, c[1] - 0.028, 0.022, aspect, TEXT_DIM);
    }

    let selected = state.draft().ammo_index();
    let options = state.draft().ammo_options();
    let counts = state.draft().ammo_counts();
    for (i, shell) in options.iter().enumerate() {
        let c = ammo_slot_center(i);
        push_panel(
            v,
            c,
            SLOT_HALF,
            CHAMFER_SLOT,
            aspect,
            if i == selected { SLOT_SELECTED } else { SLOT },
        );
        push_icon(v, ammo_icon(shell.shell_type), c[0] - 0.03, c[1] + 0.03, 0.06, aspect, ICON);
        // The slot names WHICH round it chambers (v47): "BR-412D", the round's own designation,
        // not a class label. A legacy shell with no identity shows nothing — the icon carries it.
        if let Some(round) = shell.round {
            let name = round.designation();
            let w = text_width(name, 0.016, aspect);
            push_text(v, name, c[0] - w / 2.0, c[1] - 0.002, 0.016, aspect, TEXT_DIM);
        }

        // The count row: "−  N  +". The zones are real controls (they hit-test before the slot),
        // so they read as such — bright glyphs flanking the dimmer count.
        let count = format!("{}", counts.get(i).copied().unwrap_or(0));
        let w = text_width(&count, 0.024, aspect);
        push_text(v, &count, c[0] - w / 2.0, c[1] - 0.024, 0.024, aspect, TEXT_DIM);
        let (minus, plus) = ammo_adjust_centers(i);
        let glyph_w = text_width("-", 0.026, aspect);
        push_text(v, "-", minus[0] - glyph_w / 2.0, minus[1] + 0.013, 0.026, aspect, ICON);
        let glyph_w = text_width("+", 0.026, aspect);
        push_text(v, "+", plus[0] - glyph_w / 2.0, plus[1] + 0.013, 0.026, aspect, ICON);
    }

    // The rack budget readout: how full the rack is against the vehicle's authored capacity.
    let total = format!("{}/{}", state.draft().rack_total(), state.draft().rack_capacity());
    let tc = ammo_total_center();
    let w = text_width(&total, 0.026, aspect);
    push_text(v, &total, tc[0] - w / 2.0, tc[1] + 0.013, 0.026, aspect, VALUE);
}
