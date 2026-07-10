//! The browse-only tech tree view, organised along the design's PRIMARY axis: era. Three
//! horizontal bands read chronologically top to bottom — Era I (1939-42, deliberately empty: the
//! reserved roadmap bracket explains the axis), Era II late-war metal, Era III early Cold War —
//! and each band lines its playable vehicles up as nodes tagged with their nation. A battle is
//! one era, never a mix; this screen is where the player learns that. Clicking a node selects
//! that vehicle and returns to the hangar. There is no research or economy — every playable
//! vehicle is selectable.

use game_core::{Era, VehicleKind};
use renderer_api::HudVertex;

use crate::app::garage::GarageHit;
use crate::app::garage::GarageState;
use crate::app::garage::layout::{
    BATTLE, PANEL, SLOT, SLOT_SELECTED, TEXT, TEXT_DIM, TREE_CLOSE_CENTER, TREE_CLOSE_HALF,
    TREE_ERA_LABEL_X, TREE_NODE_HALF, TREE_PANEL_CENTER, TREE_PANEL_HALF, VALUE, in_rect,
    short_name, tree_era_y, tree_node_center,
};
use crate::hud::font::{push_text, text_width};
use crate::hud::push_panel;
use crate::hud::theme::{CHAMFER_PANEL, CHAMFER_SLOT};

/// One entry in the tree layout: (index into `VehicleKind::PLAYABLE`, the kind, its node centre).
/// Vehicles line up left-to-right inside their ERA's band, in roster order.
fn tree_nodes() -> Vec<(usize, VehicleKind, [f32; 2])> {
    let mut out = Vec::new();
    for era in Era::ALL {
        for (col, kind) in era.playable().enumerate() {
            let index = VehicleKind::PLAYABLE
                .iter()
                .position(|k| *k == kind)
                .expect("era.playable() draws from PLAYABLE");
            out.push((index, kind, tree_node_center(era, col)));
        }
    }
    out
}

/// The tree-node centre for a `VehicleKind::PLAYABLE` index, or `None` if out of range. Reuses the
/// same `tree_nodes()` enumeration the draw and hit-test paths use, so a hover highlight lands on the
/// exact node the click would select (the hangar carousel and the tech tree lay nodes out
/// differently, so the hover rect must be resolved per view).
pub(in crate::app::garage) fn node_center_for_index(index: usize) -> Option<[f32; 2]> {
    tree_nodes().into_iter().find(|(i, _, _)| *i == index).map(|(_, _, center)| center)
}

