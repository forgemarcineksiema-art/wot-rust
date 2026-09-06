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
    /// One of the seven tabs on the bar (G10).
    Tab(GarageTab),
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

/// The seven tabs on the garage's bar (G10), in their order. GARAGE, TECH TREE and ARMOUR are
/// the garage's own screens; the rest open the shell's pages over the hall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum GarageTab {
    Garage,
    TechTree,
    Armour,
    Battles,
    Replays,
    Statistics,
    Settings,
}

impl GarageTab {
    pub const ALL: [GarageTab; 7] = [
        GarageTab::Garage,
        GarageTab::TechTree,
        GarageTab::Armour,
        GarageTab::Battles,
        GarageTab::Replays,
        GarageTab::Statistics,
        GarageTab::Settings,
    ];

    pub fn word(self) -> &'static str {
        use crate::ui_strings::garage as words;
        match self {
            GarageTab::Garage => words::TAB_GARAGE,
            GarageTab::TechTree => words::TAB_TECH_TREE,
            GarageTab::Armour => words::TAB_ARMOUR,
            GarageTab::Battles => words::TAB_BATTLES,
            GarageTab::Replays => words::TAB_REPLAYS,
            GarageTab::Statistics => words::TAB_STATISTICS,
            GarageTab::Settings => words::TAB_SETTINGS,
        }
    }
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
