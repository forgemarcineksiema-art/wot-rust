//! The browse-only tech tree view: nation groups, line columns, tier rows (higher tier
//! higher) — the World of Tanks tree, not an era band. Only [`VehicleKind::PLAYABLE`] nodes
//! are drawn; there is no reserved empty band and no ghost predecessor. Clicking a node
//! selects that vehicle and returns to the hangar. There is no research or economy.

use game_core::{VehicleKind, tier_roman};
use renderer_api::HudVertex;

use crate::app::garage::GarageHit;
use crate::app::garage::GarageState;
use crate::app::garage::layout::{
    BATTLE, PANEL, SLOT, SLOT_SELECTED, TEXT, TEXT_DIM, TREE_CLOSE_CENTER, TREE_CLOSE_HALF,
    TREE_LINE_LABEL_Y, TREE_NATION_LABEL_Y, TREE_PANEL_CENTER, TREE_PANEL_HALF, VALUE, in_rect,
    tree_col_x, tree_columns, tree_node_center, tree_node_half,
};
use crate::hud::font::{push_text, text_width};
use crate::hud::push_panel;
use crate::hud::theme::{CHAMFER_PANEL, CHAMFER_SLOT};

/// One entry in the tree layout: (index into `VehicleKind::PLAYABLE`, the kind, its node centre,
/// its node half-extents).
fn tree_nodes() -> Vec<(usize, VehicleKind, [f32; 2], [f32; 2])> {
    let half = tree_node_half();
    VehicleKind::PLAYABLE
        .into_iter()
        .enumerate()
        .map(|(index, kind)| (index, kind, tree_node_center(kind), half))
        .collect()
}

/// The tree-node rect (centre, half) for a `VehicleKind::PLAYABLE` index, or `None` if out of
/// range. Reuses the same `tree_nodes()` enumeration the draw and hit-test paths use, so a hover
/// highlight lands on the exact node the click would select (the hangar carousel and the tech
/// tree lay nodes out differently, so the hover rect must be resolved per view).
pub(in crate::app::garage) fn node_rect_for_index(index: usize) -> Option<([f32; 2], [f32; 2])> {
    tree_nodes().into_iter().find(|(i, ..)| *i == index).map(|(_, _, center, half)| (center, half))
}