pub(in crate::app::garage) fn draw(state: &GarageState, aspect: f32) -> Vec<HudVertex> {
    let mut v = Vec::new();
    push_panel(&mut v, TREE_PANEL_CENTER, TREE_PANEL_HALF, CHAMFER_PANEL, aspect, PANEL);

    // Era band labels down the left edge: the bracket tag big, the descriptive years under it.
    // An era with nothing fielded states so instead of hiding — the axis is the message.
    for era in Era::ALL {
        let y = tree_era_y(era);
        push_text(&mut v, era.label(), TREE_ERA_LABEL_X, y + 0.045, 0.042, aspect, VALUE);
        push_text(&mut v, era.display_name(), TREE_ERA_LABEL_X, y - 0.012, 0.02, aspect, TEXT_DIM);
        if era.playable().next().is_none() {
            let placeholder = crate::ui_strings::garage::ERA_RESERVED;
            push_text(
                &mut v,
                placeholder,
                tree_node_center(era, 0)[0] - TREE_NODE_HALF[0],
                y + 0.014,
                0.026,
                aspect,
                TEXT_DIM,
            );
        }
    }

    // Vehicle nodes inside their era band: a nation tag in the nation's colour over the name.
    let selected = state.selected_index();
    for (i, kind, center) in tree_nodes() {
        let bg = if i == selected { SLOT_SELECTED } else { SLOT };
        push_panel(&mut v, center, TREE_NODE_HALF, CHAMFER_SLOT, aspect, bg);
        let nation = kind.nation();
        let tag = nation.label();
        let color = nation.color();
        let w = text_width(tag, 0.02, aspect);
        push_text(
            &mut v,
            tag,
            center[0] - w / 2.0,
            center[1] + 0.046,
            0.02,
            aspect,
            [color[0], color[1], color[2], 0.95],
        );
        let name = short_name(kind);
        let w = text_width(name, 0.032, aspect);
        push_text(&mut v, name, center[0] - w / 2.0, center[1] + 0.012, 0.032, aspect, TEXT);
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
    fn techtree_draws_era_bands_and_vehicle_nodes() {
        let mut state = GarageState::default();
        state.open_tech_tree();
        let aspect = 16.0 / 9.0;

        let v = draw(&state, aspect);

        // Panel + close button + 6 nodes = 8 quads = 48 vertices minimum, plus era labels,
        // nation tags and node names.
        assert!(v.len() > 48, "tech tree must emit text and node vertices, got {}", v.len());
    }

    #[test]
    fn every_node_sits_in_its_own_eras_band() {
        // THE phase-5 lock: era is the layout's primary axis. Every playable vehicle's node
        // centre-line is its era's band, not a nation column.
        for (_, kind, center) in tree_nodes() {
            assert_eq!(
                center[1],
                tree_era_y(kind.era()),
                "{kind:?} must sit in the {:?} band",
                kind.era()
            );
        }
        // And the bands are genuinely distinct rows, chronological top to bottom.
        assert!(tree_era_y(Era::EarlyWar) > tree_era_y(Era::LateWar));
        assert!(tree_era_y(Era::LateWar) > tree_era_y(Era::ColdWar));
    }

    #[test]
    fn the_empty_early_war_band_states_itself_instead_of_hiding() {
        let mut state = GarageState::default();
        state.open_tech_tree();
        let v = draw(&state, 16.0 / 9.0);

        // Era I has no playable vehicles, yet its band prints glyphs (label + years + the
        // reserved placeholder) — the axis is communicated, not collapsed.
        let band_y = tree_era_y(Era::EarlyWar);
        let band_glyphs = v
            .iter()
            .filter(|vert| (vert.position[1] - band_y).abs() < 0.08 && vert.uv[0] >= 0.0)
            .count();
        assert!(band_glyphs > 60, "the reserved era band must speak: {band_glyphs} glyph verts");
    }

    #[test]
    fn techtree_hit_test_returns_close_for_close_button() {
        let mut g = GarageState::default();
        assert_eq!(at(&mut g, TREE_CLOSE_CENTER), GarageHit::CloseTechTree);
    }

    #[test]
    fn techtree_hit_test_returns_vehicle_for_node_click() {
        let mut g = GarageState::default();
        // Tiger I is the first Era II node; it is PLAYABLE index 1.
        assert_eq!(at(&mut g, tree_node_center(Era::LateWar, 0)), GarageHit::Vehicle(1));
        // T-54 is the first Era III node (PLAYABLE index 0).
        assert_eq!(at(&mut g, tree_node_center(Era::ColdWar, 0)), GarageHit::Vehicle(0));
        // The IS-3 lines up beside it in the same band (PLAYABLE index 5).
        assert_eq!(at(&mut g, tree_node_center(Era::ColdWar, 1)), GarageHit::Vehicle(5));
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
        let aspect = 16.0 / 9.0;

        let v = draw(&state, aspect);

        // Each vehicle node is a quad (6 verts) + nation tag + short name (variable glyphs × 6).
        // 6 vehicles × (6 + ~4 name glyphs × 6 + ~4 tag glyphs × 6) ≈ 324 verts from nodes alone.
        assert!(
            v.len() >= 300,
            "tech tree must draw all {} playable vehicles as nodes, got {} vertices",
            VehicleKind::PLAYABLE.len(),
            v.len()
        );
    }
}
