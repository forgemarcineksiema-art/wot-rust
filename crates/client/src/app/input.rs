use winit::event::{ElementState, KeyEvent, MouseScrollDelta};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::CursorGrabMode;

use super::{ClientApp, InputState};
use crate::{BattleCameraInput, BattleCameraMode};

const MOUSE_YAW_SENSITIVITY: f32 = 0.0035;
const MOUSE_PITCH_SENSITIVITY: f32 = 0.0030;

impl InputState {
    pub(super) fn clear_mouse_look(&mut self) {
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
    }

    pub(super) fn throttle(&self) -> f32 {
        axis(self.forward, self.back)
    }

    pub(super) fn steer(&self) -> f32 {
        axis(self.right, self.left)
    }

    pub(super) fn brake_value(&self) -> f32 {
        if self.brake { 1.0 } else { 0.0 }
    }
}

fn axis(positive: bool, negative: bool) -> f32 {
    f32::from(u8::from(positive)) - f32::from(u8::from(negative))
}

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
                PhysicalKey::Code(KeyCode::Digit6) => self.select_garage_index(5),
                PhysicalKey::Code(KeyCode::Digit7) => self.select_garage_index(6),
                PhysicalKey::Code(KeyCode::Enter) => self.confirm_garage_selection(),
                PhysicalKey::Code(KeyCode::Escape) => self.garage.close_if_started(),
                _ if !self.garage.has_started() => {}
                _ => self.on_driving_keyboard(event, pressed),
            }
            return;
        }
        self.on_driving_keyboard(event, pressed);
    }

    fn select_garage_index(&mut self, index: usize) {
        if let Some(vehicle) = game_core::VehicleKind::ALL.get(index).copied() {
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
                self.input.free_look = pressed;
                if !pressed {
                    self.desired_aim.set_yaw(self.camera_controller.orbit_yaw_rad());
                }
            }
            PhysicalKey::Code(KeyCode::Space) if pressed => self.input.fire_pending = true,
            PhysicalKey::Code(KeyCode::KeyG) if pressed && self.garage.has_started() => {
                self.open_garage();
            }
            PhysicalKey::Code(KeyCode::Digit1) if pressed => {
                self.camera_controller.set_mode(BattleCameraMode::ThirdPerson);
            }
            PhysicalKey::Code(KeyCode::Digit2) if pressed => {
                self.camera_controller.set_mode(BattleCameraMode::Sniper);
            }
            PhysicalKey::Code(KeyCode::Escape) if pressed => self.set_cursor_captured(false),
            _ => {}
        }
    }

    pub(super) fn on_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        let scroll = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(position) => position.y as f32 / 60.0,
        };
        self.camera_controller.apply_input(BattleCameraInput {
            orbit_yaw_delta_rad: 0.0,
            pitch_delta_rad: 0.0,
            zoom_delta_m: -scroll * 0.8,
        });
    }

    pub(super) fn apply_mouse_look(&mut self) {
        if !self.garage.has_started() || self.garage.is_open() {
            self.input.clear_mouse_look();
            return;
        }
        let (dx, dy) = (self.input.mouse_dx, self.input.mouse_dy);
        self.input.clear_mouse_look();
        // Mouse-right (dx > 0) must look right; +orbit_yaw points toward world +X = screen
        // left, so negate it. Moving the mouse forward tilts the view up.
        self.camera_controller.apply_input(BattleCameraInput {
            orbit_yaw_delta_rad: -dx * MOUSE_YAW_SENSITIVITY,
            pitch_delta_rad: dy * MOUSE_PITCH_SENSITIVITY,
            zoom_delta_m: 0.0,
        });
        if !self.input.free_look {
            self.desired_aim.set_yaw(self.camera_controller.orbit_yaw_rad());
            self.desired_aim.apply_pitch_delta(dy * MOUSE_PITCH_SENSITIVITY);
        }
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
    fn mouse_look_updates_desired_pitch_when_free_look_is_off() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
        app.input.mouse_dy = 100.0;

        app.apply_mouse_look();

        assert!(app.desired_aim.pitch_rad() > 0.0);
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
