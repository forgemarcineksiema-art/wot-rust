use winit::event::{ElementState, KeyEvent, MouseScrollDelta};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::CursorGrabMode;

use super::ClientApp;
use crate::{BattleCameraInput, BattleCameraMode};

const MOUSE_YAW_SENSITIVITY: f32 = 0.0035;
const MOUSE_PITCH_SENSITIVITY: f32 = 0.0030;

impl ClientApp {
    pub(super) fn on_keyboard(&mut self, event: &KeyEvent) {
        let pressed = event.state == ElementState::Pressed;
        if pressed && self.garage.is_open() {
            match event.physical_key {
                PhysicalKey::Code(KeyCode::ArrowLeft) => self.garage.cycle(-1),
                PhysicalKey::Code(KeyCode::ArrowRight) => self.garage.cycle(1),
                PhysicalKey::Code(KeyCode::Digit1) => self.select_garage_index(0),
                PhysicalKey::Code(KeyCode::Digit2) => self.select_garage_index(1),
                PhysicalKey::Code(KeyCode::Digit3) => self.select_garage_index(2),
                PhysicalKey::Code(KeyCode::Digit4) => self.select_garage_index(3),
                PhysicalKey::Code(KeyCode::Digit5) => self.select_garage_index(4),
                PhysicalKey::Code(KeyCode::Enter) => self.confirm_garage_selection(),
                PhysicalKey::Code(KeyCode::Escape) => self.garage.close_if_started(),
                // Keyboard loadout editing: focus + cycle + ammo + crew.
                PhysicalKey::Code(KeyCode::BracketLeft) => self.garage.focus_adjacent(-1),
                PhysicalKey::Code(KeyCode::BracketRight) => self.garage.focus_adjacent(1),
                PhysicalKey::Code(KeyCode::KeyQ) => self.garage.cycle_focused(-1),
                PhysicalKey::Code(KeyCode::KeyE) => self.garage.cycle_focused(1),
                PhysicalKey::Code(KeyCode::KeyZ) => self.garage.set_ammo(0),
                PhysicalKey::Code(KeyCode::KeyX) => self.garage.set_ammo(1),
                PhysicalKey::Code(KeyCode::KeyC) => self.garage.set_ammo(2),
                PhysicalKey::Code(KeyCode::Minus) => self.garage.adjust_proficiency(-1),
                PhysicalKey::Code(KeyCode::Equal) => self.garage.adjust_proficiency(1),
                PhysicalKey::Code(KeyCode::KeyT) => match self.garage.view() {
                    super::garage::GarageView::Hangar => self.garage.open_tech_tree(),
                    super::garage::GarageView::TechTree => self.garage.close_tech_tree(),
                },
                _ if !self.garage.has_started() => {}
                _ => self.on_driving_keyboard(event, pressed),
            }
            return;
        }
        self.on_driving_keyboard(event, pressed);
    }

    fn select_garage_index(&mut self, index: usize) {
        if let Some(vehicle) = game_core::VehicleKind::PLAYABLE.get(index).copied() {
            self.select_garage_vehicle(vehicle);
        }
    }

    fn on_driving_keyboard(&mut self, event: &KeyEvent, pressed: bool) {
        match event.physical_key {
            PhysicalKey::Code(KeyCode::KeyW | KeyCode::ArrowUp) => self.input.forward = pressed,
            PhysicalKey::Code(KeyCode::KeyS | KeyCode::ArrowDown) => self.input.back = pressed,
            PhysicalKey::Code(KeyCode::KeyA | KeyCode::ArrowLeft) => self.input.left = pressed,
            PhysicalKey::Code(KeyCode::KeyD | KeyCode::ArrowRight) => self.input.right = pressed,
            PhysicalKey::Code(KeyCode::ShiftLeft | KeyCode::ShiftRight) => {
                self.input.brake = pressed
            }
            PhysicalKey::Code(KeyCode::AltLeft | KeyCode::AltRight) => {
                if pressed && !self.input.free_look {
                    self.begin_free_look();
                } else if !pressed && self.input.free_look {
                    self.end_free_look();
                }
            }
            PhysicalKey::Code(KeyCode::Space) if pressed => self.input.fire_pending = true,
            PhysicalKey::Code(KeyCode::KeyG) if pressed && self.garage.has_started() => {
                self.open_garage();
            }
            PhysicalKey::Code(KeyCode::Digit1) if pressed => {
                self.camera_controller.set_mode(BattleCameraMode::ThirdPerson);
            }
            PhysicalKey::Code(KeyCode::Digit2) if pressed => self.enter_sniper_mode(),
            PhysicalKey::Code(KeyCode::Escape) if pressed => self.set_cursor_captured(false),
            _ => {}
        }
    }

