use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta};
use winit::keyboard::PhysicalKey;

use super::keybinds::{Action, Context};
use winit::window::CursorGrabMode;

use super::ClientApp;
use crate::{BattleCameraInput, BattleCameraMode};

const MOUSE_YAW_SENSITIVITY: f32 = 0.0035;
const MOUSE_PITCH_SENSITIVITY: f32 = 0.0030;

impl ClientApp {
    pub(super) fn on_keyboard(&mut self, event: &KeyEvent) {
        self.on_key(event.physical_key, event.state == ElementState::Pressed, event.repeat);
    }

    /// One key event below the winit boundary (a `KeyEvent` cannot be constructed outside winit,
    /// so the tests drive this). `repeat` is the OS auto-repeat: Windows re-sends a held key as
    /// a PRESS every ~33 ms after half a second. Every binding here is an EDGE — V toggles the
    /// scope, ESC raises or dismisses the modal, Space latches the trigger, 1/2/3 queue a switch,
    /// G opens the garage — so a repeat is not "still pressed", it is a second press the player
    /// never made: a held V flickered the view thirty times a second, a held Space knocked the
    /// refusal sound for the whole reload. Shift and Alt guarded themselves; the rest did not.
    /// Repeats are dropped here, once, for every key. Nothing needs them: a held drive key is
    /// already latched by its first press, and the garage roster is cycled per press.
    pub(in crate::app) fn on_key(&mut self, key: PhysicalKey, pressed: bool, repeat: bool) {
        if repeat {
            return;
        }
        // P8: a row listening for its key takes every press — the window's keys included, or
        // F11 could never be bound.
        if self.shell_listening().is_some() {
            if pressed {
                self.shell_take_key(key);
            }
            return;
        }
        // The window's keys (P7: `Context::Global`) work in the garage, in the battle and over
        // the ESC modal alike, and nothing underneath sees them.
        match self.keybinds.action(Context::Global, key) {
            Some(Action::ToggleFullscreen) if pressed => {
                self.toggle_fullscreen();
                return;
            }
            // H22: the palette rings until P6's settings screen lands; the choice persists.
            Some(Action::CyclePalette) if pressed => {
                self.cycle_palette();
                return;
            }
            Some(_) => return,
            None => {}
        }
        // P6/P8: a shell page — a menu, the settings, the keys — has the keyboard while it is
        // open, over the garage and the battle alike; nothing underneath sees a key.
        if self.shell_open() {
            if let Some(action) = self.keybinds.action(Context::Shell, key) {
                self.shell_action(action, pressed);
            }
            return;
        }
        if pressed && self.garage.is_open() && self.garage_keyboard(key) {
            return;
        }
        self.on_battle_keyboard(key, pressed);
    }

    /// Borderless fullscreen on the monitor the window is on, or back to the maximized window.
    /// P9: the setting is the truth — F11 writes it and the window follows
    /// (`apply_fullscreen_setting`); the pacer re-reads the display afterwards (the window may
    /// have landed on another monitor's refresh rate).
    pub(in crate::app) fn toggle_fullscreen(&mut self) {
        let borderless = !self.fullscreen;
        self.edit_settings(|settings| settings.borderless = borderless);
        self.sync_present_hz();
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
        // P6: a shell page has the keyboard while it is open; the battle sees nothing.
        if self.shell_open() {
            if let Some(action) = self.keybinds.action(Context::Shell, key) {
                self.shell_action(action, pressed);
            }
            return;
        }
        // H21: the HUD editor has the keyboard while it is open — every press and release.
        if self.hud_editor_open() {
            if let Some(action) = self.keybinds.action(Context::HudEditor, key) {
                self.hud_editor_action(action, pressed);
            }
            return;
        }
        let Some(action) = self.keybinds.action(Context::Battle, key) else { return };
        self.on_driving_action(action, pressed);
    }

