//! Top bar: the big red Battle button centred up top plus a (cosmetic) screen-tab row.

use renderer_api::HudVertex;

use crate::app::garage::layout::*;
use crate::hud::push_quad;
use crate::hud_font::{push_text, text_width};

pub(in crate::app::garage) fn draw(v: &mut Vec<HudVertex>, aspect: f32) {
    push_quad(v, [0.0, 0.93], [1.0, 0.07], PANEL);

    push_quad(v, BATTLE_CENTER, BATTLE_HALF, BATTLE);
    let w = text_width("BITWA", 0.05, aspect);
    push_text(v, "BITWA", BATTLE_CENTER[0] - w / 2.0, BATTLE_CENTER[1] + 0.025, 0.05, aspect, TEXT);

    // Screen tabs are cosmetic for now (only the garage exists); GARAGE reads as the active one.
    let mut x = -0.46;
    for (i, label) in TABS.iter().enumerate() {
        let color = if i == 0 { TEXT } else { TEXT_DIM };
        push_text(v, label, x, 0.815, 0.034, aspect, color);
        x += text_width(label, 0.034, aspect) + 0.03;
    }
}
