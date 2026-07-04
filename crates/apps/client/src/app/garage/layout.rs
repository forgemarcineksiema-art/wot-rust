//! Shared garage-overlay geometry and palette, arranged like the WoT beta garage: a top bar with
//! the Battle button, a left crew column, a right stats list, a bottom loadout strip, and a bottom
//! vehicle carousel. Every rect lives here once so drawing (panels) and hit-testing (overlay) agree.

use game_core::{Nation, VehicleKind};

use super::draft::FitSlot;

pub(super) const PANEL: [f32; 4] = [0.05, 0.06, 0.07, 0.74];
pub(super) const SLOT: [f32; 4] = [0.13, 0.15, 0.17, 0.92];
pub(super) const SLOT_SELECTED: [f32; 4] = [0.40, 0.55, 0.70, 0.95];
pub(super) const SLOT_FOCUSED: [f32; 4] = [0.22, 0.34, 0.44, 0.95];
pub(super) const REJECTED: [f32; 4] = [0.50, 0.12, 0.10, 0.92];
pub(super) const HOVER: [f32; 4] = [1.0, 1.0, 1.0, 0.10];
pub(super) const BATTLE: [f32; 4] = [0.74, 0.22, 0.18, 0.97];
pub(super) const ICON: [f32; 4] = [0.86, 0.88, 0.84, 0.96];
pub(super) const ICON_DIM: [f32; 4] = [0.70, 0.73, 0.70, 0.85];
pub(super) const TEXT: [f32; 4] = [0.90, 0.93, 0.88, 0.97];
pub(super) const TEXT_DIM: [f32; 4] = [0.72, 0.76, 0.72, 0.85];
pub(super) const VALUE: [f32; 4] = [0.96, 0.92, 0.70, 0.98];

// Top bar.
pub(super) const BATTLE_CENTER: [f32; 2] = [0.0, 0.90];
pub(super) const BATTLE_HALF: [f32; 2] = [0.13, 0.052];
/// Hit-test rect for the clickable TECH TREE tab.
pub(super) const TECH_TREE_TAB_CENTER: [f32; 2] = [0.22, 0.815];
pub(super) const TECH_TREE_TAB_HALF: [f32; 2] = [0.10, 0.03];

// Left crew column.
pub(super) const CREW_X: f32 = -0.80;
pub(super) const CREW_HALF_X: f32 = 0.18;
pub(super) const CREW_TOP: f32 = 0.74;
pub(super) const CREW_PITCH: f32 = 0.105;
pub(super) const PROF_Y: f32 = 0.18;
pub(super) const PROF_LEFT_X: f32 = -0.90;
pub(super) const PROF_RIGHT_X: f32 = -0.70;
pub(super) const ARROW_HALF: [f32; 2] = [0.024, 0.034];

// Right stats list.
pub(super) const STAT_X: f32 = 0.78;
pub(super) const STAT_HALF_X: f32 = 0.20;
pub(super) const STAT_TOP: f32 = 0.74;
pub(super) const STAT_PITCH: f32 = 0.105;

// Bottom loadout strip: six module slots, a gap, then three ammo slots.
pub(super) const LOADOUT_Y: f32 = -0.64;
pub(super) const SLOT_HALF: [f32; 2] = [0.042, 0.058];
const MODULE_START_X: f32 = -0.46;
const SLOT_STEP: f32 = 0.10;
const AMMO_START_X: f32 = 0.22;

// Bottom vehicle carousel.
pub(super) const CAR_Y: f32 = -0.87;
pub(super) const CAR_HALF: [f32; 2] = [0.058, 0.072];
const CAR_STEP: f32 = 0.13;
pub(super) const NATION_TEXT_SIZE: f32 = 0.022;

pub(super) fn module_slot_center(i: usize) -> [f32; 2] {
    [MODULE_START_X + i as f32 * SLOT_STEP, LOADOUT_Y]
}

pub(super) fn ammo_slot_center(i: usize) -> [f32; 2] {
    [AMMO_START_X + i as f32 * SLOT_STEP, LOADOUT_Y]
}

pub(super) fn carousel_center(i: usize, count: usize) -> [f32; 2] {
    let start = -((count as f32 - 1.0) / 2.0) * CAR_STEP;
    [start + i as f32 * CAR_STEP, CAR_Y]
}

pub(super) fn crew_prof_arrows() -> ([f32; 2], [f32; 2]) {
    ([PROF_LEFT_X, PROF_Y], [PROF_RIGHT_X, PROF_Y])
}

pub(super) fn in_rect(point: [f32; 2], center: [f32; 2], half: [f32; 2]) -> bool {
    (point[0] - center[0]).abs() <= half[0] && (point[1] - center[1]).abs() <= half[1]
}

pub(super) fn short_name(kind: VehicleKind) -> &'static str {
    match kind {
        VehicleKind::PrototypeMedium => "Proto",
        VehicleKind::T54_1951 => "T-54",
        VehicleKind::T55A => "T-55A",
        VehicleKind::TigerI => "Tiger I",
        VehicleKind::TigerII => "Tiger II",
        VehicleKind::Jagdtiger => "Jagdtg",
        VehicleKind::PantherII => "Panth II",
        VehicleKind::IS3 => "IS-3",
    }
}

/// The icon for a fitting slot.
pub(super) fn slot_icon(slot: FitSlot) -> crate::hud::icons::HudIcon {
    use crate::hud::icons::HudIcon;
    match slot {
        FitSlot::Turret => HudIcon::SlotTurret,
        FitSlot::Gun => HudIcon::SlotGun,
        FitSlot::Hull => HudIcon::SlotHull,
        FitSlot::Engine => HudIcon::SlotEngine,
        FitSlot::Suspension => HudIcon::SlotSuspension,
        FitSlot::Radio => HudIcon::SlotRadio,
    }
}

/// The icon for an ammo index (0 = AP stock, 1 = APCR, 2 = HE).
pub(super) fn ammo_icon(index: usize) -> crate::hud::icons::HudIcon {
    use crate::hud::icons::HudIcon;
    match index {
        1 => HudIcon::AmmoApcr,
        2 => HudIcon::AmmoHe,
        _ => HudIcon::AmmoAp,
    }
}

// Tech tree view: vehicles grouped by nation in vertical columns (the beta-WoT signature).
pub(super) const TREE_PANEL_CENTER: [f32; 2] = [0.0, -0.05];
pub(super) const TREE_PANEL_HALF: [f32; 2] = [0.95, 0.80];
pub(super) const TREE_USSR_X: f32 = -0.40;
pub(super) const TREE_GERMANY_X: f32 = 0.40;
pub(super) const TREE_NODE_HALF: [f32; 2] = [0.14, 0.045];
pub(super) const TREE_NODE_PITCH: f32 = 0.14;
pub(super) const TREE_TOP_Y: f32 = 0.50;
pub(super) const TREE_HEADER_Y: f32 = 0.66;
pub(super) const TREE_CLOSE_CENTER: [f32; 2] = [0.86, 0.80];
pub(super) const TREE_CLOSE_HALF: [f32; 2] = [0.06, 0.04];

pub(super) fn tree_column_x(nation: Nation) -> f32 {
    match nation {
        Nation::Ussr => TREE_USSR_X,
        Nation::Germany => TREE_GERMANY_X,
    }
}

pub(super) fn tree_node_center(nation: Nation, row: usize) -> [f32; 2] {
    [tree_column_x(nation), TREE_TOP_Y - row as f32 * TREE_NODE_PITCH]
}