    /// The battle's actions (P7: the table's word for the key). ESC in a live battle raises
    /// the leave-or-stay modal; the cursor is freed so the player can answer it, which also
    /// preserves what ESC always did here: give the mouse back.
    fn on_driving_action(&mut self, action: Action, pressed: bool) {
        match action {
            Action::Forward => self.input.forward = pressed,
            Action::Back => self.input.back = pressed,
            // H19: a dead crew rides its allies — the arrows step through the living ones.
            Action::Left if pressed && self.camera_controller.death_spectate() => {
                self.spectate_step(-1)
            }
            Action::Right if pressed && self.camera_controller.death_spectate() => {
                self.spectate_step(1)
            }
            // H20: Enter takes the banner's hand-off at once.
            Action::Continue if pressed && self.battle_outcome.is_some() => self.hand_off_outcome(),
            Action::Continue => {}
            Action::Left => self.input.left = pressed,
            Action::Right => self.input.right = pressed,
            Action::Brake => self.input.set_brake(pressed),
            // H5, World of Tanks' cruise control: R steps the latched throttle up, F down; a
            // key repeat must not climb the ladder on its own, so the edge alone counts.
            Action::CruiseUp if pressed => self.input.cruise_up(),
            Action::CruiseDown if pressed => self.input.cruise_down(),
            Action::CruiseUp | Action::CruiseDown => {}
            // H16: Z holds the command wheel open; its release says the chosen word.
            Action::CommandWheel => {
                if pressed {
                    self.open_command_wheel();
                } else {
                    self.release_command_wheel();
                }
            }
            // H15: M cycles the minimap through its three sizes.
            Action::MinimapSize if pressed => self.input.cycle_minimap(),
            // H8: N folds the hit log to its newest row and unfolds it again.
            Action::HitLogFold if pressed => self.input.toggle_hit_log(),
            // H11: T marks the hull under the reticle as THE target and tells the team. It
            // never lays the gun (the owner, 2026-09-02: no aim assist of any kind).
            Action::MarkTarget if pressed => self.mark_target(),
            Action::MinimapSize | Action::HitLogFold | Action::MarkTarget => {}
            // The sniper key toggles the scope (World of Tanks, the default) or holds it —
            // the player's setting (P6).
            Action::Sniper if self.settings.sniper_toggle => {
                if pressed && !self.garage.is_open() {
                    self.toggle_camera_mode();
                }
            }
            Action::Sniper => {
                if pressed {
                    self.begin_sniper_hold();
                } else {
                    self.end_sniper_hold();
                }
            }
            Action::FreeLook => {
                if pressed && !self.input.free_look {
                    self.begin_free_look();
                } else if !pressed && self.input.free_look {
                    self.end_free_look();
                }
            }
            Action::Fire if pressed => self.input.fire_pending = true,
            Action::ToGarage if pressed && self.garage.has_started() => self.open_garage(),
            Action::Fire | Action::ToGarage => {}
            // 1/2/3 select ammo (genre standard; the vision's ammo-rack slots). The camera
            // moved to V — the wheel scroll-through stays the primary camera path.
            Action::Ammo1 if pressed => self.request_ammo_slot(0),
            Action::Ammo2 if pressed => self.request_ammo_slot(1),
            Action::Ammo3 if pressed => self.request_ammo_slot(2),
            Action::CameraToggle if pressed => self.toggle_camera_mode(),
            Action::Ammo1 | Action::Ammo2 | Action::Ammo3 | Action::CameraToggle => {}
            // In a live battle ESC asks the question; before one exists (garage never left) it
            // keeps its plain meaning of handing the cursor back.
            Action::Escape if pressed => {
                if self.garage.has_started() && !self.garage.is_open() {
                    self.open_pause_menu();
                } else {
                    self.set_cursor_captured(false);
                }
            }
            Action::Escape => {}
            // Another context's word: not this router's.
            _ => {}
        }
    }

