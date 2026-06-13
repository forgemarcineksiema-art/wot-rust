//! Garage HUD orchestration: the left tech-tree list (vehicles + the current tank's modules), the
//! fitting tabs, and cursor hit-testing. The right-hand panels live in [`super::panels`] and all
//! geometry/palette in [`super::layout`], so drawing and hit-testing share one source of truth.

use game_core::VehicleKind;
use renderer_api::HudVertex;

use super::draft::FitSlot;
use super::layout::*;
use super::{GarageHit, GarageState, GarageTab, panels};
use crate::hud::push_quad;
use crate::hud_font::{push_text, text_width};

pub(super) fn build(state: &GarageState, aspect: f32) -> Vec<HudVertex> {
    let mut v = Vec::new();
    let spec = state.draft().assembled_spec();

    push_tech_tree(&mut v, state, aspect);
    push_tabs(&mut v, state, aspect);
    match state.active_tab() {
        GarageTab::Modules => panels::push_modules_panel(&mut v, state, aspect),
        GarageTab::Ammo => panels::push_ammo_panel(&mut v, state, aspect),
        GarageTab::Crew => panels::push_crew_panel(&mut v, state, aspect),
    }
    panels::push_stats_panel(&mut v, &spec, aspect);
    panels::push_battle_button(&mut v, aspect);
    v
}

pub(super) fn hit_test(state: &GarageState) -> GarageHit {
    let p = state.cursor_clip();

    if in_rect(p, BATTLE_CENTER, BATTLE_HALF) {
        return GarageHit::Battle;
    }
    for (i, tab) in GarageTab::ALL.into_iter().enumerate() {
        if in_rect(p, tab_center(i), TAB_HALF) {
            return GarageHit::Tab(tab);
        }
    }
    match state.active_tab() {
        GarageTab::Modules => {
            for (i, slot) in FitSlot::ALL.into_iter().enumerate() {
                if !state.draft().has_choice(slot) {
                    continue;
                }
                let y = PANEL_ROW_TOP - i as f32 * PANEL_ROW_PITCH;
                if in_rect(p, [ARROW_LEFT_X, y], ARROW_HALF) {
                    return GarageHit::ModuleCycle(slot, -1);
                }
                if in_rect(p, [ARROW_RIGHT_X, y], ARROW_HALF) {
                    return GarageHit::ModuleCycle(slot, 1);
                }
            }
        }
        GarageTab::Ammo => {
            for i in 0..state.draft().ammo_options().len() {
                let y = AMMO_TOP - i as f32 * AMMO_PITCH;
                if in_rect(p, [AMMO_CENTER_X, y], AMMO_HALF) {
                    return GarageHit::AmmoSelect(i);
                }
            }
        }
        GarageTab::Crew => {
            if in_rect(p, [PROF_LEFT_X, PROF_Y], ARROW_HALF) {
                return GarageHit::CrewProf(-1);
            }
            if in_rect(p, [PROF_RIGHT_X, PROF_Y], ARROW_HALF) {
                return GarageHit::CrewProf(1);
            }
        }
    }
    for i in 0..VehicleKind::ALL.len() {
        if in_rect(p, [TREE_X, VEH_TOP - i as f32 * VEH_PITCH], [TREE_HALF_X, ROW_HALF_Y]) {
            return GarageHit::Vehicle(i);
        }
    }
    // The module section of the tree is a shortcut into the Modules tab.
    for i in 0..FitSlot::ALL.len() {
        if in_rect(p, [TREE_X, MOD_TOP - i as f32 * MOD_PITCH], [TREE_HALF_X, ROW_HALF_Y]) {
            return GarageHit::Tab(GarageTab::Modules);
        }
    }
    GarageHit::Scene
}

