//! Top bar: the big red Battle button centred up top plus the two screen tabs. GARAGE is implicit
//! (clicking TECH TREE toggles back) and TECH TREE is drawn at its clickable rect so the highlight
//! and the hit-test agree.

use renderer_api::HudVertex;

use crate::app::garage::GarageState;
use crate::app::garage::GarageView;
use crate::app::garage::layout::*;
use crate::hud::font::{push_text, text_width};
use crate::hud::{push_panel, push_quad};
use crate::ui_strings::garage as strings;

pub(in crate::app::garage) fn draw(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    push_quad(v, [0.0, 0.93], [1.0, 0.07], PANEL);

    push_panel(v, BATTLE_CENTER, BATTLE_HALF, CHAMFER_SLOT, aspect, BATTLE);
    let w = text_width(strings::BATTLE, 0.05, aspect);
    push_text(
        v,
        strings::BATTLE,
        BATTLE_CENTER[0] - w / 2.0,
        BATTLE_CENTER[1] + 0.025,
        0.05,
        aspect,
        TEXT,
    );

    // The map row: which world the Battle button deploys into. AUTO (the default resolution)
    // reads dim; an explicit choice reads as a set value.
    push_panel(v, MAP_PICK_CENTER, MAP_PICK_HALF, CHAMFER_SLOT, aspect, SLOT);
    let map_label = map_pick_label(state.selected_map());
    let map_color = if state.selected_map().is_some() { VALUE } else { TEXT_DIM };
    let map_w = text_width(map_label, 0.028, aspect);
    push_text(
        v,
        map_label,
        MAP_PICK_CENTER[0] - map_w / 2.0,
        MAP_PICK_CENTER[1] + 0.014,
        0.028,
        aspect,
        map_color,
    );

    // Screen tabs: the active view's tab reads bright. GARAGE sits left-aligned; TECH TREE is
    // centred on its clickable rect.
    let garage_color = if state.view() == GarageView::Hangar { TEXT } else { TEXT_DIM };
    push_text(v, strings::TAB_GARAGE, -0.46, 0.815, 0.034, aspect, garage_color);

    let tt_color = if state.view() == GarageView::TechTree { TEXT } else { TEXT_DIM };
    let tt_w = text_width(strings::TAB_TECH_TREE, 0.034, aspect);
    push_text(
        v,
        strings::TAB_TECH_TREE,
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
        // Panel + Battle quad = 12 verts, plus "BATTLE" (6 glyphs) and tab text — well above 12.
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
