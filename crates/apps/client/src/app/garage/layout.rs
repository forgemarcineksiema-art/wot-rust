//! Shared garage-overlay geometry and palette, arranged like the WoT beta garage: a top bar with
//! the Battle button, a left crew column, a right stats list, a bottom loadout strip, and a bottom
//! vehicle carousel. Every rect lives here once so drawing (panels) and hit-testing (overlay) agree.

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

// Top bar. The plate has to reach DOWN far enough to carry the tab row: it used to stop at 0.86
// while the tabs sat at 0.785..0.845, so the only screen switch in the game rendered as dim grey
// text floating on the hangar wall with no plate behind it. Locked by
// `the_top_bar_plate_carries_every_control_that_sits_on_it`.
pub(super) const TOP_BAR_CENTER: [f32; 2] = [0.0, 0.89];
pub(super) const TOP_BAR_HALF: [f32; 2] = [1.0, 0.11];
/// The bar's lower edge, where its closing hairline runs.
pub(super) const TOP_BAR_BOTTOM: f32 = TOP_BAR_CENTER[1] - TOP_BAR_HALF[1];
pub(super) const BATTLE_CENTER: [f32; 2] = [0.0, 0.90];
pub(super) const BATTLE_HALF: [f32; 2] = [0.13, 0.052];
/// Hit-test rect for the clickable TECH TREE tab.
pub(super) const TECH_TREE_TAB_CENTER: [f32; 2] = [0.22, 0.815];
pub(super) const TECH_TREE_TAB_HALF: [f32; 2] = [0.10, 0.03];
/// The GARAGE tab. It used to be drawn left-aligned and was not hit-testable at all — the pair
/// read as two tabs but only one of them answered a click. It is now drawn centred on this rect,
/// like TECH TREE, so the highlight and the hit-test cannot disagree.
pub(super) const GARAGE_TAB_CENTER: [f32; 2] = [-0.38, 0.815];
pub(super) const GARAGE_TAB_HALF: [f32; 2] = [0.10, 0.03];
/// The map row right of the Battle button: click cycles the pre-battle map choice.
pub(super) const MAP_PICK_CENTER: [f32; 2] = [0.34, 0.90];
pub(super) const MAP_PICK_HALF: [f32; 2] = [0.15, 0.036];

/// Label for the map row. `None` is AUTO — the env/default resolution, not a map name.
pub(super) fn map_pick_label(map: Option<terrain::MapId>) -> &'static str {
    use terrain::MapId;
    match map {
        None => "MAP: AUTO",
        Some(MapId::ProkhorovkaHill252_2) => "MAP: PROKHOROVKA",
        Some(MapId::BystraValley) => "MAP: BYSTRA VALLEY",
        Some(MapId::OrlinyPereval) => "MAP: ORLINY PEREVAL",
        Some(MapId::Ostrogorsk) => "MAP: OSTROGORSK",
        Some(MapId::Scratch) => "MAP: SCRATCH",
    }
}

// Left crew column.
pub(super) const CREW_X: f32 = -0.80;
pub(super) const CREW_HALF_X: f32 = 0.18;
// The first row sits below the header hairline (0.755): the row-0 icon top is `CREW_TOP + 0.035`,
// so this must stay under ~0.72 or the first role crashes into the "CREW" header. See the
// `first_row_clears_the_header_hairline` lock in `panels/stats.rs`.
pub(super) const CREW_TOP: f32 = 0.71;
pub(super) const CREW_PITCH: f32 = 0.105;
pub(super) const PROF_Y: f32 = 0.18;
pub(super) const PROF_LEFT_X: f32 = -0.90;
pub(super) const PROF_RIGHT_X: f32 = -0.70;
pub(super) const ARROW_HALF: [f32; 2] = [0.024, 0.034];

// Right stats list.
pub(super) const STAT_X: f32 = 0.78;
pub(super) const STAT_HALF_X: f32 = 0.20;
// Matches `CREW_TOP`: the row-0 icon top is `STAT_TOP + 0.03`, which must clear the header hairline
// at 0.755 (see `first_row_clears_the_header_hairline`).
pub(super) const STAT_TOP: f32 = 0.71;
// Nine rows at the old 0.105 pitch would run off the bottom of the screen. Tightened to 0.095,
// which still leaves 0.055 of air between a 0.04 value and the next row's icon.
pub(super) const STAT_PITCH: f32 = 0.095;
/// Rows in the VEHICLE column. The panel is sized FROM this (`stat_panel`), so adding a stat can
/// never leave its row hanging outside the plate — the failure the six-row panel was one row from.
pub(super) const STAT_ROWS: usize = 9;
/// Top edge of both side panels, and the baseline their header prints from.
pub(super) const PANEL_TOP: f32 = 0.80;
/// Air under the last row's glyphs before the plate ends.
const STAT_PANEL_PAD: f32 = 0.07;

