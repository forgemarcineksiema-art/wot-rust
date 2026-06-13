//! Garage HUD: the left tech-tree list (vehicles + the current tank's modules), the fitting tabs
//! (Modules / Ammo / Crew), a live stats panel driven by the edited loadout, and the Battle
//! button. Drawing and cursor hit-testing share the same rect helpers so they cannot drift apart.

use game_core::{ShellType, TankSpec, VehicleKind};
use renderer_api::HudVertex;

use super::draft::FitSlot;
use super::{GarageHit, GarageState, GarageTab};
use crate::hud::push_quad;
use crate::hud_font::{push_text, text_width};

const PANEL: [f32; 4] = [0.04, 0.05, 0.06, 0.78];
const ROW: [f32; 4] = [0.12, 0.14, 0.16, 0.86];
const ROW_SELECTED: [f32; 4] = [0.62, 0.78, 0.42, 0.92];
const TAB: [f32; 4] = [0.14, 0.16, 0.19, 0.90];
const TAB_ACTIVE: [f32; 4] = [0.40, 0.55, 0.70, 0.95];
const BUTTON: [f32; 4] = [0.20, 0.24, 0.28, 0.95];
const BATTLE: [f32; 4] = [0.78, 0.30, 0.20, 0.95];
const STAT_HP: [f32; 4] = [0.35, 0.78, 0.36, 0.95];
const STAT_SPEED: [f32; 4] = [0.35, 0.62, 0.92, 0.95];
const STAT_RELOAD: [f32; 4] = [0.90, 0.62, 0.32, 0.95];
const TEXT: [f32; 4] = [0.90, 0.93, 0.88, 0.97];
const TEXT_DIM: [f32; 4] = [0.74, 0.78, 0.74, 0.85];
const TEXT_DARK: [f32; 4] = [0.10, 0.12, 0.10, 0.98];

// Left tech-tree list.
const TREE_X: f32 = -0.74;
const TREE_HALF_X: f32 = 0.22;
const ROW_HALF_Y: f32 = 0.030;
const VEH_TOP: f32 = 0.85;
const VEH_PITCH: f32 = 0.072;
const MOD_TOP: f32 = 0.27;
const MOD_PITCH: f32 = 0.072;

// Fitting tabs (top-right).
const TAB_Y: f32 = 0.90;
const TAB_X0: f32 = 0.34;
const TAB_DX: f32 = 0.24;
const TAB_HALF: [f32; 2] = [0.11, 0.045];

// Module-panel rows + cycle arrows (six slots).
const PANEL_ROW_TOP: f32 = 0.70;
const PANEL_ROW_PITCH: f32 = 0.105;
const ARROW_LEFT_X: f32 = 0.86;
const ARROW_RIGHT_X: f32 = 0.93;
const ARROW_HALF: [f32; 2] = [0.024, 0.035];

// Ammo rows.
const AMMO_TOP: f32 = 0.66;
const AMMO_PITCH: f32 = 0.15;
const AMMO_HALF: [f32; 2] = [0.36, 0.055];
const AMMO_CENTER_X: f32 = 0.58;

// Crew proficiency control.
const PROF_Y: f32 = 0.14;
const PROF_LEFT_X: f32 = 0.74;
const PROF_RIGHT_X: f32 = 0.92;

const BATTLE_CENTER: [f32; 2] = [0.70, -0.82];
const BATTLE_HALF: [f32; 2] = [0.24, 0.10];