fn push_tech_tree(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    push_quad(v, [TREE_X, 0.36], [TREE_HALF_X + 0.03, 0.60], PANEL);

    push_text(v, "VEHICLES", TREE_X - TREE_HALF_X + 0.01, 0.94, 0.042, aspect, TEXT_DIM);
    for (i, kind) in VehicleKind::ALL.into_iter().enumerate() {
        let y = VEH_TOP - i as f32 * VEH_PITCH;
        let selected = i == state.selected_index();
        push_quad(v, [TREE_X, y], [TREE_HALF_X, ROW_HALF_Y], if selected { ROW_SELECTED } else { ROW });
        let color = if selected { TEXT_DARK } else { TEXT };
        let label = format!("{}  {}", i + 1, short_name(kind));
        push_text(v, &label, TREE_X - TREE_HALF_X + 0.02, y + 0.022, 0.04, aspect, color);
    }

    push_text(v, "MODULES", TREE_X - TREE_HALF_X + 0.01, 0.345, 0.042, aspect, TEXT_DIM);
    for (i, slot) in FitSlot::ALL.into_iter().enumerate() {
        let y = MOD_TOP - i as f32 * MOD_PITCH;
        push_quad(v, [TREE_X, y], [TREE_HALF_X, ROW_HALF_Y], ROW);
        let name = truncate(&state.draft().module_name(slot), 16);
        push_text(v, slot.label(), TREE_X - TREE_HALF_X + 0.02, y + 0.022, 0.034, aspect, TEXT_DIM);
        push_text(v, &name, TREE_X - 0.05, y + 0.022, 0.034, aspect, TEXT);
    }
}

fn push_tabs(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    for (i, tab) in GarageTab::ALL.into_iter().enumerate() {
        let center = tab_center(i);
        let active = tab == state.active_tab();
        push_quad(v, center, TAB_HALF, if active { TAB_ACTIVE } else { TAB });
        let w = text_width(tab.label(), 0.04, aspect);
        push_text(v, tab.label(), center[0] - w / 2.0, center[1] + 0.02, 0.04, aspect, TEXT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(garage: &mut GarageState, point: [f32; 2]) -> GarageHit {
        garage.set_cursor(point);
        garage.hit_test()
    }

    #[test]
    fn cursor_hits_tabs_vehicles_and_battle() {
        let mut g = GarageState::default();
        assert_eq!(at(&mut g, tab_center(1)), GarageHit::Tab(GarageTab::Ammo));
        assert_eq!(at(&mut g, BATTLE_CENTER), GarageHit::Battle);
        assert_eq!(at(&mut g, [TREE_X, VEH_TOP - 3.0 * VEH_PITCH]), GarageHit::Vehicle(3));
        assert_eq!(at(&mut g, [0.0, -0.05]), GarageHit::Scene);
    }

    #[test]
    fn module_cycle_arrows_hit_only_for_swappable_slots() {
        // T-54 has two guns (index 1 in FitSlot::ALL) -> its arrows are live.
        let mut g = GarageState::default();
        g.select_vehicle(VehicleKind::T54_1951);
        let y = PANEL_ROW_TOP - 1.0 * PANEL_ROW_PITCH;
        assert_eq!(at(&mut g, [ARROW_RIGHT_X, y]), GarageHit::ModuleCycle(FitSlot::Gun, 1));
        assert_eq!(at(&mut g, [ARROW_LEFT_X, y]), GarageHit::ModuleCycle(FitSlot::Gun, -1));
    }

    #[test]
    fn ammo_rows_select_when_the_ammo_tab_is_open() {
        let mut g = GarageState::default();
        g.set_tab(GarageTab::Ammo);
        assert_eq!(at(&mut g, [AMMO_CENTER_X, AMMO_TOP - AMMO_PITCH]), GarageHit::AmmoSelect(1));
    }

    #[test]
    fn crew_proficiency_arrows_hit_when_the_crew_tab_is_open() {
        let mut g = GarageState::default();
        g.set_tab(GarageTab::Crew);
        assert_eq!(at(&mut g, [PROF_RIGHT_X, PROF_Y]), GarageHit::CrewProf(1));
        assert_eq!(at(&mut g, [PROF_LEFT_X, PROF_Y]), GarageHit::CrewProf(-1));
    }
}
