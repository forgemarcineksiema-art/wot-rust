mod actions;
mod draft;
mod layout;
mod overlay;
mod panels;

use game_core::{TankSpec, VehicleKind};

pub(crate) use self::draft::{FitSlot, LoadoutDraft};

/// What a left-button press in the garage landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GarageHit {
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
    /// Empty scene — start orbiting the camera.
    Scene,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct GarageState {
    open: bool,
    started: bool,
    selected_index: usize,
    draft: LoadoutDraft,
    orbit_yaw: f32,
    orbit_pitch: f32,
    orbit_distance: f32,
    cursor_clip: [f32; 2],
    dragging: bool,
}

impl Default for GarageState {
    fn default() -> Self {
        Self {
            open: true,
            started: false,
            selected_index: 0,
            draft: LoadoutDraft::for_vehicle(VehicleKind::PLAYABLE[0]),
            orbit_yaw: 2.4,
            orbit_pitch: 0.12,
            orbit_distance: 12.0,
            cursor_clip: [2.0, 2.0],
            dragging: false,
        }
    }
}

impl GarageState {
    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    pub(super) fn has_started(&self) -> bool {
        self.started
    }

    pub(super) fn open(&mut self) {
        self.open = true;
        self.dragging = false;
    }

    pub(super) fn close_if_started(&mut self) {
        if self.started {
            self.open = false;
        }
    }

    pub(super) fn selected_vehicle(&self) -> VehicleKind {
        VehicleKind::PLAYABLE[self.selected_index]
    }

    pub(super) fn select_vehicle(&mut self, vehicle: VehicleKind) {
        if let Some(index) = VehicleKind::PLAYABLE.iter().position(|kind| *kind == vehicle) {
            self.select_index(index);
        }
    }

    pub(super) fn select_index(&mut self, index: usize) {
        if index < VehicleKind::PLAYABLE.len() && index != self.selected_index {
            self.selected_index = index;
            // A different vehicle starts from its own stock loadout, ammo, and crew.
            self.draft = LoadoutDraft::for_vehicle(self.selected_vehicle());
        }
    }

    pub(super) fn cycle(&mut self, delta: isize) {
        let len = VehicleKind::PLAYABLE.len() as isize;
        let index = (self.selected_index as isize + delta).rem_euclid(len) as usize;
        self.select_index(index);
    }

    pub(super) fn cycle_module(&mut self, slot: FitSlot, dir: isize) {
        self.draft.cycle_module(slot, dir);
    }

    pub(super) fn set_ammo(&mut self, index: usize) {
        self.draft.set_ammo(index);
    }

    pub(super) fn adjust_proficiency(&mut self, dir: isize) {
        self.draft.adjust_proficiency(dir);
    }

    /// Commit the edited loadout: lock the garage and hand back the assembled spec to install.
    pub(super) fn confirm(&mut self) -> TankSpec {
        self.started = true;
        self.open = false;
        self.dragging = false;
        self.draft.assembled_spec()
    }

    // --- accessors for the overlay (and hit test) -----------------------------------------

    pub(super) fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub(super) fn draft(&self) -> &LoadoutDraft {
        &self.draft
    }

    pub(super) fn cursor_clip(&self) -> [f32; 2] {
        self.cursor_clip
    }

    /// Length scale for the parked tank's gun submesh so swapping guns visibly changes the
    /// silhouette: the ratio of the installed barrel to the vehicle's stock barrel (the baked mesh
    /// represents the stock gun).
    pub(super) fn gun_silhouette_scale(&self) -> f32 {
        let stock = self.selected_vehicle().stock_barrel_length_m();
        if stock <= 0.0 {
            return 1.0;
        }
        (self.draft.gun_barrel_length() / stock).clamp(0.6, 1.6)
    }

    pub(super) fn overlay_vertices(&self, aspect: f32) -> Vec<renderer_api::HudVertex> {
        if !self.open {
            return Vec::new();
        }
        overlay::build(self, aspect)
    }

    pub(super) fn hit_test(&self) -> GarageHit {
        overlay::hit_test(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selecting_a_new_vehicle_resets_the_draft_to_its_stock_loadout() {
        let mut garage = GarageState::default();
        garage.select_index(1);
        garage.cycle_module(FitSlot::Gun, 1);
        // Switching vehicles and back must discard the edit.
        garage.select_index(2);
        garage.select_index(1);
        let stock = VehicleKind::PLAYABLE[1].spec();
        assert_eq!(garage.draft().assembled_spec().gun.shell, stock.gun.shell);
    }

    #[test]
    fn garage_roster_starts_on_t54_and_rejects_t55a_legacy_clone() {
        let mut garage = GarageState::default();
        assert_eq!(garage.selected_vehicle(), VehicleKind::T54_1951);
        assert!(!VehicleKind::PLAYABLE.contains(&VehicleKind::T55A));

        garage.select_vehicle(VehicleKind::T55A);

        assert_eq!(garage.selected_vehicle(), VehicleKind::T54_1951);
    }

    #[test]
    fn confirm_returns_the_edited_spec_and_closes_the_garage() {
        let mut garage = GarageState::default();
        garage.select_vehicle(VehicleKind::TigerII);
        garage.adjust_proficiency(-1);
        let green = garage.draft().assembled_spec().gun.reload_seconds;
        let spec = garage.confirm();
        assert!(!garage.is_open() && garage.has_started());
        assert_eq!(spec.kind, VehicleKind::TigerII);
        assert!(
            (spec.gun.reload_seconds - green).abs() < 1.0e-6,
            "confirmed spec carries the edit"
        );
    }
}
