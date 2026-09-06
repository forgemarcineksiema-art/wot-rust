//! The garage's words and window arithmetic: the map row's words, the slots' words and icons,
//! and the carousel's window. Everything the garage draws is laid out in `u` by `screen.rs`
//! and `tree.rs`.

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
}
