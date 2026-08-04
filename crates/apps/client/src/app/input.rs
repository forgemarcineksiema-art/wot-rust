use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::CursorGrabMode;

use super::ClientApp;
use crate::{BattleCameraInput, BattleCameraMode};

const MOUSE_YAW_SENSITIVITY: f32 = 0.0035;
const MOUSE_PITCH_SENSITIVITY: f32 = 0.0030;

impl ClientApp {
    pub(super) fn on_keyboard(&mut self, event: &KeyEvent) {
        let pressed = event.state == ElementState::Pressed;
        if pressed && self.garage.is_open() && self.garage_keyboard(event.physical_key) {
            return;
        }
        self.on_battle_keyboard(event.physical_key, pressed);
    }

    /// Battle-side key dispatch, taken below the winit boundary so tests can drive it — a winit
    /// `KeyEvent` cannot be constructed outside winit, the same reason `garage_keyboard` takes a
    /// bare `PhysicalKey`.
    ///
    /// The ESC modal is modal: while it is up the only PRESS it answers is ESC, which dismisses
    /// it. Every other press is swallowed, so a player reading the menu cannot drive or fire by
    /// leaning on the keyboard. Releases still fall through — swallowing those would strand a key
    /// that was already held when the menu opened.
    pub(in crate::app) fn on_battle_keyboard(&mut self, key: PhysicalKey, pressed: bool) {
        if pressed && self.pause_menu.is_some() {
            if matches!(key, PhysicalKey::Code(KeyCode::Escape)) {
                self.close_pause_menu();
            }
            return;
        }
        self.on_driving_keyboard(key, pressed);
    }

    /// ESC in a live battle raises the leave-or-stay modal. The cursor is freed so the player can
    /// answer it, which also preserves what ESC always did here: give the mouse back.
    pub(in crate::app) fn open_pause_menu(&mut self) {
        self.pause_menu = Some(super::PauseMenuState::opened());
        // Release the driving keys rather than leaving them latched: the battle does NOT pause,
        // and a hull driving on by itself while its commander reads a menu is exactly the kind of
        // hidden consequence this game refuses. It coasts to a stop, in the open, visibly.
        self.input.release_driving();
        self.set_cursor_captured(false);
    }

    pub(in crate::app) fn close_pause_menu(&mut self) {
        self.pause_menu = None;
        // Mouse motion accumulated while the menu was up must not be spent as a look delta the
        // moment it closes, or the turret jumps to wherever the player was pointing at a button.
        self.input.clear_mouse_look();
        self.set_cursor_captured(true);
    }

