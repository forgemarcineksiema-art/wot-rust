//! Garage hit-test results and view enum, split from `mod.rs` for reviewability.

use super::draft::FitSlot;
use super::filter::Chip;

/// What a left-button press in the garage landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum GarageHit {
    /// A vehicle cell in the bottom carousel (absolute roster index).
    Vehicle(usize),
    /// A carousel scroll arrow: `-1` scrolls the window left, `+1` right.
    CarouselScroll(i8),
    /// Cycle a module slot's option by `dir` (-1 / +1).
    ModuleCycle(FitSlot, isize),
    /// Pick option `index` from the open module list for `slot` (installs it and closes the list).
    OptionRow(FitSlot, usize),
    /// Select an ammo option by index.
    AmmoSelect(usize),
    /// Move rounds into (`+1`) or out of (`-1`) rack slot `index` — the count editor's − / +
    /// zones under the ammo slot. Shift steps by 5.
    AmmoAdjust(usize, isize),
    /// The "Battle" button.
    Battle,
    /// The map row next to the Battle button: cycle the pre-battle map choice by `dir`
    /// (+1 plain click, -1 shift-click — the module-slot convention).
    MapCycle(i8),
    /// Open the browse-only tech tree view.
    OpenTechTree,
    /// Close the tech tree view and return to the hangar.
    CloseTechTree,
    /// Empty scene — start orbiting the camera.
    Scene,
    /// BATTLE while the hull is locked in a battle that still runs (G14): a knock, nothing more.
    Locked,
    /// Shift-click on a carousel cell (G3): compare the VEHICLE column against this hull
    /// (absolute roster index); the same cell again clears it.
    Compare(usize),
    /// A filter chip (G9): walk its ring by `dir` (+1 plain click, -1 shift-click).
    Chip(Chip, i8),
}

/// What a press on the scene took (G8): nothing, the orbit camera, or the hero's turret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::app) enum Drag {
    #[default]
    None,
    Camera,
    Turret,
}

/// Which garage screen is active: the hangar (vehicle + loadout editor) or the browse-only tech
/// tree. The carousel remains the primary selector; the tech tree is an organisational view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::app) enum GarageView {
    #[default]
    Hangar,
    TechTree,
}
