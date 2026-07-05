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
        if pressed && self.garage.is_open() && self.garage_keyboard(event) {
            return;
        }
        self.on_driving_keyboard(event, pressed);
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
            // 1/2/3 select ammo (genre standard; the vision's ammo-rack slots). The camera
            // moved to V â€” the wheel scroll-through stays the primary camera path.
            PhysicalKey::Code(KeyCode::Digit1) if pressed => self.request_ammo_slot(0),
            PhysicalKey::Code(KeyCode::Digit2) if pressed => self.request_ammo_slot(1),
            PhysicalKey::Code(KeyCode::Digit3) if pressed => self.request_ammo_slot(2),
            PhysicalKey::Code(KeyCode::KeyV) if pressed => self.toggle_camera_mode(),
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

    /// Queue an ammo switch for the server and adopt it optimistically in the predictor, so the
    /// reticle's ballistics (muzzle velocity, drag, pen hint) answer on the same frame.
    pub(super) fn request_ammo_slot(&mut self, slot: u8) {
        self.input.pending_ammo_select = Some(slot);
        self.predictor.set_selected_ammo(slot);
    }

    /// V toggles third person <-> sniper (the wheel remains the primary camera path).
    pub(super) fn toggle_camera_mode(&mut self) {
        if self.camera_controller.mode() == BattleCameraMode::Sniper {
            self.camera_controller.set_mode(BattleCameraMode::ThirdPerson);
        } else {
            self.enter_sniper_mode();
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

    pub(super) fn begin_free_look(&mut self) {
        self.input.free_look = true;
        self.input.free_look_return_pitch = Some(self.camera_controller.pitch_rad());
    }

    /// Free look never moves the aim: on release the camera returns to the sight lane instead
    /// of the turret swinging to wherever the player glanced.
    pub(super) fn end_free_look(&mut self) {
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
            // The sniper view *is* the aim. Mouse forward looks up, mouse back looks down â€”
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
