//! Shared garage-overlay geometry and palette. Every rect the overlay draws and the hit test
//! checks lives here once, so drawing and clicking can never drift apart.

use game_core::{ShellType, VehicleKind};

pub(super) const PANEL: [f32; 4] = [0.04, 0.05, 0.06, 0.78];
pub(super) const ROW: [f32; 4] = [0.12, 0.14, 0.16, 0.86];
pub(super) const ROW_SELECTED: [f32; 4] = [0.62, 0.78, 0.42, 0.92];
pub(super) const TAB: [f32; 4] = [0.14, 0.16, 0.19, 0.90];
pub(super) const TAB_ACTIVE: [f32; 4] = [0.40, 0.55, 0.70, 0.95];
pub(super) const BUTTON: [f32; 4] = [0.20, 0.24, 0.28, 0.95];
pub(super) const BATTLE: [f32; 4] = [0.78, 0.30, 0.20, 0.95];
pub(super) const STAT_HP: [f32; 4] = [0.35, 0.78, 0.36, 0.95];
pub(super) const STAT_SPEED: [f32; 4] = [0.35, 0.62, 0.92, 0.95];
pub(super) const STAT_RELOAD: [f32; 4] = [0.90, 0.62, 0.32, 0.95];
pub(super) const TEXT: [f32; 4] = [0.90, 0.93, 0.88, 0.97];
pub(super) const TEXT_DIM: [f32; 4] = [0.74, 0.78, 0.74, 0.85];
pub(super) const TEXT_DARK: [f32; 4] = [0.10, 0.12, 0.10, 0.98];

// Left tech-tree list.
pub(super) const TREE_X: f32 = -0.74;
pub(super) const TREE_HALF_X: f32 = 0.22;
pub(super) const ROW_HALF_Y: f32 = 0.030;
pub(super) const VEH_TOP: f32 = 0.85;
pub(super) const VEH_PITCH: f32 = 0.072;
pub(super) const MOD_TOP: f32 = 0.27;
pub(super) const MOD_PITCH: f32 = 0.072;

// Fitting tabs (top-right).
pub(super) const TAB_Y: f32 = 0.90;
pub(super) const TAB_X0: f32 = 0.34;
pub(super) const TAB_DX: f32 = 0.24;
pub(super) const TAB_HALF: [f32; 2] = [0.11, 0.045];

// Module-panel rows + cycle arrows (six slots).
pub(super) const PANEL_ROW_TOP: f32 = 0.70;
pub(super) const PANEL_ROW_PITCH: f32 = 0.105;
pub(super) const ARROW_LEFT_X: f32 = 0.86;
pub(super) const ARROW_RIGHT_X: f32 = 0.93;
pub(super) const ARROW_HALF: [f32; 2] = [0.024, 0.035];

// Ammo rows.
pub(super) const AMMO_TOP: f32 = 0.66;
pub(super) const AMMO_PITCH: f32 = 0.15;
pub(super) const AMMO_HALF: [f32; 2] = [0.36, 0.055];
pub(super) const AMMO_CENTER_X: f32 = 0.58;

// Crew proficiency control.
pub(super) const PROF_Y: f32 = 0.14;
pub(super) const PROF_LEFT_X: f32 = 0.74;
pub(super) const PROF_RIGHT_X: f32 = 0.92;

pub(super) const BATTLE_CENTER: [f32; 2] = [0.70, -0.82];
pub(super) const BATTLE_HALF: [f32; 2] = [0.24, 0.10];

pub(super) fn tab_center(i: usize) -> [f32; 2] {
    [TAB_X0 + i as f32 * TAB_DX, TAB_Y]
}

pub(super) fn in_rect(point: [f32; 2], center: [f32; 2], half: [f32; 2]) -> bool {
    (point[0] - center[0]).abs() <= half[0] && (point[1] - center[1]).abs() <= half[1]
}

pub(super) fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        text.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}

pub(super) fn shell_label(kind: ShellType) -> &'static str {
    match kind {
        ShellType::ArmorPiercing => "AP",
        ShellType::Apcr => "APCR",
        ShellType::Heat => "HEAT",
        ShellType::HighExplosive => "HE",
    }
}

pub(super) fn fleet_max_hp() -> f32 {
    VehicleKind::ALL.iter().map(|k| k.spec().hit_points).max().unwrap_or(1) as f32
}

pub(super) fn fleet_max_speed() -> f32 {
    VehicleKind::ALL.iter().map(|k| k.spec().max_forward_speed_mps).fold(1.0, f32::max)
}

pub(super) fn fleet_min_reload() -> f32 {
    VehicleKind::ALL.iter().map(|k| k.spec().gun.reload_seconds).fold(f32::INFINITY, f32::min)
}

pub(super) fn short_name(kind: VehicleKind) -> &'static str {
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
