//! What the garage still lays out in clip space: the tech tree (until G12), the map row's
//! words, the slots' words and icons, and the carousel's window arithmetic. Everything the
//! hangar screen draws is laid out in `u` by `screen.rs`.

use super::draft::FitSlot;

/// Label for the map row. `None` is AUTO — the env/default resolution, not a map name.
pub(super) fn map_pick_label(map: Option<terrain::MapId>) -> &'static str {
    use terrain::MapId;
    match map {
        None => "MAP: AUTO",
        Some(MapId::ProkhorovkaHill252_2) => "MAP: PROKHOROVKA",
        Some(MapId::BystraValley) => "MAP: BYSTRA VALLEY",
        Some(MapId::OrlinyPereval) => "MAP: ORLINY PEREVAL",
        Some(MapId::Ostrogorsk) => "MAP: OSTROGORSK",
        Some(MapId::MazurskiPrzesmyk) => "MAP: MAZURSKI PRZESMYK",
        Some(MapId::Scratch) => "MAP: SCRATCH",
    }
}

/// Most carousel cells shown at once; the roster scrolls through this window when larger.
pub(super) const CAR_VISIBLE: usize = 8;

/// Whether the roster needs the scroll window (and the arrows).
pub(super) fn carousel_overflows(count: usize) -> bool {
    count > CAR_VISIBLE
}

/// Clamp a desired first-visible index to the valid range for `count` (0 when it all fits).
pub(super) fn clamp_carousel_scroll(count: usize, scroll: usize) -> usize {
    scroll.min(count.saturating_sub(CAR_VISIBLE))
}

/// The absolute roster indices currently visible, given `count` and the clamped `scroll`.
pub(super) fn carousel_window(count: usize, scroll: usize) -> std::ops::Range<usize> {
    if count <= CAR_VISIBLE {
        return 0..count;
    }
    let start = clamp_carousel_scroll(count, scroll);
    start..start + CAR_VISIBLE
}

/// The garage's rects are the HUD's rects — one hit test for every clip-space surface left.
pub(super) use crate::hud::primitives::in_rect;

/// The header label for a fitting slot, shown atop its option list.
pub(super) fn slot_label(slot: FitSlot) -> &'static str {
    match slot {
        FitSlot::Turret => "TURRET",
        FitSlot::Gun => "GUN",
        FitSlot::Hull => "HULL",
        FitSlot::Engine => "ENGINE",
        FitSlot::Suspension => "SUSPENSION",
        FitSlot::Radio => "RADIO",
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

/// The icon for a rack slot, mapped from the shell's TYPE (a HEAT-loading gun shows the
/// shaped-charge glyph, not a fixed per-index icon).
pub(super) fn ammo_icon(shell_type: game_core::ShellType) -> crate::hud::icons::HudIcon {
    crate::hud::icons::HudIcon::for_shell(shell_type)
}

// Tech tree: nation groups, line columns, tier rows (higher tier higher). Only PLAYABLE
// vehicles get a node — no reserved empty bands, no ghost predecessors.
pub(super) const TREE_PANEL_CENTER: [f32; 2] = [0.0, 0.30];
pub(super) const TREE_PANEL_HALF: [f32; 2] = [0.82, 0.36];
pub(super) const TREE_CLOSE_CENTER: [f32; 2] = [0.86, 0.80];
pub(super) const TREE_CLOSE_HALF: [f32; 2] = [0.06, 0.04];
const TREE_COL_LEFT: f32 = -0.70;
const TREE_COL_RIGHT: f32 = 0.72;
const TREE_TIER_TOP: f32 = 0.40;
const TREE_TIER_PITCH: f32 = 0.155;
const TREE_NODE_HALF_Y: f32 = 0.052;
const TREE_HIGHEST_TIER: u8 = 9;
pub(super) const TREE_NATION_LABEL_Y: f32 = 0.58;
pub(super) const TREE_LINE_LABEL_Y: f32 = 0.515;

/// Occupied (nation, class) columns, nation-major then class-major. Empty lines are skipped.
pub(super) fn tree_columns() -> Vec<(game_core::Nation, game_core::VehicleClass)> {
    let mut cols = Vec::new();
    for nation in game_core::Nation::ALL {
        for class in game_core::VehicleClass::ALL {
            if game_core::VehicleKind::PLAYABLE
                .iter()
                .any(|kind| kind.nation() == nation && kind.class() == class)
            {
                cols.push((nation, class));
            }
        }
    }
    cols
}

pub(super) fn tree_tier_y(tier: u8) -> f32 {
    TREE_TIER_TOP - f32::from(TREE_HIGHEST_TIER.saturating_sub(tier)) * TREE_TIER_PITCH
}

pub(super) fn tree_col_x(col: usize) -> f32 {
    let n = tree_columns().len().max(1) as f32;
    let pitch = (TREE_COL_RIGHT - TREE_COL_LEFT) / n;
    TREE_COL_LEFT + pitch * (col as f32 + 0.5)
}

pub(super) fn tree_node_half() -> [f32; 2] {
    let n = tree_columns().len().max(1) as f32;
    let pitch = (TREE_COL_RIGHT - TREE_COL_LEFT) / n;
    [(pitch * 0.40).min(0.10), TREE_NODE_HALF_Y]
}

pub(super) fn tree_node_center(kind: game_core::VehicleKind) -> [f32; 2] {
    let cols = tree_columns();
    let col = cols
        .iter()
        .position(|&(nation, class)| nation == kind.nation() && class == kind.class())
        .expect("playable vehicle owns a tree column");
    [tree_col_x(col), tree_tier_y(kind.tier())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_roster_that_fits_shows_everything_and_never_scrolls() {
        let count = CAR_VISIBLE - 1;
        assert!(!carousel_overflows(count));
        assert_eq!(carousel_window(count, 0), 0..count);
        assert_eq!(clamp_carousel_scroll(count, 3), 0);
        assert_eq!(carousel_window(count, 3), 0..count);
    }

    #[test]
    fn a_large_roster_windows_and_clamps_the_scroll() {
        let count = 20;
        assert!(carousel_overflows(count));
        assert_eq!(carousel_window(count, 0), 0..CAR_VISIBLE);
        assert_eq!(carousel_window(count, 5), 5..5 + CAR_VISIBLE);
        let max_scroll = count - CAR_VISIBLE;
        assert_eq!(clamp_carousel_scroll(count, 999), max_scroll);
        assert_eq!(carousel_window(count, 999), max_scroll..count);
    }

    #[test]
    fn every_tree_node_of_the_live_fleet_stays_inside_the_panel() {
        let half = tree_node_half();
        for kind in game_core::VehicleKind::PLAYABLE {
            let center = tree_node_center(kind);
            assert!(
                center[0] - half[0] >= TREE_PANEL_CENTER[0] - TREE_PANEL_HALF[0],
                "{kind:?} leaks off the panel's left edge"
            );
            assert!(
                center[0] + half[0] <= TREE_PANEL_CENTER[0] + TREE_PANEL_HALF[0],
                "{kind:?} leaks off the panel's right edge"
            );
        }
        let cols = tree_columns();
        if cols.len() >= 2 {
            let gap = tree_col_x(1) - tree_col_x(0);
            assert!(gap >= 2.0 * half[0] + 0.01, "line columns overlap");
        }
    }
}
