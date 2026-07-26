//! Top bar: the big red Battle button centred up top plus the two screen tabs. GARAGE is implicit
//! (clicking TECH TREE toggles back) and TECH TREE is drawn at its clickable rect so the highlight
//! and the hit-test agree.

use renderer_api::HudVertex;

use crate::app::garage::GarageState;
use crate::app::garage::GarageView;
use crate::app::garage::layout::*;
use crate::hud::font::{push_text, text_width};
use crate::hud::{push_hairline, push_panel, push_quad};
use crate::ui_strings::garage as strings;

pub(in crate::app::garage) fn draw(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    push_quad(v, TOP_BAR_CENTER, TOP_BAR_HALF, PANEL);
    push_hairline(v, -1.0, 1.0, TOP_BAR_BOTTOM, HAIRLINE);

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

    // Screen tabs: the active view's tab reads bright, and BOTH are drawn centred on the rect they
    // hit-test, so the highlight lands where the click does.
    let garage_color = if state.view() == GarageView::Hangar { TEXT } else { TEXT_DIM };
    push_tab(v, strings::TAB_GARAGE, GARAGE_TAB_CENTER, garage_color, aspect);

    let tt_color = if state.view() == GarageView::TechTree { TEXT } else { TEXT_DIM };
    push_tab(v, strings::TAB_TECH_TREE, TECH_TREE_TAB_CENTER, tt_color, aspect);
}

fn push_tab(v: &mut Vec<HudVertex>, label: &str, center: [f32; 2], color: [f32; 4], aspect: f32) {
    let w = text_width(label, 0.034, aspect);
    push_text(v, label, center[0] - w / 2.0, center[1], 0.034, aspect, color);
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

    /// Regression, and the reason this bar was rebuilt: the plate stopped at y=0.86 while the tab
    /// row hit-tested 0.785..0.845, so GARAGE and TECH TREE rendered as dim grey text directly on
    /// the hangar wall. Every control the bar owns must sit on the bar.
    #[test]
    fn the_top_bar_plate_carries_every_control_that_sits_on_it() {
        let top = TOP_BAR_CENTER[1] + TOP_BAR_HALF[1];
        let bottom = TOP_BAR_CENTER[1] - TOP_BAR_HALF[1];
        for (name, center, half) in [
            ("battle", BATTLE_CENTER, BATTLE_HALF),
            ("map row", MAP_PICK_CENTER, MAP_PICK_HALF),
            ("garage tab", GARAGE_TAB_CENTER, GARAGE_TAB_HALF),
            ("tech tree tab", TECH_TREE_TAB_CENTER, TECH_TREE_TAB_HALF),
        ] {
            assert!(
                center[1] - half[1] >= bottom && center[1] + half[1] <= top,
                "{name} hangs off the top bar ({}..{} outside {bottom}..{top})",
                center[1] - half[1],
                center[1] + half[1]
            );
            assert!(
                center[0] - half[0] >= -1.0 && center[0] + half[0] <= 1.0,
                "{name} runs off the screen"
            );
        }
    }

    /// The two tabs must not overlap, or one of them eats the other's clicks. Compile-time: the
    /// layout is all consts, so an overlap should fail the build, not a test run.
    #[test]
    fn the_two_tabs_do_not_overlap() {
        const {
            assert!(
                GARAGE_TAB_CENTER[0] + GARAGE_TAB_HALF[0]
                    < TECH_TREE_TAB_CENTER[0] - TECH_TREE_TAB_HALF[0],
                "the GARAGE and TECH TREE tabs overlap"
            )
        };
    }

    /// The nameplate sits under the bar, not behind it. The bar grew downward to carry its tabs;
    /// this is the guard on how far down it may grow.
    #[test]
    fn the_bar_clears_the_vehicle_nameplate() {
        use crate::app::garage::panels::nameplate::{NAMEPLATE_CENTER, NAMEPLATE_HALF};
        const {
            assert!(
                NAMEPLATE_CENTER[1] + NAMEPLATE_HALF[1] < TOP_BAR_CENTER[1] - TOP_BAR_HALF[1],
                "the top bar now overlaps the nameplate"
            )
        };
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
