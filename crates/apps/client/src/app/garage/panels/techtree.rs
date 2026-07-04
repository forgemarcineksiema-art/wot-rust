//! The browse-only tech tree view: vehicles grouped by nation in vertical columns (the beta-WoT
//! signature flourish). Clicking a node selects that vehicle and returns to the hangar. There is
//! no research or economy — every playable vehicle is selectable.

use game_core::{Nation, VehicleKind};
use renderer_api::HudVertex;

use crate::app::garage::GarageHit;
use crate::app::garage::GarageState;
use crate::app::garage::layout::{
    BATTLE, PANEL, SLOT, SLOT_SELECTED, TEXT, TREE_CLOSE_CENTER, TREE_CLOSE_HALF, TREE_HEADER_Y,
    TREE_NODE_HALF, TREE_PANEL_CENTER, TREE_PANEL_HALF, in_rect, short_name, tree_column_x,
    tree_node_center,
};
use crate::hud::font::{push_text, text_width};
use crate::hud::push_panel;
use crate::hud::theme::{CHAMFER_PANEL, CHAMFER_SLOT};

/// One entry in the tree layout: (index into `VehicleKind::PLAYABLE`, the kind, its node centre).
fn tree_nodes() -> Vec<(usize, VehicleKind, [f32; 2])> {
    let mut out = Vec::new();
    let mut ussr_row = 0;
    let mut germany_row = 0;
    for (i, kind) in VehicleKind::PLAYABLE.into_iter().enumerate() {
        let nation = kind.nation();
        let row = match nation {
            Nation::Ussr => {
                let r = ussr_row;
                ussr_row += 1;
                r
            }
            Nation::Germany => {
                let r = germany_row;
                germany_row += 1;
                r
            }
        };
        out.push((i, kind, tree_node_center(nation, row)));
    }
    out
}

pub(in crate::app::garage) fn draw(state: &GarageState, aspect: f32) -> Vec<HudVertex> {
    let mut v = Vec::new();
    push_panel(&mut v, TREE_PANEL_CENTER, TREE_PANEL_HALF, CHAMFER_PANEL, aspect, PANEL);

    // Nation column headers, colored by nation.
    for nation in [Nation::Ussr, Nation::Germany] {
        let label = nation.label();
        let x = tree_column_x(nation);
        let color = nation.color();
        let w = text_width(label, 0.05, aspect);
        push_text(
            &mut v,
            label,
            x - w / 2.0,
            TREE_HEADER_Y,
            0.05,
            aspect,
            [color[0], color[1], color[2], 0.98],
        );
    }

    // Vehicle nodes in each nation's column.
    let selected = state.selected_index();
    for (i, kind, center) in tree_nodes() {
        let bg = if i == selected { SLOT_SELECTED } else { SLOT };
        push_panel(&mut v, center, TREE_NODE_HALF, CHAMFER_SLOT, aspect, bg);
        let name = short_name(kind);
        let w = text_width(name, 0.034, aspect);
        push_text(&mut v, name, center[0] - w / 2.0, center[1] + 0.022, 0.034, aspect, TEXT);
    }

    // Close button in the top-right corner.
    push_panel(&mut v, TREE_CLOSE_CENTER, TREE_CLOSE_HALF, CHAMFER_SLOT, aspect, BATTLE);
    let label = crate::ui_strings::garage::BACK;
    let w = text_width(label, 0.028, aspect);
    push_text(
        &mut v,
        label,
        TREE_CLOSE_CENTER[0] - w / 2.0,
        TREE_CLOSE_CENTER[1] + 0.018,
        0.028,
        aspect,
        TEXT,
    );

    v
}

pub(in crate::app::garage) fn hit_test(state: &GarageState) -> GarageHit {
    let p = state.cursor_clip();

    if in_rect(p, TREE_CLOSE_CENTER, TREE_CLOSE_HALF) {
        return GarageHit::CloseTechTree;
    }

    for (i, _kind, center) in tree_nodes() {
        if in_rect(p, center, TREE_NODE_HALF) {
            return GarageHit::Vehicle(i);
        }
    }

    GarageHit::Scene
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(garage: &mut GarageState, point: [f32; 2]) -> GarageHit {
        garage.open_tech_tree();
        garage.set_cursor(point);
        hit_test(garage)
    }

    #[test]
    fn techtree_draws_nation_columns_and_vehicle_nodes() {
        let mut state = GarageState::default();
        state.open_tech_tree();
        let aspect = 16.0 / 9.0;

        let v = draw(&state, aspect);

        // Panel + close button + 5 nodes = 7 quads = 42 vertices minimum, plus text glyphs.
        assert!(v.len() > 42, "tech tree must emit text and node vertices, got {}", v.len());
    }

    #[test]
    fn techtree_hit_test_returns_close_for_close_button() {
        let mut g = GarageState::default();
        assert_eq!(at(&mut g, TREE_CLOSE_CENTER), GarageHit::CloseTechTree);
    }

    #[test]
    fn techtree_hit_test_returns_vehicle_for_node_click() {
        let mut g = GarageState::default();
        // T-54 is the first USSR node (row 0).
        let t54_center = tree_node_center(Nation::Ussr, 0);
        assert_eq!(at(&mut g, t54_center), GarageHit::Vehicle(0));

        // Tiger I is the first Germany node (row 0); it is PLAYABLE index 1.
        let tiger_center = tree_node_center(Nation::Germany, 0);
        assert_eq!(at(&mut g, tiger_center), GarageHit::Vehicle(1));
    }

    #[test]
    fn techtree_hit_test_returns_scene_for_empty_space() {
        let mut g = GarageState::default();
        assert_eq!(at(&mut g, [0.0, 0.0]), GarageHit::Scene);
    }

    #[test]
    fn techtree_draws_every_playable_vehicle_as_a_node() {
        let mut state = GarageState::default();
        state.open_tech_tree();
        let aspect = 16.0 / 9.0;

        let v = draw(&state, aspect);

        // Each vehicle node is a quad (6 verts) + its short name (variable glyphs × 6 verts).
        // 5 vehicles × (6 + name glyphs × 6) must be present. The minimum name is "T-54" (4 glyphs)
        // so 5 × (6 + 24) = 150 vertices from nodes alone, plus panel/close/headers.
        assert!(
            v.len() >= 150,
            "tech tree must draw all {} playable vehicles as nodes, got {} vertices",
            VehicleKind::PLAYABLE.len(),
            v.len()
        );
    }
}
