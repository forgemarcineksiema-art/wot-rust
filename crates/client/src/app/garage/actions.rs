//! The garage orbit/inspection camera and the `ClientApp` glue that turns cursor clicks into
//! selection, fitting edits, Battle, or camera drag. Kept apart from the state core in
//! [`super`] for reviewability; both operate on the same private [`GarageState`] fields.

use game_core::VehicleKind;
use glam::Vec3;
use renderer_api::Camera;

use super::{GarageHit, GarageState};
use crate::app::ClientApp;
use crate::garage_scene::hangar_camera_pivot;

// Orbit camera limits.
const MIN_PITCH: f32 = -0.05;
const MAX_PITCH: f32 = 1.20;
const MIN_DISTANCE: f32 = 6.0;
const MAX_DISTANCE: f32 = 24.0;
const ORBIT_SENSITIVITY: f32 = 0.005;
const ZOOM_STEP_M: f32 = 1.2;

impl GarageState {
    pub(in crate::app) fn orbit_camera(&self) -> Camera {
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

    pub(in crate::app) fn apply_drag(&mut self, dx: f32, dy: f32) {
        if !self.dragging {
            return;
        }
        self.orbit_yaw += dx * ORBIT_SENSITIVITY;
        self.orbit_pitch = (self.orbit_pitch - dy * ORBIT_SENSITIVITY).clamp(MIN_PITCH, MAX_PITCH);
    }

    pub(in crate::app) fn apply_zoom(&mut self, notches: f32) {
        self.orbit_distance =
            (self.orbit_distance - notches * ZOOM_STEP_M).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    pub(in crate::app) fn set_cursor(&mut self, clip: [f32; 2]) {
        self.cursor_clip = clip;
    }
}

impl ClientApp {
    pub(in crate::app) fn open_garage(&mut self) {
        self.garage.open();
        self.input.clear_mouse_look();
        self.set_cursor_captured(false);
    }

    pub(in crate::app) fn select_garage_vehicle(&mut self, vehicle: VehicleKind) {
        self.garage.select_vehicle(vehicle);
    }

    /// Route a left-button press in the garage to selection, fitting, Battle, or orbiting.
    pub(in crate::app) fn garage_primary_press(&mut self) {
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

    pub(in crate::app) fn garage_primary_release(&mut self) {
        self.garage.end_drag();
    }

    pub(in crate::app) fn confirm_garage_selection(&mut self) {
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
    fn drag_and_zoom_stay_clamped() {
        let mut garage = GarageState::default();
        garage.begin_drag();
        garage.apply_drag(0.0, -100_000.0);
        assert!(garage.orbit_pitch <= MAX_PITCH + 1.0e-6);
        garage.apply_zoom(1_000.0);
        assert!(garage.orbit_distance >= MIN_DISTANCE - 1.0e-6);
    }
}