    pub(super) fn on_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        let lines = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(position) => position.y as f32 / 60.0,
        };
        if self.garage.is_open() {
            self.garage.apply_zoom(lines);
            return;
        }
        if !self.garage.has_started() {
            return;
        }
        // High-resolution wheels and touchpads deliver one notch as many fractional events;
        // accumulate to whole notches so one gesture cannot step the sniper ladder repeatedly.
        self.input.wheel_pending_lines += lines;
        while self.input.wheel_pending_lines.abs() >= 1.0 {
            let notch = self.input.wheel_pending_lines.signum();
            self.input.wheel_pending_lines -= notch;
            let mode_before = self.camera_controller.mode();
            self.camera_controller.apply_input(BattleCameraInput {
                orbit_yaw_delta_rad: 0.0,
                pitch_delta_rad: 0.0,
                zoom_delta_m: -notch * 0.8,
            });
            // Scrolling through the shortest boom hands over to sniper; align the view to the gun.
            if mode_before == BattleCameraMode::ThirdPerson
                && self.camera_controller.mode() == BattleCameraMode::Sniper
            {
                self.sync_sniper_entry();
            }
        }
    }

    pub(super) fn enter_sniper_mode(&mut self) {
        if self.camera_controller.mode() == BattleCameraMode::Sniper {
            return;
        }
        self.camera_controller.set_mode(BattleCameraMode::Sniper);
        self.sync_sniper_entry();
    }

    /// Start the sniper view where the gun actually points, so entering the mode never jumps
    /// the sight to a stale pitch; the view tracks the mouse from there.
    fn sync_sniper_entry(&mut self) {
        self.desired_aim =
            crate::aim::DesiredAim::new(self.desired_aim.yaw_rad(), self.predictor.gun_pitch());
    }

    fn begin_free_look(&mut self) {
        self.input.free_look = true;
        self.input.free_look_return_pitch = Some(self.camera_controller.pitch_rad());
    }

    /// Free look never moves the aim: on release the camera returns to the sight lane instead
    /// of the turret swinging to wherever the player glanced.
    fn end_free_look(&mut self) {
        self.input.free_look = false;
        self.camera_controller.set_orbit_yaw(self.desired_aim.yaw_rad());
        if let Some(pitch) = self.input.free_look_return_pitch.take() {
            self.camera_controller.set_pitch(pitch);
        }
    }

    pub(super) fn apply_mouse_look(&mut self) {
        if self.garage.is_open() {
            // In the garage, mouse motion orbits the inspection camera (only while dragging).
            let (dx, dy) = (self.input.mouse_dx, self.input.mouse_dy);
            self.input.clear_mouse_look();
            self.garage.apply_drag(dx, dy);
            return;
        }
        if !self.garage.has_started() {
            self.input.clear_mouse_look();
            return;
        }
        let (dx, dy) = (self.input.mouse_dx, self.input.mouse_dy);
        self.input.clear_mouse_look();
        // Mouse-right (dx > 0) must look right; +orbit_yaw points toward world +X = screen
        // left, so negate it. The FOV ratio slows the look exactly as much as zoom magnifies it.
        let scale = self.camera_controller.look_sensitivity_scale();
        let yaw_delta = -dx * MOUSE_YAW_SENSITIVITY * scale;
        let pitch_delta = dy * MOUSE_PITCH_SENSITIVITY * scale;
        if self.input.free_look {
            // Free look orbits only the camera; `end_free_look` restores it to the aim.
            self.camera_controller.apply_input(BattleCameraInput {
                orbit_yaw_delta_rad: yaw_delta,
                pitch_delta_rad: pitch_delta,
                zoom_delta_m: 0.0,
            });
            return;
        }
        if self.camera_controller.mode() == BattleCameraMode::Sniper {
            // The sniper view *is* the aim. Mouse forward looks up, mouse back looks down —
            // the same vertical sense as the third-person camera (camera pitch raises the eye
            // to look down; gun pitch raises the muzzle to look up, hence the sign flip).
            self.desired_aim.set_yaw(self.desired_aim.yaw_rad() + yaw_delta);
            self.desired_aim.apply_pitch_delta(-pitch_delta);
            self.camera_controller.set_orbit_yaw(self.desired_aim.yaw_rad());
            return;
        }
        self.camera_controller.apply_input(BattleCameraInput {
            orbit_yaw_delta_rad: yaw_delta,
            pitch_delta_rad: pitch_delta,
            zoom_delta_m: 0.0,
        });
        self.desired_aim.set_yaw(self.camera_controller.orbit_yaw_rad());
    }

    /// Map a window-pixel cursor position into clip space for the garage UI hit test.
    pub(super) fn on_cursor_moved(&mut self, x: f32, y: f32) {
        if !self.garage.is_open() {
            return;
        }
        let (w, h) = self.viewport;
        let clip_x = (x / w as f32) * 2.0 - 1.0;
        let clip_y = 1.0 - (y / h as f32) * 2.0;
        self.garage.set_cursor([clip_x, clip_y]);
    }

    pub(super) fn set_cursor_captured(&self, captured: bool) {
        let Some(window) = &self.window else {
            return;
        };
        if captured {
            let _ = window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
        } else {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
        }
        window.set_cursor_visible(!captured);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_look_updates_aim_yaw_when_free_look_is_off() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.camera_controller.set_orbit_yaw(0.0);
        app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
        app.input.mouse_dx = 100.0;

        app.apply_mouse_look();

        assert!((app.desired_aim.yaw_rad() - app.camera_controller.orbit_yaw_rad()).abs() < 1.0e-5);
        assert!(app.desired_aim.yaw_rad() < 0.0);
    }

    #[test]
    fn third_person_mouse_look_leaves_desired_pitch_to_the_sight_solver() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
        let pitch_before = app.camera_controller.pitch_rad();
        app.input.mouse_dy = 100.0;

        app.apply_mouse_look();

        // The TPP gun follows the screen-centre ray; the raw desired pitch must not drift with
        // the mouse, or entering sniper later starts from accumulated junk.
        assert_eq!(app.desired_aim.pitch_rad(), 0.0);
        assert!(app.camera_controller.pitch_rad() > pitch_before, "mouse back tilts the boom");
    }

    #[test]
    fn sniper_mouse_back_aims_down_matching_the_third_person_sense() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.enter_sniper_mode();
        app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
        app.input.mouse_dy = 100.0; // mouse pulled back/down

        app.apply_mouse_look();

        // In third person, mouse back looks down; the sniper view must agree, and gun pitch
        // raises the muzzle, so looking down means a *negative* desired pitch.
        assert!(app.desired_aim.pitch_rad() < 0.0);
    }

    #[test]
    fn sniper_mouse_look_slows_with_magnification() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.enter_sniper_mode();
        let wide_fov = app.camera_controller.sniper_fov_degrees();
        app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
        app.input.mouse_dx = 100.0;
        app.apply_mouse_look();
        let wide_turn = app.desired_aim.yaw_rad().abs();

        // Step the zoom to the narrowest FOV and repeat the identical mouse motion.
        while app.camera_controller.sniper_fov_degrees() > 3.0 {
            app.camera_controller.apply_input(BattleCameraInput {
                orbit_yaw_delta_rad: 0.0,
                pitch_delta_rad: 0.0,
                zoom_delta_m: -0.8,
            });
        }
        app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
        app.camera_controller.set_orbit_yaw(0.0);
        app.input.mouse_dx = 100.0;
        app.apply_mouse_look();
        let narrow_turn = app.desired_aim.yaw_rad().abs();

        let expected_ratio = 3.0 / wide_fov;
        assert!(
            (narrow_turn / wide_turn - expected_ratio).abs() < 1.0e-3,
            "look speed must scale with FOV: {narrow_turn} vs {wide_turn}"
        );
    }

    #[test]
    fn free_look_release_returns_the_camera_to_the_aim() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.camera_controller.set_orbit_yaw(0.5);
        app.desired_aim = crate::aim::DesiredAim::new(0.5, 0.0);
        let pitch_before = app.camera_controller.pitch_rad();

        app.begin_free_look();
        app.input.mouse_dx = 200.0;
        app.input.mouse_dy = 100.0;
        app.apply_mouse_look();
        assert!((app.camera_controller.orbit_yaw_rad() - 0.5).abs() > 0.1, "free look orbits");

        app.end_free_look();

        // The glance must not move the gun: aim is untouched, the camera snaps back to it.
        assert_eq!(app.desired_aim.yaw_rad(), 0.5);
        assert_eq!(app.camera_controller.orbit_yaw_rad(), 0.5);
        assert_eq!(app.camera_controller.pitch_rad(), pitch_before);
    }

    #[test]
    fn mouse_wheel_is_ignored_until_battle_control_starts() {
        let mut app = ClientApp::new();
        let distance_before = app.camera_controller.distance_m();

        app.on_mouse_wheel(MouseScrollDelta::LineDelta(0.0, 1.0));

        assert_eq!(app.camera_controller.distance_m(), distance_before);
    }

    #[test]
    fn fractional_wheel_events_accumulate_to_whole_notches() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        let distance_before = app.camera_controller.distance_m();

        // A touchpad swipe arrives as many small pixel deltas; 3 x 20 px = exactly one notch.
        for _ in 0..3 {
            app.on_mouse_wheel(MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(
                0.0, 20.0,
            )));
        }

        let moved = distance_before - app.camera_controller.distance_m();
        assert!((moved - 0.8).abs() < 1.0e-5, "one notch moves one zoom step, got {moved}");
    }

    #[test]
    fn scrolling_past_the_shortest_boom_enters_sniper_aligned_to_the_gun() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.run_fixed_ticks(1); // seed prediction so the gun has an authoritative pitch
        app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.3);

        for _ in 0..15 {
            app.on_mouse_wheel(MouseScrollDelta::LineDelta(0.0, 1.0));
        }

        assert_eq!(app.camera_controller.mode(), BattleCameraMode::Sniper);
        assert!(
            (app.desired_aim.pitch_rad() - app.predictor.gun_pitch()).abs() < 1.0e-5,
            "the sniper view must open on the gun, not on stale desired pitch"
        );
    }

    #[test]
    fn entering_sniper_with_the_key_syncs_the_view_to_the_gun() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.run_fixed_ticks(40); // let the gun converge on the sight solution
        app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.3);

        app.enter_sniper_mode();

        assert!((app.desired_aim.pitch_rad() - app.predictor.gun_pitch()).abs() < 1.0e-5);
        assert!((app.desired_aim.pitch_rad() - 0.3).abs() > 1.0e-3, "stale pitch must not leak");
    }

    #[test]
    fn free_look_moves_camera_without_changing_aim_yaw() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.camera_controller.set_orbit_yaw(0.0);
        app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
        app.input.free_look = true;
        app.input.mouse_dx = 100.0;

        app.apply_mouse_look();

        assert!(app.camera_controller.orbit_yaw_rad() < 0.0);
        assert_eq!(app.desired_aim.yaw_rad(), 0.0);
    }

    #[test]
    fn garage_mouse_delta_is_discarded_before_battle_control_starts() {
        let mut app = ClientApp::new();
        app.camera_controller.set_orbit_yaw(0.0);
        app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
        app.input.mouse_dx = 240.0;

        app.confirm_garage_selection();
        app.apply_mouse_look();

        assert_eq!(app.input.mouse_dx, 0.0);
        assert_eq!(app.camera_controller.orbit_yaw_rad(), 0.0);
        assert_eq!(app.desired_aim.yaw_rad(), 0.0);
    }
}