pub(in crate::app::garage) fn draw(state: &GarageState, aspect: f32) -> Vec<HudVertex> {
    let mut v = Vec::new();
    push_panel(&mut v, TREE_PANEL_CENTER, TREE_PANEL_HALF, CHAMFER_PANEL, aspect, PANEL);

    let cols = tree_columns();
    // Nation headers span their occupied line columns.
    let mut col = 0;
    while col < cols.len() {
        let nation = cols[col].0;
        let mut end = col + 1;
        while end < cols.len() && cols[end].0 == nation {
            end += 1;
        }
        let x0 = tree_col_x(col);
        let x1 = tree_col_x(end - 1);
        let tag = nation.label();
        let color = nation.color();
        let w = text_width(tag, 0.028, aspect);
        push_text(
            &mut v,
            tag,
            (x0 + x1) * 0.5 - w / 2.0,
            TREE_NATION_LABEL_Y,
            0.028,
            aspect,
            [color[0], color[1], color[2], 0.95],
        );
        col = end;
    }

    for (index, &(_, class)) in cols.iter().enumerate() {
        let tag = class.label();
        let w = text_width(tag, 0.018, aspect);
        push_text(
            &mut v,
            tag,
            tree_col_x(index) - w / 2.0,
            TREE_LINE_LABEL_Y,
            0.018,
            aspect,
            TEXT_DIM,
        );
    }

    let selected = state.selected_index();
    for (i, kind, center, half) in tree_nodes() {
        let bg = if i == selected { SLOT_SELECTED } else { SLOT };
        push_panel(&mut v, center, half, CHAMFER_SLOT, aspect, bg);
        let roman = tier_roman(kind.tier());
        let w = text_width(roman, 0.018, aspect);
        push_text(&mut v, roman, center[0] - w / 2.0, center[1] + 0.046, 0.018, aspect, VALUE);
        let name = kind.short_name();
        let mut size = 0.028;
        let max_w = 2.0 * half[0] - 0.02;
        let w = text_width(name, size, aspect);
        if w > max_w {
            size *= max_w / w;
        }
        let w = text_width(name, size, aspect);
        push_text(&mut v, name, center[0] - w / 2.0, center[1] + 0.012, size, aspect, TEXT);
    }

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

    for (i, _kind, center, half) in tree_nodes() {
        if in_rect(p, center, half) {
            return GarageHit::Vehicle(i);
        }
    }

    GarageHit::Scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::garage::layout::tree_tier_y;
    use game_core::{Nation, VehicleClass};

    fn at(garage: &mut GarageState, point: [f32; 2]) -> GarageHit {
        garage.open_tech_tree();
        garage.set_cursor(point);
        hit_test(garage)
    }

    fn playable_index(kind: VehicleKind) -> usize {
        VehicleKind::PLAYABLE.iter().position(|k| *k == kind).expect("playable")
    }

    #[test]
    fn techtree_draws_nation_columns_and_vehicle_nodes() {
        let mut state = GarageState::default();
        state.open_tech_tree();
        let v = draw(&state, 16.0 / 9.0);
        assert!(v.len() > 48, "tech tree must emit text and node vertices, got {}", v.len());
    }

    #[test]
    fn every_node_sits_on_its_nation_line_and_tier() {
        for (_, kind, center, _) in tree_nodes() {
            assert_eq!(center[1], tree_tier_y(kind.tier()), "{kind:?} must sit on its tier row");
            let cols = tree_columns();
            let col = cols
                .iter()
                .position(|&(nation, class)| nation == kind.nation() && class == kind.class())
                .expect("column");
            assert!(
                (center[0] - tree_col_x(col)).abs() < 1.0e-5,
                "{kind:?} must sit in its nation/line column"
            );
        }
        // Higher tier sits higher on the panel.
        assert!(tree_tier_y(9) > tree_tier_y(8));
        assert!(tree_tier_y(8) > tree_tier_y(7));
        assert!(tree_tier_y(7) > tree_tier_y(6));
    }

    #[test]
    fn the_tree_has_no_reserved_empty_band() {
        // No reserved empty band — every drawn vehicle node is a playable tank.
        assert_eq!(tree_nodes().len(), VehicleKind::PLAYABLE.len());
    }

    #[test]
    fn techtree_hit_test_returns_close_for_close_button() {
        let mut g = GarageState::default();
        assert_eq!(at(&mut g, TREE_CLOSE_CENTER), GarageHit::CloseTechTree);
    }

    #[test]
    fn techtree_hit_test_returns_vehicle_for_node_click() {
        let mut g = GarageState::default();
        assert_eq!(
            at(&mut g, tree_node_center(VehicleKind::TigerI)),
            GarageHit::Vehicle(playable_index(VehicleKind::TigerI))
        );
        assert_eq!(
            at(&mut g, tree_node_center(VehicleKind::T54_1951)),
            GarageHit::Vehicle(playable_index(VehicleKind::T54_1951))
        );
        assert_eq!(
            at(&mut g, tree_node_center(VehicleKind::IS3)),
            GarageHit::Vehicle(playable_index(VehicleKind::IS3))
        );
    }

    #[test]
    fn techtree_hit_test_returns_scene_for_empty_space() {
        let mut g = GarageState::default();
        assert_eq!(at(&mut g, [0.0, -0.4]), GarageHit::Scene);
    }

    #[test]
    fn techtree_draws_every_playable_vehicle_as_a_node() {
        let mut state = GarageState::default();
        state.open_tech_tree();
        let v = draw(&state, 16.0 / 9.0);
        assert!(
            v.len() >= 300,
            "tech tree must draw all {} playable vehicles as nodes, got {} vertices",
            VehicleKind::PLAYABLE.len(),
            v.len()
        );
    }

    #[test]
    fn occupied_lines_are_nation_then_class() {
        assert_eq!(
            tree_columns(),
            [
                (Nation::Ussr, VehicleClass::Medium),
                (Nation::Ussr, VehicleClass::Heavy),
                (Nation::Germany, VehicleClass::Medium),
                (Nation::Germany, VehicleClass::Heavy),
                (Nation::Germany, VehicleClass::TankDestroyer),
                (Nation::Britain, VehicleClass::Medium),
            ]
        );
    }
}
