//! Garage HUD orchestration and cursor hit-testing. Draws the WoT-style regions (top bar, left
//! crew, right stats, bottom loadout strip, bottom carousel) from [`super::panels`] and maps the
//! cursor to a [`GarageHit`] using the same rects from [`super::layout`]. A subtle bright quad is
//! drawn over the element under the cursor so clickable areas read as interactive.

use game_core::VehicleKind;
use renderer_api::HudVertex;

use super::draft::FitSlot;
use super::layout::*;
use super::{GarageHit, GarageState, GarageView, panels};
use crate::hud::push_quad;

pub(super) fn build(state: &GarageState, aspect: f32) -> Vec<HudVertex> {
    if !state.is_open() {
        return Vec::new();
    }
    match state.view() {
        GarageView::Hangar => build_hangar(state, aspect),
        GarageView::TechTree => build_tech_tree(state, aspect),
    }
}

fn build_hangar(state: &GarageState, aspect: f32) -> Vec<HudVertex> {
    let mut v = Vec::new();
    let spec = state.draft().assembled_spec();
    panels::topbar::draw(&mut v, state, aspect);
    panels::crew::draw(&mut v, state, aspect);
    panels::stats::draw(&mut v, &spec, aspect);
    panels::loadout::draw(&mut v, state, aspect);
    panels::carousel::draw(&mut v, state, aspect);

    if let Some((center, half)) = hover_rect(state, &state.hit_test(false)) {
        push_quad(&mut v, center, half, HOVER);
    }

    v
}

fn build_tech_tree(state: &GarageState, aspect: f32) -> Vec<HudVertex> {
    let mut v = Vec::new();
    panels::topbar::draw(&mut v, state, aspect);
    panels::techtree::draw(state, aspect).into_iter().for_each(|vertex| v.push(vertex));

    if let Some((center, half)) = hover_rect(state, &state.hit_test(false)) {
        push_quad(&mut v, center, half, HOVER);
    }

    v
}

/// Map the element under the cursor to its rect so a hover highlight can be drawn.
fn hover_rect(state: &GarageState, hit: &GarageHit) -> Option<([f32; 2], [f32; 2])> {
    match hit {
        GarageHit::Battle => Some((BATTLE_CENTER, BATTLE_HALF)),
        GarageHit::ModuleCycle(slot, _) => Some((module_slot_center(slot.index()), SLOT_HALF)),
        GarageHit::AmmoSelect(i) => Some((ammo_slot_center(*i), SLOT_HALF)),
        GarageHit::CrewProf(dir) => {
            let (l, r) = crew_prof_arrows();
            let center = if *dir < 0 { l } else { r };
            Some((center, ARROW_HALF))
        }
        GarageHit::Vehicle(i) => carousel_cell_rect(state, *i),
        GarageHit::CarouselScroll(dir) => {
            let (left, right) = carousel_arrows();
            Some((if *dir < 0 { left } else { right }, CAR_ARROW_HALF))
        }
        GarageHit::OpenTechTree | GarageHit::CloseTechTree | GarageHit::Scene => None,
    }
}

/// Screen rect of an absolute roster cell, if it currently sits inside the visible window.
fn carousel_cell_rect(state: &GarageState, absolute: usize) -> Option<([f32; 2], [f32; 2])> {
    let count = VehicleKind::PLAYABLE.len();
    let window = carousel_window(count, state.carousel_scroll());
    window
        .contains(&absolute)
        .then(|| (carousel_cell_center(absolute - window.start, window.len()), CAR_HALF))
}

pub(super) fn hit_test(state: &GarageState, shift: bool) -> GarageHit {
    let p = state.cursor_clip();

    // The TECH TREE tab toggles between views and is hit-testable in both.
    if in_rect(p, TECH_TREE_TAB_CENTER, TECH_TREE_TAB_HALF) {
        return match state.view() {
            GarageView::Hangar => GarageHit::OpenTechTree,
            GarageView::TechTree => GarageHit::CloseTechTree,
        };
    }

    match state.view() {
        GarageView::Hangar => hit_test_hangar(state, shift),
        GarageView::TechTree => panels::techtree::hit_test(state),
    }
}