pub(super) fn build(state: &GarageState, aspect: f32) -> Vec<HudVertex> {
    let mut v = Vec::new();
    let spec = state.draft().assembled_spec();

    push_tech_tree(&mut v, state, aspect);
    push_tabs(&mut v, state, aspect);
    match state.active_tab() {
        GarageTab::Modules => push_modules_panel(&mut v, state, aspect),
        GarageTab::Ammo => push_ammo_panel(&mut v, state, aspect),
        GarageTab::Crew => push_crew_panel(&mut v, state, aspect),
    }
    push_stats_panel(&mut v, &spec, aspect);
    push_battle_button(&mut v, aspect);
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

fn push_modules_panel(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    push_quad(v, [0.58, 0.46], [0.40, 0.33], PANEL);
    for (i, slot) in FitSlot::ALL.into_iter().enumerate() {
        let y = PANEL_ROW_TOP - i as f32 * PANEL_ROW_PITCH;
        push_text(v, slot.label(), 0.22, y + 0.03, 0.045, aspect, TEXT_DIM);
        let name = truncate(&state.draft().module_name(slot), 22);
        push_text(v, &name, 0.40, y + 0.03, 0.045, aspect, TEXT);
        if state.draft().has_choice(slot) {
            push_arrow(v, [ARROW_LEFT_X, y], false, aspect);
            push_arrow(v, [ARROW_RIGHT_X, y], true, aspect);
        }
    }
}

fn push_ammo_panel(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    push_quad(v, [0.58, 0.46], [0.40, 0.33], PANEL);
    let options = state.draft().ammo_options();
    let selected = state.draft().ammo_index();
    for (i, shell) in options.iter().enumerate() {
        let y = AMMO_TOP - i as f32 * AMMO_PITCH;
        push_quad(v, [AMMO_CENTER_X, y], AMMO_HALF, if i == selected { ROW_SELECTED } else { ROW });
        let color = if i == selected { TEXT_DARK } else { TEXT };
        push_text(v, shell_label(shell.shell_type), AMMO_CENTER_X - AMMO_HALF[0] + 0.02, y + 0.02, 0.045, aspect, color);
        let stats = format!("PEN {}  DMG {}", shell.penetration_mm_at_100m.round() as u32, shell.damage_hp);
        push_text(v, &stats, AMMO_CENTER_X - 0.05, y + 0.02, 0.04, aspect, color);
    }
}

fn push_crew_panel(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    push_quad(v, [0.58, 0.46], [0.40, 0.33], PANEL);
    for (i, role) in game_core::Crew::roles().into_iter().enumerate() {
        let y = 0.70 - i as f32 * 0.10;
        push_text(v, role.label(), 0.22, y + 0.025, 0.045, aspect, TEXT);
    }
    push_text(v, "Proficiency", 0.22, PROF_Y + 0.03, 0.045, aspect, TEXT_DIM);
    push_arrow(v, [PROF_LEFT_X, PROF_Y], false, aspect);
    push_arrow(v, [PROF_RIGHT_X, PROF_Y], true, aspect);
    let pct = (state.draft().crew().proficiency() * 100.0).round() as u32;
    let label = format!("{pct}%");
    let w = text_width(&label, 0.05, aspect);
    push_text(v, &label, (PROF_LEFT_X + PROF_RIGHT_X) / 2.0 - w / 2.0, PROF_Y + 0.028, 0.05, aspect, TEXT);
}

fn push_stats_panel(v: &mut Vec<HudVertex>, spec: &TankSpec, aspect: f32) {
    push_quad(v, [0.58, -0.30], [0.40, 0.22], PANEL);
    push_labeled_stat(v, "HP", [0.36, -0.14], spec.hit_points as f32 / fleet_max_hp(), STAT_HP, aspect);
    push_labeled_stat(v, "SPD", [0.36, -0.24], spec.max_forward_speed_mps / fleet_max_speed(), STAT_SPEED, aspect);
    push_labeled_stat(v, "RLD", [0.36, -0.34], fleet_min_reload() / spec.gun.reload_seconds, STAT_RELOAD, aspect);
    let line = format!(
        "PEN {}mm   PWR {}kW",
        spec.gun.shell.penetration_mm_at_100m.round() as u32,
        spec.engine_power_kw.round() as u32,
    );
    push_text(v, &line, 0.22, -0.44, 0.04, aspect, TEXT_DIM);
}

fn push_battle_button(v: &mut Vec<HudVertex>, aspect: f32) {
    push_quad(v, BATTLE_CENTER, BATTLE_HALF, BATTLE);
    let w = text_width("BITWA", 0.08, aspect);
    push_text(v, "BITWA", BATTLE_CENTER[0] - w / 2.0, BATTLE_CENTER[1] + 0.04, 0.08, aspect, TEXT);
}

fn push_arrow(v: &mut Vec<HudVertex>, center: [f32; 2], right: bool, aspect: f32) {
    push_quad(v, center, ARROW_HALF, BUTTON);
    let glyph = if right { ">" } else { "<" };
    let w = text_width(glyph, 0.05, aspect);
    push_text(v, glyph, center[0] - w / 2.0, center[1] + 0.025, 0.05, aspect, TEXT);
}

fn push_labeled_stat(
    v: &mut Vec<HudVertex>,
    label: &str,
    center: [f32; 2],
    fraction: f32,
    color: [f32; 4],
    aspect: f32,
) {
    push_text(v, label, center[0] - 0.14, center[1] + 0.022, 0.04, aspect, TEXT_DIM);
    let half = [0.26, 0.014];
    let bar_center = [center[0] + 0.30, center[1]];
    push_quad(v, bar_center, half, [0.0, 0.0, 0.0, 0.55]);
    let fill = half[0] * fraction.clamp(0.0, 1.0);
    push_quad(v, [bar_center[0] - half[0] + fill, bar_center[1]], [fill, half[1]], color);
}

fn tab_center(i: usize) -> [f32; 2] {
    [TAB_X0 + i as f32 * TAB_DX, TAB_Y]
}

fn in_rect(point: [f32; 2], center: [f32; 2], half: [f32; 2]) -> bool {
    (point[0] - center[0]).abs() <= half[0] && (point[1] - center[1]).abs() <= half[1]
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        text.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}

fn shell_label(kind: ShellType) -> &'static str {
    match kind {
        ShellType::ArmorPiercing => "AP",
        ShellType::Apcr => "APCR",
        ShellType::Heat => "HEAT",
        ShellType::HighExplosive => "HE",
    }
}

fn fleet_max_hp() -> f32 {
    VehicleKind::ALL.iter().map(|k| k.spec().hit_points).max().unwrap_or(1) as f32
}

fn fleet_max_speed() -> f32 {
    VehicleKind::ALL.iter().map(|k| k.spec().max_forward_speed_mps).fold(1.0, f32::max)
}

fn fleet_min_reload() -> f32 {
    VehicleKind::ALL.iter().map(|k| k.spec().gun.reload_seconds).fold(f32::INFINITY, f32::min)
}

fn short_name(kind: VehicleKind) -> &'static str {
    match kind {
        VehicleKind::PrototypeMedium => "Prototype",
        VehicleKind::T54_1951 => "T-54",
        VehicleKind::T55A => "T-55A",
        VehicleKind::TigerI => "Tiger I",
        VehicleKind::TigerII => "Tiger II",
        VehicleKind::Jagdtiger => "Jagdtiger",
        VehicleKind::PantherII => "Panther II",
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