    fn on_driving_keyboard(&mut self, key: PhysicalKey, pressed: bool) {
        match key {
            PhysicalKey::Code(KeyCode::KeyW | KeyCode::ArrowUp) => self.input.forward = pressed,
            PhysicalKey::Code(KeyCode::KeyS | KeyCode::ArrowDown) => self.input.back = pressed,
            PhysicalKey::Code(KeyCode::KeyA | KeyCode::ArrowLeft) => self.input.left = pressed,
            PhysicalKey::Code(KeyCode::KeyD | KeyCode::ArrowRight) => self.input.right = pressed,
            PhysicalKey::Code(KeyCode::ControlLeft | KeyCode::ControlRight) => {
                self.input.brake = pressed
            }
            PhysicalKey::Code(KeyCode::ShiftLeft | KeyCode::ShiftRight) => {
                if pressed {
                    self.begin_sniper_hold();
                } else {
                    self.end_sniper_hold();
                }
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
            // moved to V — the wheel scroll-through stays the primary camera path.
            PhysicalKey::Code(KeyCode::Digit1) if pressed => self.request_ammo_slot(0),
            PhysicalKey::Code(KeyCode::Digit2) if pressed => self.request_ammo_slot(1),
            PhysicalKey::Code(KeyCode::Digit3) if pressed => self.request_ammo_slot(2),
            PhysicalKey::Code(KeyCode::KeyV) if pressed => self.toggle_camera_mode(),
            // In a live battle ESC asks the question; before one exists (garage never left) it
            // keeps its plain meaning of handing the cursor back.
            PhysicalKey::Code(KeyCode::Escape) if pressed => {
                if self.garage.has_started() && !self.garage.is_open() {
                    self.open_pause_menu();
                } else {
                    self.set_cursor_captured(false);
                }
            }
            _ => {}
        }
    }

    /// A mouse press in the live battle view (no garage, no modal): it (re)captures the cursor,
    /// and the left button latches the trigger for the next fixed-tick batch. A left press
    /// inside the post-deploy window is the second half of a double-click on BATTLE — UI
    /// residue, not a fire order — so it captures without latching.
    pub(in crate::app) fn on_battle_mouse_press(&mut self, button: MouseButton) {
        self.set_cursor_captured(true);
        if button == MouseButton::Left && self.input.deploy_fire_shield_ticks == 0 {
            self.input.fire_pending = true;
        }
    }

    /// Alt-tab and friends. An unfocused window receives no key or button releases, so anything
    /// latched at the moment of the switch would stay latched until re-pressed — the hull driving
    /// itself, a queued shot going off on return, the garage orbit glued to a button nobody
    /// holds. Drop all of it on either edge; the cursor is recaptured only for the live battle
    /// view (the garage menu and the ESC modal keep it free).
    pub(in crate::app) fn on_focus_change(&mut self, focused: bool) {
        self.input.release_driving();
        self.input.clear_mouse_look();
        self.garage.end_drag();
        self.set_cursor_captured(focused && !self.garage.is_open() && self.pause_menu.is_none());
    }

    pub(super) fn on_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        let lines = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(position) => position.y as f32 / 60.0,
        };
        if self.pause_menu.is_some() {
            // No camera dolly behind an open modal — the view stays where the player left it.
            return;
        }
        if self.garage.is_open() {
            // Over the carousel the wheel scrolls the roster; anywhere else it zooms the camera.
            if self.garage.cursor_over_carousel() {
                self.garage.scroll_carousel(-lines.signum() as i8);
            } else {
                self.garage.apply_zoom(lines);
            }
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
            // Capture the crosshair's world sight ray before the zoom step may hand over to sniper.
            let seed = (mode_before == BattleCameraMode::ThirdPerson)
                .then(|| self.world_sight_seed())
                .flatten();
            self.camera_controller.apply_input(BattleCameraInput {
                orbit_yaw_delta_rad: 0.0,
                pitch_delta_rad: 0.0,
                zoom_delta_m: -notch * 0.8,
            });
            // Scrolling through the shortest boom hands over to sniper; open on the same point.
            if mode_before == BattleCameraMode::ThirdPerson
                && self.camera_controller.mode() == BattleCameraMode::Sniper
            {
                self.apply_sniper_seed(seed);
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
        // Capture where the crosshair rests NOW (still third person), so the sniper view opens on
        // the same world point instead of jumping to the barrel line or the sky.
        let seed = self.world_sight_seed();
        self.camera_controller.set_mode(BattleCameraMode::Sniper);
        // Key entry always opens at the default magnification, never the last wheel step: an
        // absent-minded Shift peek must never snap open at 20x. The wheel dials deeper from here.
        self.camera_controller.reset_sniper_zoom();
        self.apply_sniper_seed(seed);
    }

    /// Open the sniper sight on `seed` (the world sight ray under the outgoing crosshair), clamped
    /// to what the gun can reach on the current hull. Yaw stays if no seed is available.
    fn apply_sniper_seed(&mut self, seed: Option<(f32, f32)>) {
        if let Some((yaw_rad, pitch_rad)) = seed {
            self.desired_aim = crate::aim::DesiredAim::new(yaw_rad, pitch_rad);
        }
        self.clamp_desired_aim_to_gun_reach();
        self.camera_controller.set_orbit_yaw(self.desired_aim.yaw_rad());
    }

    /// Holding Shift opens the scope; releasing returns to the mode from before the hold. This is
    /// the "aim-down-sights" path that complements the `V` toggle and the wheel handover.
    pub(super) fn begin_sniper_hold(&mut self) {
        // Swallow winit key-repeat, and never open the scope from the garage (there Shift+click
        // cycles a module backward — the sniper must not open behind the garage overlay).
        if self.input.sniper_hold_return.is_some() || self.garage.is_open() {
            return;
        }
        self.input.sniper_hold_return = Some(self.camera_controller.mode());
        // `enter_sniper_mode` seeds the crosshair sight ray (no-op if already in sniper), so the
        // view opens on the current aim point instead of jumping to the barrel line or the sky.
        self.enter_sniper_mode();
    }

    /// Releasing Shift restores the pre-hold mode: from third person it returns to third person,
    /// and if the player was already in sniper (via `V`), it stays in sniper.
    pub(super) fn end_sniper_hold(&mut self) {
        if let Some(prior) = self.input.sniper_hold_return.take() {
            self.camera_controller.set_mode(prior);
        }
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
        if self.pause_menu.is_some() {
            // The cursor is answering the modal, not aiming the gun.
            self.input.clear_mouse_look();
            return;
        }
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
            // The sniper view *is* the world sight ray. Mouse forward looks up, mouse back looks
            // down. Pitch is a world elevation now, clamped after the delta to what the gun can
            // reach on the current hull, so the crosshair never points where the gun cannot.
            self.desired_aim.set_yaw(self.desired_aim.yaw_rad() + yaw_delta);
            self.desired_aim.apply_pitch_delta(-pitch_delta);
            self.clamp_desired_aim_to_gun_reach();
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

    /// Map a window-pixel cursor position into clip space for the garage UI and the ESC modal.
    pub(super) fn on_cursor_moved(&mut self, x: f32, y: f32) {
        if !self.garage.is_open() && self.pause_menu.is_none() {
            return;
        }
        let (w, h) = self.viewport;
        let clip_x = (x / w as f32) * 2.0 - 1.0;
        let clip_y = 1.0 - (y / h as f32) * 2.0;
        if let Some(menu) = &mut self.pause_menu {
            menu.cursor_clip = [clip_x, clip_y];
            return;
        }
        self.garage.set_cursor([clip_x, clip_y]);
    }

    /// A left click while the ESC modal is up. Off both buttons it does nothing: a modal that
    /// closed on a stray click would drop the player back into the battle without an answer.
    pub(in crate::app) fn pause_menu_primary_press(&mut self) {
        let Some(menu) = &self.pause_menu else {
            return;
        };
        match menu.hovered() {
            Some(crate::hud::pause_menu::PauseMenuButton::ExitToGarage) => {
                self.queue_audio(audio::AudioEvent::UiClick { accent: true });
                self.pause_menu = None;
                self.open_garage();
            }
            Some(crate::hud::pause_menu::PauseMenuButton::Stay) => {
                self.queue_audio(audio::AudioEvent::UiClick { accent: false });
                self.close_pause_menu();
            }
            None => {}
        }
    }

    pub(super) fn set_cursor_captured(&mut self, captured: bool) {
        self.cursor_captured = captured;
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