/// The VEHICLE plate, derived from the rows it has to hold rather than hand-tuned beside them.
pub(super) fn stat_panel() -> ([f32; 2], [f32; 2]) {
    let bottom = STAT_TOP - (STAT_ROWS as f32 - 1.0) * STAT_PITCH - STAT_PANEL_PAD;
    let half_y = (PANEL_TOP - bottom) / 2.0;
    ([STAT_X, bottom + half_y], [STAT_HALF_X + 0.02, half_y])
}

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
// At 0.022 the condensed nation label rendered as a ~7px-cap blur; 0.028 brings it up to a legible
// superscript over the vehicle name (the atlas master is 64px, so size — not resolution — was the
// bottleneck). The label y is lifted to match (`carousel.rs`) so it stays clear of the name row.
pub(super) const NATION_TEXT_SIZE: f32 = 0.028;
/// Most cells shown at once; the roster scrolls through this window when larger. Eight cells
/// at the 0.13 step span ±0.52 clip — inside the frame with the arrow rects still clear.
pub(super) const CAR_VISIBLE: usize = 8;
/// Scroll-arrow hit rects, just outside the widest window (drawn only when the roster overflows).
pub(super) const CAR_ARROW_HALF: [f32; 2] = [0.028, 0.072];
// 0.58, not the old 0.52: a full 8-cell window's outer cell edge reaches 0.513, and an arrow
// at 0.52 (half 0.028) started at 0.492 — overlapping the very cell it scrolls, exactly when
// the arrows first appear (the ninth vehicle). Locked by
// `the_scroll_arrows_clear_the_widest_carousel_window`.
const CAR_ARROW_X: f32 = 0.58;

pub(super) fn module_slot_center(i: usize) -> [f32; 2] {
    [MODULE_START_X + i as f32 * SLOT_STEP, LOADOUT_Y]
}

// Module option list: the informed-swap popup the player opens by clicking a swappable slot. Rows
// stack upward from just above the loadout strip, centred on the clicked slot's column, so the
// popup reads as rising out of that slot. Each row carries an option name, its headline stat, and a
// coloured delta against the installed option.
pub(super) const OPTION_ROW_HALF: [f32; 2] = [0.21, 0.030];
pub(super) const OPTION_ROW_PITCH: f32 = 0.066;
/// Lowest row centre (nearest the loadout strip); higher options stack upward toward the tank.
const OPTION_LIST_BASE_Y: f32 = -0.50;
/// Semantic delta colours: a swap that improves the slot's headline stat reads green, a regression
/// red. Kept with the feature — the instrument theme (`hud/theme.rs`) deliberately carries no green,
/// reserving green/red for meaning (matches the reticle penetration verdict palette).
pub(super) const DELTA_GOOD: [f32; 4] = [0.35, 0.85, 0.40, 0.95];
pub(super) const DELTA_BAD: [f32; 4] = [0.90, 0.30, 0.25, 0.95];

/// Screen centre of option `row` (0 = nearest the strip) in the list for the slot at `slot_index`.
pub(super) fn option_row_center(slot_index: usize, row: usize) -> [f32; 2] {
    let x = module_slot_center(slot_index)[0].clamp(-0.72, 0.72);
    [x, OPTION_LIST_BASE_Y + row as f32 * OPTION_ROW_PITCH]
}

pub(super) fn ammo_slot_center(i: usize) -> [f32; 2] {
    [AMMO_START_X + i as f32 * SLOT_STEP, LOADOUT_Y]
}

// Ammo-rack count editor: a − / + zone pair inside the bottom band of each ammo slot, flanking
// the slot's round count. They hit-test BEFORE the slot's select area, so clicking a zone edits
// the fill without switching the loaded round.
pub(super) const AMMO_ADJUST_HALF: [f32; 2] = [0.013, 0.020];
const AMMO_ADJUST_DX: f32 = 0.028;
const AMMO_ADJUST_DY: f32 = -0.036;

/// The (−, +) zone centres for ammo slot `i`.
pub(super) fn ammo_adjust_centers(i: usize) -> ([f32; 2], [f32; 2]) {
    let c = ammo_slot_center(i);
    ([c[0] - AMMO_ADJUST_DX, c[1] + AMMO_ADJUST_DY], [c[0] + AMMO_ADJUST_DX, c[1] + AMMO_ADJUST_DY])
}