    /// ESC in a live battle: the menu (P8: a shell page). A wheel open under it would say a
    /// word on the next Z release: it closes unsaid. The driving keys are released rather than
    /// left latched: the battle does NOT pause, and a hull driving on by itself while its
    /// commander reads a menu is exactly the kind of hidden consequence this game refuses. It
    /// coasts to a stop, in the open, visibly.
    pub(in crate::app) fn open_pause_menu(&mut self) {
        self.open_menu(crate::hud::shell::MenuKind::Battle);
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

    /// Alt-tab and friends. Whether an unfocused window ever sees the releases of the keys held
    /// at the switch is the platform's business, not ours: winit on Windows synthesizes them on
    /// `WM_KILLFOCUS`, other backends may not, and a future winit may change either. This app
    /// does not depend on it. On either focus edge EVERY latch the keyboard holds is dropped —
    /// the drive keys and the trigger, and the two modal holds that used to be forgotten here:
    /// a free look left latched keeps the mouse on the camera and off the gun for the rest of
    /// the match ("my turret stopped following the mouse" after an Alt+Tab, Alt being the
    /// free-look key), and a Shift hold left open swallows the next Shift press whole. Both are
    /// ended through their own release paths so the camera returns to the aim and the scope to
    /// its prior mode, exactly as a real key-up would have done. The cursor is recaptured only
    /// for the live battle view (the garage menu and the ESC modal keep it free).
    pub(in crate::app) fn on_focus_change(&mut self, focused: bool) {
        if self.input.free_look {
            self.end_free_look();
        }
        self.end_sniper_hold();
        self.command_wheel = None;
        self.input.release_all_latches();
        self.garage.end_drag();
        self.set_cursor_captured(focused && !self.garage.is_open() && !self.shell_open());
    }

    pub(super) fn on_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        let lines = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(position) => position.y as f32 / 60.0,
        };
        if self.shell_open() {
            // No camera dolly behind an open page — the view stays where the player left it.
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

    /// T (H11): the hull under the reticle becomes THE target — the full marker's owner — and
    /// the team hears „attack this" through the server's relay (W-5). With nothing under the
    /// reticle the mark stays as it was. The gun is not touched: a mark is a word, not a lay.
    pub(super) fn mark_target(&mut self) {
        if let Some(hull) = self.hull_under_reticle {
            self.target_mark = Some(hull);
            self.say(net::TeamCommand::Attack, Some(hull), None);
        }
    }

    /// The other team in a two-team battle: whose spotting bit is the sixth sense's.
    pub(super) fn enemy_team(&self) -> game_core::TeamId {
        if self.player_team() == game_core::TeamId(1) {
            game_core::TeamId(2)
        } else {
            game_core::TeamId(1)
        }
    }

    /// The sixth sense's edge (H13): the chime plays when the own mask goes from clear to set,
    /// once per spotted span — never while it stays set, never on a memory of it.
    pub(super) fn sixth_sense_edge(&mut self, lit_now: bool) {
        if lit_now && !self.spotted_before {
            self.queue_audio(audio::AudioEvent::SixthSense);
        }
        self.spotted_before = lit_now;
    }

    /// The mark dies with the hull's visibility: gone from the snapshot — unspotted, or dead —
    /// gone from the HUD, so the marker can never point at a memory.
    pub(super) fn refresh_target_mark(&mut self, visible: impl Iterator<Item = game_core::TankId>) {
        if let Some(target) = self.target_mark
            && !visible.into_iter().any(|id| id == target)
        {
            self.target_mark = None;
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
        if self.shell_open() || self.hud_editor_open() {
            // The cursor is answering a page, not aiming the gun.
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
        // H16: while the wheel is open the mouse picks a sector; the gun holds its lay.
        if let Some(travel) = self.command_wheel.as_mut() {
            travel[0] += dx;
            travel[1] += dy;
            return;
        }
        // Mouse-right (dx > 0) must look right; +orbit_yaw points toward world +X = screen
        // left, so negate it. The FOV ratio slows the look exactly as much as zoom magnifies it.
        // The player's own sensitivity for this zoom step (P6) rides on the sight's scale.
        let scale = self.camera_controller.look_sensitivity_scale()
            * self.settings.sensitivity_for(self.zoom_step());
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
        // Tracked in every mode (interface program F5/F7): the HUD editor and the command
        // wheel read the cursor in battle; capture decides visibility, not tracking.
        self.cursor_px = [x, y];
        if self.shell_open() {
            self.shell_cursor([x, y]);
            return;
        }
        if self.hud_editor_open() {
            self.hud_editor_cursor([x, y]);
            return;
        }
        if !self.garage.is_open() {
            return;
        }
        let (w, h) = self.viewport;
        let clip_x = (x / w as f32) * 2.0 - 1.0;
        let clip_y = 1.0 - (y / h as f32) * 2.0;
        self.garage.set_cursor([clip_x, clip_y]);
    }

    /// The cursor's last position in physical pixels, in every mode. Read by the HUD editor
    /// and the command wheel when they land (H21, H16); the lock reads it today.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn cursor_px(&self) -> [f32; 2] {
        self.cursor_px
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
