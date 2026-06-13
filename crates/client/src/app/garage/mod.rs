mod draft;
mod overlay;

use game_core::{TankSpec, VehicleKind};
use glam::Vec3;
use renderer_api::Camera;

use self::draft::{FitSlot, LoadoutDraft};
use super::ClientApp;
use crate::garage_scene::hangar_camera_pivot;

// Orbit camera limits.
const MIN_PITCH: f32 = -0.05;
const MAX_PITCH: f32 = 1.20;
const MIN_DISTANCE: f32 = 6.0;
const MAX_DISTANCE: f32 = 24.0;
const ORBIT_SENSITIVITY: f32 = 0.005;
const ZOOM_STEP_M: f32 = 1.2;

/// The fitting panel currently shown on the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GarageTab {
    Modules,
    Ammo,
    Crew,
}

impl GarageTab {
    pub(super) const ALL: [GarageTab; 3] = [GarageTab::Modules, GarageTab::Ammo, GarageTab::Crew];

    pub(super) fn label(self) -> &'static str {
        match self {
            GarageTab::Modules => "MODULES",
            GarageTab::Ammo => "AMMO",
            GarageTab::Crew => "CREW",
        }
    }
}

/// What a left-button press in the garage landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GarageHit {
    /// A vehicle row in the tech-tree list.
    Vehicle(usize),
    /// A fitting-panel tab.
    Tab(GarageTab),
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
    active_tab: GarageTab,
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
            active_tab: GarageTab::Modules,
            draft: LoadoutDraft::for_vehicle(VehicleKind::ALL[0]),
            orbit_yaw: 2.4,
            orbit_pitch: 0.22,
            orbit_distance: 13.0,
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
        VehicleKind::ALL[self.selected_index]
    }

    pub(super) fn select_vehicle(&mut self, vehicle: VehicleKind) {
        if let Some(index) = VehicleKind::ALL.iter().position(|kind| *kind == vehicle) {
            self.select_index(index);
        }
    }

    pub(super) fn select_index(&mut self, index: usize) {
        if index < VehicleKind::ALL.len() && index != self.selected_index {
            self.selected_index = index;
            // A different vehicle starts from its own stock loadout, ammo, and crew.
            self.draft = LoadoutDraft::for_vehicle(self.selected_vehicle());
        }
    }

    pub(super) fn cycle(&mut self, delta: isize) {
        let len = VehicleKind::ALL.len() as isize;
        let index = (self.selected_index as isize + delta).rem_euclid(len) as usize;
        self.select_index(index);
    }

    pub(super) fn set_tab(&mut self, tab: GarageTab) {
        self.active_tab = tab;
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

    pub(super) fn active_tab(&self) -> GarageTab {
        self.active_tab
    }

    pub(super) fn draft(&self) -> &LoadoutDraft {
        &self.draft
    }

    pub(super) fn cursor_clip(&self) -> [f32; 2] {
        self.cursor_clip
    }

    // --- orbit camera ---------------------------------------------------------------------

    pub(super) fn orbit_camera(&self) -> Camera {
        let pivot = hangar_camera_pivot();
        let horizontal = self.orbit_distance * self.orbit_pitch.cos();
        let eye = pivot
            + Vec3::new(
                horizontal * self.orbit_yaw.sin(),
                self.orbit_distance * self.orbit_pitch.sin(),
                horizontal * self.orbit_yaw.cos(),
            );
        Camera { eye: eye.to_array(), target: pivot.to_array(), vertical_fov_degrees: 42.0 }
    }

    pub(super) fn begin_drag(&mut self) {
        self.dragging = true;
    }

    pub(super) fn end_drag(&mut self) {
        self.dragging = false;
    }

    pub(super) fn apply_drag(&mut self, dx: f32, dy: f32) {
        if !self.dragging {
            return;
        }
        self.orbit_yaw += dx * ORBIT_SENSITIVITY;
        self.orbit_pitch = (self.orbit_pitch - dy * ORBIT_SENSITIVITY).clamp(MIN_PITCH, MAX_PITCH);
    }

    pub(super) fn apply_zoom(&mut self, notches: f32) {
        self.orbit_distance =
            (self.orbit_distance - notches * ZOOM_STEP_M).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    pub(super) fn set_cursor(&mut self, clip: [f32; 2]) {
        self.cursor_clip = clip;
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

impl ClientApp {
    pub(super) fn open_garage(&mut self) {
        self.garage.open();
        self.input.clear_mouse_look();
        self.set_cursor_captured(false);
    }

    pub(super) fn select_garage_vehicle(&mut self, vehicle: VehicleKind) {
        self.garage.select_vehicle(vehicle);
    }

    /// Route a left-button press in the garage to selection, fitting, Battle, or orbiting.
    pub(super) fn garage_primary_press(&mut self) {
        match self.garage.hit_test() {
            GarageHit::Vehicle(index) => self.garage.select_index(index),
            GarageHit::Tab(tab) => self.garage.set_tab(tab),
            GarageHit::ModuleCycle(slot, dir) => self.garage.cycle_module(slot, dir),
            GarageHit::AmmoSelect(index) => self.garage.set_ammo(index),
            GarageHit::CrewProf(dir) => self.garage.adjust_proficiency(dir),
            GarageHit::Battle => self.confirm_garage_selection(),
            GarageHit::Scene => self.garage.begin_drag(),
        }
    }

    pub(super) fn garage_primary_release(&mut self) {
        self.garage.end_drag();
    }

    pub(super) fn confirm_garage_selection(&mut self) {
        let spec = self.garage.confirm();
        let display_name = spec.name.clone();
        let snapshot = self.local_server.change_player_vehicle_with_spec(spec.clone());
        self.player_tank = self.local_server.player_tank();
        self.predictor.reset_to_spec(&spec);
        self.render_state = crate::InterpolatedBattleState::default();
        self.input.fire_pending = false;
        self.input.clear_mouse_look();
        self.accept_and_sync(snapshot);
        self.set_cursor_captured(true);
        if let Some(window) = &self.window {
            window.set_title(&format!("WOT Rust Prototype - {display_name}"));
        }
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
        let stock = VehicleKind::ALL[1].spec();
        assert_eq!(garage.draft().assembled_spec().gun.shell, stock.gun.shell);
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
        assert!((spec.gun.reload_seconds - green).abs() < 1.0e-6, "confirmed spec carries the edit");
    }

    #[test]
    fn drag_and_zoom_stay_clamped() {
        let mut garage = GarageState::default();
        garage.begin_drag();
        garage.apply_drag(0.0, -100_000.0);
        assert!(garage.orbit_pitch <= MAX_PITCH + 1.0e-6);
        garage.apply_zoom(1_000.0);
        assert!(garage.orbit_distance >= MIN_DISTANCE - 1.0e-6);
    }
}