fn hit_test_hangar(state: &GarageState, shift: bool) -> GarageHit {
    let p = state.cursor_clip();
    let cycle_dir = if shift { -1 } else { 1 };

    if in_rect(p, BATTLE_CENTER, BATTLE_HALF) {
        return GarageHit::Battle;
    }
    for (i, slot) in FitSlot::ALL.into_iter().enumerate() {
        if in_rect(p, module_slot_center(i), SLOT_HALF) {
            return GarageHit::ModuleCycle(slot, cycle_dir);
        }
    }
    for i in 0..state.draft().ammo_options().len() {
        if in_rect(p, ammo_slot_center(i), SLOT_HALF) {
            return GarageHit::AmmoSelect(i);
        }
    }
    let (left, right) = crew_prof_arrows();
    if in_rect(p, left, ARROW_HALF) {
        return GarageHit::CrewProf(-1);
    }
    if in_rect(p, right, ARROW_HALF) {
        return GarageHit::CrewProf(1);
    }
    let count = VehicleKind::PLAYABLE.len();
    if carousel_overflows(count) {
        let (left, right) = carousel_arrows();
        if in_rect(p, left, CAR_ARROW_HALF) {
            return GarageHit::CarouselScroll(-1);
        }
        if in_rect(p, right, CAR_ARROW_HALF) {
            return GarageHit::CarouselScroll(1);
        }
    }
    let window = carousel_window(count, state.carousel_scroll());
    for (slot, absolute) in window.clone().enumerate() {
        if in_rect(p, carousel_cell_center(slot, window.len()), CAR_HALF) {
            return GarageHit::Vehicle(absolute);
        }
    }
    GarageHit::Scene
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(garage: &mut GarageState, point: [f32; 2]) -> GarageHit {
        garage.set_cursor(point);
        garage.hit_test(false)
    }

    fn at_shift(garage: &mut GarageState, point: [f32; 2]) -> GarageHit {
        garage.set_cursor(point);
        garage.hit_test(true)
    }

    #[test]
    fn cursor_hits_battle_carousel_and_scene() {
        let mut g = GarageState::default();
        assert_eq!(at(&mut g, BATTLE_CENTER), GarageHit::Battle);
        // The roster fits (< CAR_VISIBLE), so absolute index == visible slot.
        assert_eq!(
            at(&mut g, carousel_cell_center(3, VehicleKind::PLAYABLE.len())),
            GarageHit::Vehicle(3)
        );
        assert_eq!(at(&mut g, [0.0, 0.0]), GarageHit::Scene);
    }

    #[test]
    fn clicking_a_module_slot_cycles_it_and_ammo_slot_selects() {
        let mut g = GarageState::default();
        g.select_vehicle(VehicleKind::T54_1951);
        // Gun is index 1 in FitSlot::ALL.
        assert_eq!(at(&mut g, module_slot_center(1)), GarageHit::ModuleCycle(FitSlot::Gun, 1));
        assert_eq!(at(&mut g, ammo_slot_center(1)), GarageHit::AmmoSelect(1));
    }

    #[test]
    fn shift_click_on_a_module_slot_cycles_backward() {
        let mut g = GarageState::default();
        g.select_vehicle(VehicleKind::T54_1951);
        assert_eq!(
            at_shift(&mut g, module_slot_center(1)),
            GarageHit::ModuleCycle(FitSlot::Gun, -1)
        );
        // Non-shift still cycles forward (regression guard).
        assert_eq!(at(&mut g, module_slot_center(1)), GarageHit::ModuleCycle(FitSlot::Gun, 1));
    }

    #[test]
    fn shift_does_not_change_ammo_battle_or_vehicle_hits() {
        let mut g = GarageState::default();
        g.select_vehicle(VehicleKind::T54_1951);
        assert_eq!(at_shift(&mut g, BATTLE_CENTER), GarageHit::Battle);
        assert_eq!(at_shift(&mut g, ammo_slot_center(1)), GarageHit::AmmoSelect(1));
        assert_eq!(
            at_shift(&mut g, carousel_cell_center(2, VehicleKind::PLAYABLE.len())),
            GarageHit::Vehicle(2)
        );
    }

    #[test]
    fn crew_proficiency_arrows_hit() {
        let mut g = GarageState::default();
        let (left, right) = crew_prof_arrows();
        assert_eq!(at(&mut g, right), GarageHit::CrewProf(1));
        assert_eq!(at(&mut g, left), GarageHit::CrewProf(-1));
    }

    #[test]
    fn hover_rect_returns_none_for_scene() {
        let mut g = GarageState::default();
        g.set_cursor([0.0, 0.0]);
        assert_eq!(hover_rect(&g, &g.hit_test(false)), None);
    }

    #[test]
    fn hover_rect_returns_the_battle_button_rect() {
        let mut g = GarageState::default();
        g.set_cursor(BATTLE_CENTER);
        assert_eq!(hover_rect(&g, &g.hit_test(false)), Some((BATTLE_CENTER, BATTLE_HALF)));
    }

    #[test]
    fn hover_rect_returns_a_module_slot_rect() {
        let mut g = GarageState::default();
        let center = module_slot_center(1);
        g.set_cursor(center);
        assert_eq!(hover_rect(&g, &g.hit_test(false)), Some((center, SLOT_HALF)));
    }

    #[test]
    fn build_emits_hover_quad_over_clickable_elements() {
        let mut g = GarageState::default();
        let aspect = 16.0 / 9.0;

        g.set_cursor([0.0, 0.0]);
        let baseline = build(&g, aspect).len();

        g.set_cursor(BATTLE_CENTER);
        let with_hover = build(&g, aspect).len();
        assert!(with_hover > baseline, "hovering a clickable element should emit extra vertices");
    }

    #[test]
    fn clicking_tech_tree_tab_in_hangar_opens_tech_tree() {
        let mut g = GarageState::default();
        assert_eq!(at(&mut g, TECH_TREE_TAB_CENTER), GarageHit::OpenTechTree);
    }

    #[test]
    fn clicking_tech_tree_tab_in_tech_tree_closes_it() {
        let mut g = GarageState::default();
        g.open_tech_tree();
        g.set_cursor(TECH_TREE_TAB_CENTER);
        assert_eq!(g.hit_test(false), GarageHit::CloseTechTree);
    }
}