/// Centre of the rack total readout ("N/CAP"), under the ammo slot group between the loadout
/// strip and the carousel.
pub(super) fn ammo_total_center() -> [f32; 2] {
    [AMMO_START_X + SLOT_STEP, LOADOUT_Y - SLOT_HALF[1] - 0.035]
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

/// The garage's rects are the HUD's rects — one hit test for every clickable surface in the
/// client, beside the primitives that draw them.
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

// Tech tree view: ERA is the primary axis — the design's three eras replace tier spread, and the
// tree is where that axis is explained. Three horizontal era bands read chronologically top to
// bottom (Era I 1939-42 → Era III 1946-55); each band holds its playable vehicles as a row of
// nodes tagged with their nation. Era I is deliberately drawn EMPTY — the reserved roadmap
// bracket says "this axis grows" louder than hiding it would.
pub(super) const TREE_PANEL_CENTER: [f32; 2] = [0.0, 0.30];
pub(super) const TREE_PANEL_HALF: [f32; 2] = [0.82, 0.36];
/// Left edge where each band's era label + years print.
pub(super) const TREE_ERA_LABEL_X: f32 = -0.76;
// The node row's usable band: left edge clear of the era-label column, right edge inside the
// panel (air before its chamfer). A band with few nodes keeps the roomy default node width —
// the numbers below reproduce the old fixed layout exactly for up to four nodes — while a
// crowded band derives a narrower node and pitch from its OWN count, so the last node sits
// inside the panel by construction. The 5-vehicle Era II row used to print its fifth node
// centred at x 0.90: past the panel edge (0.82) and clipped by the screen.
const TREE_ROW_LEFT: f32 = -0.48;
const TREE_ROW_RIGHT: f32 = 0.78;
const TREE_NODE_GAP: f32 = 0.03;
const TREE_NODE_MAX_HALF_X: f32 = 0.14;
const TREE_NODE_HALF_Y: f32 = 0.052;

/// Number of playable vehicles in `era`'s band.
pub(super) fn tree_band_len(era: game_core::Era) -> usize {
    era.playable().count()
}

/// Node half-extents in `era`'s band — every node in a band shares one size, derived from the
/// band's population so the whole row always fits the panel.
pub(super) fn tree_node_half(era: game_core::Era) -> [f32; 2] {
    let count = tree_band_len(era).max(1) as f32;
    let width = ((TREE_ROW_RIGHT - TREE_ROW_LEFT - (count - 1.0) * TREE_NODE_GAP) / count)
        .min(2.0 * TREE_NODE_MAX_HALF_X);
    [width / 2.0, TREE_NODE_HALF_Y]
}
pub(super) const TREE_CLOSE_CENTER: [f32; 2] = [0.86, 0.80];
pub(super) const TREE_CLOSE_HALF: [f32; 2] = [0.06, 0.04];

/// The band centre-line y for an era (chronological, oldest on top).
pub(super) fn tree_era_y(era: game_core::Era) -> f32 {
    match era {
        game_core::Era::EarlyWar => 0.52,
        game_core::Era::LateWar => 0.30,
        game_core::Era::ColdWar => 0.08,
    }
}

/// Centre of the `col`-th vehicle node in `era`'s band.
pub(super) fn tree_node_center(era: game_core::Era, col: usize) -> [f32; 2] {
    let half_x = tree_node_half(era)[0];
    let pitch = 2.0 * half_x + TREE_NODE_GAP;
    [TREE_ROW_LEFT + half_x + col as f32 * pitch, tree_era_y(era)]
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
    fn every_tree_node_of_the_live_fleet_stays_inside_the_panel() {
        // The defect this locks: the 5-vehicle Era II band printed its fifth node past the
        // panel and the screen. Node size and pitch now derive from each band's population,
        // so this holds for the CURRENT fleet by construction — and keeps holding as W2/W4
        // content lands more vehicles per era.
        for era in game_core::Era::ALL {
            let half = tree_node_half(era);
            for col in 0..tree_band_len(era) {
                let center = tree_node_center(era, col);
                assert!(
                    center[0] - half[0] >= TREE_PANEL_CENTER[0] - TREE_PANEL_HALF[0],
                    "{era:?} col {col} leaks off the panel's left edge"
                );
                assert!(
                    center[0] + half[0] <= TREE_PANEL_CENTER[0] + TREE_PANEL_HALF[0],
                    "{era:?} col {col} leaks off the panel's right edge"
                );
            }
            // Neighbours never overlap: the pitch always exceeds one node's width.
            if tree_band_len(era) >= 2 {
                let gap = tree_node_center(era, 1)[0] - tree_node_center(era, 0)[0];
                assert!(gap >= 2.0 * half[0] + 0.01, "{era:?} nodes overlap");
            }
            // A roomy band keeps the roomy node — no needless shrink below five vehicles.
            if tree_band_len(era) <= 4 {
                assert_eq!(half[0], 0.14, "{era:?} shrank a band that fits");
            }
        }
    }

    #[test]
    fn the_scroll_arrows_clear_the_widest_carousel_window() {
        // The arrows only appear when the roster overflows — which is exactly when the window
        // is at its widest. At the old x 0.52 the right arrow overlapped the eighth cell by
        // 0.021 clip: the control and the content it scrolls fought for the same pixels.
        let (left, right) = carousel_arrows();
        let first = carousel_cell_center(0, CAR_VISIBLE);
        let last = carousel_cell_center(CAR_VISIBLE - 1, CAR_VISIBLE);
        assert!(
            right[0] - CAR_ARROW_HALF[0] > last[0] + CAR_HALF[0] + 0.005,
            "the right arrow clips the outermost cell"
        );
        assert!(
            left[0] + CAR_ARROW_HALF[0] < first[0] - CAR_HALF[0] - 0.005,
            "the left arrow clips the outermost cell"
        );
        assert!(right[0] + CAR_ARROW_HALF[0] < 1.0, "the arrow stays on screen");
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
