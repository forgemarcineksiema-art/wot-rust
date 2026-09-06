//! The garage overlay's seams: the screen as vertices, and the cursor as a hit — both off the
//! one draw list `screen.rs` builds (interface program G1, G12: the tree included).

use renderer_api::HudVertex;
#[cfg(test)]
use ui_kit::draw_list::DrawList;
use ui_kit::theme::Theme;
use ui_kit::ui::Ui;

#[cfg(test)]
use super::elements::GarageElement;
use super::{GarageHit, GarageState, screen};

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

#[cfg(test)]
mod tests {
    use super::*;
    use ui_kit::draw_list::Payload;

    /// G12: no garage screen carries a legacy element any more — the hangar and the tree are
    /// the toolkit's, and the tree's nodes are elements of their own.
    #[test]
    fn no_garage_screen_carries_a_legacy_element() {
        let mut state = GarageState::default();
        state.open();
        let ui = state.ui();
        let theme = Theme::standard();
        for tree in [false, true] {
            if tree {
                state.open_tech_tree();
            }
            let list = build_list(&state, &ui, &theme);
            assert!(
                list.iter().all(|e| !matches!(e.payload, Payload::Legacy(_))),
                "the screen is the toolkit's"
            );
            assert!(list.find(GarageElement::TechTree).is_none());
            assert!(list.find(GarageElement::Hover).is_none());
            assert_eq!(list.find(GarageElement::TreeNode(0)).is_some(), tree);
        }
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
