//! The garage overlay's seams: the screen as vertices, and the cursor as a hit — both off the
//! one draw list `screen.rs` builds (interface program G1). The tree view still carries its
//! legacy vertices and their hover wash until G12.

use renderer_api::HudVertex;
#[cfg(test)]
use ui_kit::draw_list::DrawList;
use ui_kit::theme::Theme;
use ui_kit::ui::Ui;

#[cfg(test)]
use super::elements::GarageElement;
use super::layout::{TREE_CLOSE_CENTER, TREE_CLOSE_HALF, in_rect};
use super::{GarageHit, GarageState, GarageView, panels, screen};

impl GarageState {
    /// Length scale for the parked tank's gun submesh so swapping guns visibly changes the
    /// silhouette: the installed barrel over the vehicle's stock barrel (the baked mesh is stock).
    pub(in crate::app) fn gun_silhouette_scale(&self) -> f32 {
        let stock = self.selected_vehicle().stock_barrel_length_m();
        if stock <= 0.0 {
            return 1.0;
        }
        (self.draft().gun_barrel_length() / stock).clamp(0.6, 1.6)
    }
}

pub(super) fn build(state: &GarageState, ui: &Ui, theme: &Theme) -> Vec<HudVertex> {
    screen::build_screen(state, ui, theme).emit(ui, theme)
}

/// The screen as a draw list, the hovered control lit.
#[cfg(test)]
pub(super) fn build_list(state: &GarageState, ui: &Ui, theme: &Theme) -> DrawList<GarageElement> {
    screen::build_screen(state, ui, theme)
}

pub(super) fn hit_test(state: &GarageState, shift: bool) -> GarageHit {
    screen::hit_screen(state, &state.ui(), shift)
}

/// The tree's legacy hover: a node or BACK under the cursor, in clip space.
pub(super) fn tree_hover_rect(state: &GarageState) -> Option<([f32; 2], [f32; 2])> {
    if state.view() != GarageView::TechTree {
        return None;
    }
    let p = state.cursor_clip();
    if in_rect(p, TREE_CLOSE_CENTER, TREE_CLOSE_HALF) {
        return Some((TREE_CLOSE_CENTER, TREE_CLOSE_HALF));
    }
    match panels::techtree::hit_test(state) {
        GarageHit::Vehicle(index) => panels::techtree::node_rect_for_index(index),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::VehicleKind;
    use ui_kit::draw_list::Payload;

    /// The tree view carries its legacy vertices verbatim on one element and lights a hovered
    /// node with the old wash; the hangar carries no legacy element at all.
    #[test]
    fn the_tree_rides_its_legacy_element_and_the_hangar_none() {
        let mut state = GarageState::default();
        state.open();
        let ui = state.ui();
        let theme = Theme::standard();
        let hangar = build_list(&state, &ui, &theme);
        assert!(
            hangar.iter().all(|e| !matches!(e.payload, Payload::Legacy(_))),
            "the hangar is the toolkit's"
        );
        state.open_tech_tree();
        let tree = build_list(&state, &ui, &theme);
        assert!(tree.find(GarageElement::TechTree).is_some());
        assert!(tree.find(GarageElement::Hover).is_none(), "nothing hovered");
        state.set_cursor(super::super::layout::tree_node_center(VehicleKind::TigerI));
        let tree = build_list(&state, &ui, &theme);
        assert!(tree.find(GarageElement::Hover).is_some(), "the node under the cursor is lit");
        assert_eq!(
            hit_test(&state, false),
            GarageHit::Vehicle(
                VehicleKind::PLAYABLE
                    .iter()
                    .position(|k| *k == VehicleKind::TigerI)
                    .expect("playable")
            )
        );
        state.set_cursor(TREE_CLOSE_CENTER);
        assert_eq!(hit_test(&state, false), GarageHit::CloseTechTree);
        assert_eq!(tree_hover_rect(&state), Some((TREE_CLOSE_CENTER, TREE_CLOSE_HALF)));
    }

    /// The vertices are the list's emission — nothing is drawn beside the list.
    #[test]
    fn the_overlay_is_the_lists_emission() {
        let mut state = GarageState::default();
        state.open();
        let ui = state.ui();
        let theme = Theme::standard();
        assert_eq!(build(&state, &ui, &theme), build_list(&state, &ui, &theme).emit(&ui, &theme));
        assert!(!build(&state, &ui, &theme).is_empty());
    }
}
