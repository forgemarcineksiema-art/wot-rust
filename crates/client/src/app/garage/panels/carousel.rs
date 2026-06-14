//! Bottom vehicle carousel: a horizontal strip of owned tanks; click selects (resets the draft).

use game_core::VehicleKind;
use renderer_api::HudVertex;

use crate::app::garage::GarageState;
use crate::app::garage::layout::*;
use crate::hud::push_quad;
use crate::hud_font::push_text;

pub(in crate::app::garage) fn draw(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    let count = VehicleKind::PLAYABLE.len();
    push_quad(v, [0.0, CAR_Y], [count as f32 * 0.065 + 0.02, CAR_HALF[1] + 0.02], PANEL);

    for (i, kind) in VehicleKind::PLAYABLE.into_iter().enumerate() {
        let c = carousel_center(i, count);
        let selected = i == state.selected_index();
        push_quad(v, c, CAR_HALF, if selected { SLOT_SELECTED } else { SLOT });
        let text_color = if selected { TEXT } else { TEXT_DIM };
        push_text(
            v,
            short_name(kind),
            c[0] - CAR_HALF[0] + 0.01,
            c[1] + 0.045,
            0.03,
            aspect,
            text_color,
        );
        push_text(
            v,
            &format!("{}", i + 1),
            c[0] - CAR_HALF[0] + 0.01,
            c[1] - 0.005,
            0.026,
            aspect,
            TEXT_DIM,
        );
    }
}
