//! Garage hit-test results and view enum, split from `mod.rs` for reviewability.

use super::draft::FitSlot;

/// What a left-button press in the garage landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum GarageHit {
    /// A vehicle cell in the bottom carousel.
    Vehicle(usize),
    /// Cycle a module slot's option by `dir` (-1 / +1).
    ModuleCycle(FitSlot, isize),
    /// Select an ammo option by index.
    AmmoSelect(usize),
    /// Nudge crew proficiency by `dir`.
    CrewProf(isize),
    /// The "Battle" button.
    Battle,
    /// Open the browse-only tech tree view.
    OpenTechTree,
    /// Close the tech tree view and return to the hangar.
    CloseTechTree,
    /// Empty scene — start orbiting the camera.
    Scene,
}

/// Which garage screen is active: the hangar (vehicle + loadout editor) or the browse-only tech
/// tree. The carousel remains the primary selector; the tech tree is an organisational view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::app) enum GarageView {
    #[default]
    Hangar,
    TechTree,
}
