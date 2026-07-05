//! Shared garage-overlay geometry and palette, arranged like the WoT beta garage: a top bar with
//! the Battle button, a left crew column, a right stats list, a bottom loadout strip, and a bottom
//! vehicle carousel. Every rect lives here once so drawing (panels) and hit-testing (overlay) agree.

use game_core::Nation;

use super::draft::FitSlot;
use crate::hud::theme;

// Palette: aliases into the art-direction tokens (`hud/theme.rs`) so the garage and the battle
// HUD stay one visual system. Retint in the theme, not here.
pub(super) const PANEL: [f32; 4] = theme::color::PANEL;
pub(super) const SLOT: [f32; 4] = theme::color::SLOT;
pub(super) const SLOT_SELECTED: [f32; 4] = theme::color::SLOT_SELECTED;
pub(super) const SLOT_FOCUSED: [f32; 4] = theme::color::SLOT_FOCUSED;
pub(super) const REJECTED: [f32; 4] = theme::color::REJECTED;
pub(super) const HOVER: [f32; 4] = theme::color::HOVER;
pub(super) const BATTLE: [f32; 4] = theme::color::SIGNAL;
pub(super) const ICON: [f32; 4] = theme::color::ICON;
pub(super) const ICON_DIM: [f32; 4] = theme::color::ICON_DIM;
pub(super) const TEXT: [f32; 4] = theme::color::TEXT;
pub(super) const TEXT_DIM: [f32; 4] = theme::color::TEXT_DIM;
pub(super) const VALUE: [f32; 4] = theme::color::VALUE;
pub(super) const HAIRLINE: [f32; 4] = theme::color::HAIRLINE;
pub(super) const CHAMFER_PANEL: f32 = theme::CHAMFER_PANEL;
pub(super) const CHAMFER_SLOT: f32 = theme::CHAMFER_SLOT;

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

// Bottom vehicle carousel. A fixed window of cells centred on screen scrolls through the roster
// once it outgrows `CAR_VISIBLE`; below that everything fits and the scroll arrows stay hidden.
pub(super) const CAR_Y: f32 = -0.87;
pub(super) const CAR_HALF: [f32; 2] = [0.058, 0.072];
const CAR_STEP: f32 = 0.13;
pub(super) const NATION_TEXT_SIZE: f32 = 0.022;
/// Most cells shown at once; the roster scrolls through this window when larger.
pub(super) const CAR_VISIBLE: usize = 7;
/// Scroll-arrow hit rects, just outside the widest window (drawn only when the roster overflows).
pub(super) const CAR_ARROW_HALF: [f32; 2] = [0.028, 0.072];
const CAR_ARROW_X: f32 = 0.52;

pub(super) fn module_slot_center(i: usize) -> [f32; 2] {
    [MODULE_START_X + i as f32 * SLOT_STEP, LOADOUT_Y]
}

pub(super) fn ammo_slot_center(i: usize) -> [f32; 2] {
    [AMMO_START_X + i as f32 * SLOT_STEP, LOADOUT_Y]
}

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

/// Screen centre of the `slot`-th visible cell (0-based within a window of `visible` cells),
/// centred on screen so a partial last window still reads as centred.
pub(super) fn carousel_cell_center(slot: usize, visible: usize) -> [f32; 2] {
    let start = -((visible as f32 - 1.0) / 2.0) * CAR_STEP;
    [start + slot as f32 * CAR_STEP, CAR_Y]
}

/// Left/right scroll-arrow centres.
pub(super) fn carousel_arrows() -> ([f32; 2], [f32; 2]) {
    ([-CAR_ARROW_X, CAR_Y], [CAR_ARROW_X, CAR_Y])
}

pub(super) fn crew_prof_arrows() -> ([f32; 2], [f32; 2]) {
    ([PROF_LEFT_X, PROF_Y], [PROF_RIGHT_X, PROF_Y])
}

pub(super) fn in_rect(point: [f32; 2], center: [f32; 2], half: [f32; 2]) -> bool {
    (point[0] - center[0]).abs() <= half[0] && (point[1] - center[1]).abs() <= half[1]
}

pub(super) use crate::vehicle::display::short_name;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_roster_that_fits_shows_everything_and_never_scrolls() {
        let count = CAR_VISIBLE - 1;
        assert!(!carousel_overflows(count));
        assert_eq!(carousel_window(count, 0), 0..count);
        // Even a non-zero scroll request clamps to 0 when it all fits.
        assert_eq!(clamp_carousel_scroll(count, 3), 0);
        assert_eq!(carousel_window(count, 3), 0..count);
    }

    #[test]
    fn a_large_roster_windows_and_clamps_the_scroll() {
        let count = 20;
        assert!(carousel_overflows(count));
        // The window is always CAR_VISIBLE wide and slides with the scroll.
        assert_eq!(carousel_window(count, 0), 0..CAR_VISIBLE);
        assert_eq!(carousel_window(count, 5), 5..5 + CAR_VISIBLE);
        // Scroll past the end clamps so the last cell stays flush against the right edge.
        let max_scroll = count - CAR_VISIBLE;
        assert_eq!(clamp_carousel_scroll(count, 999), max_scroll);
        assert_eq!(carousel_window(count, 999), max_scroll..count);
    }

    #[test]
    fn visible_cells_are_centred_on_screen() {
        // A full window is symmetric about x = 0.
        let first = carousel_cell_center(0, CAR_VISIBLE);
        let last = carousel_cell_center(CAR_VISIBLE - 1, CAR_VISIBLE);
        assert!((first[0] + last[0]).abs() < 1.0e-6, "the window straddles the centre");
        assert_eq!(first[1], CAR_Y);
    }
}
