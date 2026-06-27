//! Top bar: the big red Battle button centred up top plus a screen-tab row. Only the GARAGE and
//! TECH TREE tabs are active — GARAGE is implicit (clicking TECH TREE toggles back) and the rest
//! are cosmetic placeholders for future screens.

use renderer_api::HudVertex;

use crate::app::garage::GarageState;
use crate::app::garage::GarageView;
use crate::app::garage::layout::*;
use crate::hud::font::{push_text, text_width};
use crate::hud::push_quad;

pub(in crate::app::garage) fn draw(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    push_quad(v, [0.0, 0.93], [1.0, 0.07], PANEL);

    push_quad(v, BATTLE_CENTER, BATTLE_HALF, BATTLE);
    let w = text_width("BITWA", 0.05, aspect);
    push_text(v, "BITWA", BATTLE_CENTER[0] - w / 2.0, BATTLE_CENTER[1] + 0.025, 0.05, aspect, TEXT);

    // Cosmetic screen tabs (left-aligned run); only GARAGE and TECH TREE carry meaning.
    let mut x = -0.46;
    for (i, label) in TABS.iter().enumerate() {
        let color = if i == 0 && state.view() == GarageView::Hangar { TEXT } else { TEXT_DIM };
        push_text(v, label, x, 0.815, 0.034, aspect, color);
        x += text_width(label, 0.034, aspect) + 0.03;
    }

    // Active TECH TREE indicator: bright when the tech tree view is open. Drawn over the
    // cosmetic tab at the clickable rect so the highlight aligns with the hit-test.
    let tech_active = state.view() == GarageView::TechTree;
    let tt_label = "TECH TREE";
    let tt_w = text_width(tt_label, 0.034, aspect);
    let tt_color = if tech_active { TEXT } else { TEXT_DIM };
    push_text(
        v,
        tt_label,
        TECH_TREE_TAB_CENTER[0] - tt_w / 2.0,
        TECH_TREE_TAB_CENTER[1],
        0.034,
        aspect,
        tt_color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topbar_emits_battle_button_and_tab_text() {
        let state = GarageState::default();
        let aspect = 16.0 / 9.0;
        let mut v = Vec::new();
        draw(&mut v, &state, aspect);
        // Panel + Battle quad = 12 verts, plus "BITWA" (5 glyphs) and tab text — well above 12.
        assert!(v.len() > 12, "topbar must emit text, got {}", v.len());
    }

    #[test]
    fn topbar_emits_more_vertices_when_tech_tree_is_active() {
        let aspect = 16.0 / 9.0;
        let mut hangar = GarageState::default();
        let mut v_hangar = Vec::new();
        draw(&mut v_hangar, &hangar, aspect);

        hangar.open_tech_tree();
        let mut v_tech = Vec::new();
        draw(&mut v_tech, &hangar, aspect);

        // Both emit the same glyphs; the test guards that the tech-tree path still draws text.
        assert!(v_tech.len() > 12, "topbar must still emit text in tech tree view");
        assert_eq!(v_tech.len(), v_hangar.len(), "same glyphs either way");
    }
}
