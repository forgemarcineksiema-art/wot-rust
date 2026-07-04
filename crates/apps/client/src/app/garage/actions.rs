//! The garage orbit/inspection camera and the `ClientApp` glue that turns cursor clicks into
//! selection, fitting edits, Battle, or camera drag. Kept apart from the state core in
//! [`super`] for reviewability; both operate on the same private [`GarageState`] fields.

use game_core::VehicleKind;
use glam::Vec3;
use renderer_api::Camera;

use super::{GarageHit, GarageState};
use crate::app::ClientApp;
use crate::scene::hangar::hangar_camera_pivot;

// Orbit camera limits.
const MIN_PITCH: f32 = -0.05;
const MAX_PITCH: f32 = 1.20;
const MIN_DISTANCE: f32 = 8.5;
const MAX_DISTANCE: f32 = 20.0;
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
        Camera { eye: eye.to_array(), target: pivot.to_array(), vertical_fov_degrees: 32.0 }
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
    /// Shift held while clicking a module slot cycles that slot backward.
    pub(in crate::app) fn garage_primary_press(&mut self) {
        let shift = self.input.shift;
        let view = self.garage.view();
        match self.garage.hit_test(shift) {
            GarageHit::Vehicle(index) => {
                self.garage.select_index(index);
                // Selecting a vehicle from the tech tree returns to the hangar view.
                if view == super::GarageView::TechTree {
                    self.garage.close_tech_tree();
                }
            }
            GarageHit::ModuleCycle(slot, dir) => self.garage.cycle_module(slot, dir),
            GarageHit::AmmoSelect(index) => self.garage.set_ammo(index),
            GarageHit::CrewProf(dir) => self.garage.adjust_proficiency(dir),
            GarageHit::Battle => self.confirm_garage_selection(),
            GarageHit::OpenTechTree => self.garage.open_tech_tree(),
            GarageHit::CloseTechTree => self.garage.close_tech_tree(),
            GarageHit::Scene => self.garage.begin_drag(),
        }
    }

    pub(in crate::app) fn garage_primary_release(&mut self) {
        self.garage.end_drag();
    }

    /// Route a right-button press in the garage. Only module slots act on it (cycling backward);
    /// every other hit is ignored so right-click never fires Battle, selects ammo, or starts a drag.
    pub(in crate::app) fn garage_secondary_press(&mut self) {
        if let GarageHit::ModuleCycle(slot, _) = self.garage.hit_test(true) {
            self.garage.cycle_module(slot, -1);
        }
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
            window.set_title(&format!("{} - {display_name}", crate::ui_strings::WINDOW_TITLE));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::layout::{BATTLE_CENTER, ammo_slot_center, module_slot_center};
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

    #[test]
    fn right_click_on_module_slot_cycles_backward() {
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::T54_1951);
        app.garage.set_cursor(module_slot_center(1)); // Gun slot
        let before = app.garage.draft().gun_barrel_length();
        app.garage_secondary_press();
        let after = app.garage.draft().gun_barrel_length();
        assert_ne!(before, after, "right-click cycles the gun backward");
    }

    #[test]
    fn right_click_on_battle_does_not_fire() {
        let mut app = ClientApp::new();
        app.garage.set_cursor(BATTLE_CENTER);
        app.garage_secondary_press();
        assert!(app.garage.is_open(), "right-click never commits to battle");
        assert!(!app.garage.has_started());
    }

    #[test]
    fn right_click_on_ammo_slot_does_not_select() {
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::T54_1951);
        let before = app.garage.draft().ammo_index();
        app.garage.set_cursor(ammo_slot_center(1));
        app.garage_secondary_press();
        assert_eq!(app.garage.draft().ammo_index(), before, "right-click does not touch ammo");
    }

    #[test]
    fn shift_left_click_cycles_backward_opposite_of_plain_click() {
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::T54_1951);
        app.garage.set_cursor(module_slot_center(1)); // Gun slot
        let stock = app.garage.draft().gun_barrel_length();

        // Plain click cycles forward to the alternate gun.
        app.garage_primary_press();
        let forward = app.garage.draft().gun_barrel_length();
        assert_ne!(stock, forward, "plain click moves off the stock gun");

        // Shift+click cycles backward, returning to the stock gun.
        app.input.set_shift(true);
        app.garage_primary_press();
        let backward = app.garage.draft().gun_barrel_length();
        assert_eq!(stock, backward, "shift+click returns to stock (opposite direction)");
    }

    #[test]
    fn selecting_vehicle_from_tech_tree_returns_to_hangar() {
        use super::super::GarageView;
        use crate::app::garage::layout::tree_node_center;
        use game_core::Nation;

        let mut app = ClientApp::new();
        app.garage.open_tech_tree();
        assert_eq!(app.garage.view(), GarageView::TechTree);

        // Click the Tiger I node (first Germany node = PLAYABLE index 1).
        app.garage.set_cursor(tree_node_center(Nation::Germany, 0));
        app.garage_primary_press();

        assert_eq!(app.garage.view(), GarageView::Hangar, "returns to hangar");
        assert_eq!(app.garage.selected_vehicle(), VehicleKind::TigerI);
    }

    #[test]
    fn close_button_in_tech_tree_returns_to_hangar() {
        use super::super::GarageView;
        use crate::app::garage::layout::TREE_CLOSE_CENTER;

        let mut app = ClientApp::new();
        app.garage.open_tech_tree();
        app.garage.set_cursor(TREE_CLOSE_CENTER);
        app.garage_primary_press();
        assert_eq!(app.garage.view(), GarageView::Hangar);
    }
}
