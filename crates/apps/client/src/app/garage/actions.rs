//! The garage orbit/inspection camera and the `ClientApp` glue that turns cursor clicks into
//! selection, fitting edits, Battle, or camera drag. Kept apart from the state core in
//! [`super`] for reviewability; both operate on the same private [`GarageState`] fields.

#[cfg(test)]
use game_core::VehicleKind;
use winit::event::KeyEvent;
use winit::keyboard::{KeyCode, PhysicalKey};

use super::{GarageHit, GarageState};
use crate::app::ClientApp;
impl GarageState {
    pub(super) fn begin_drag(&mut self) {
        self.dragging = true;
    }

    pub(super) fn end_drag(&mut self) {
        self.dragging = false;
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

    #[cfg(test)]
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
            GarageHit::CarouselScroll(dir) => self.garage.scroll_carousel(dir),
            GarageHit::ModuleCycle(slot, dir) => {
                self.garage.cycle_module(slot, dir);
                // Clicking a module both cycles it and flies the camera to frame it.
                self.garage.focus_module(slot);
            }
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

    /// Garage keyboard bindings: selection, loadout editing, crew, tech tree, Battle. Returns
    /// `false` for keys the garage does not own (they fall through to driving once started).
    pub(in crate::app) fn garage_keyboard(&mut self, event: &KeyEvent) -> bool {
        match event.physical_key {
            // Arrow keys cycle the roster. The old 1-5 vehicle digits are retired: with a scroll
            // window, a window-relative digit selects a different tank than the label implies.
            PhysicalKey::Code(KeyCode::ArrowLeft) => self.garage.cycle(-1),
            PhysicalKey::Code(KeyCode::ArrowRight) => self.garage.cycle(1),
            PhysicalKey::Code(KeyCode::Enter) => self.confirm_garage_selection(),
            // Escape first backs out of a module focus (return to the hero framing); a second
            // press, with the camera already at rest on the hero view, closes the garage.
            PhysicalKey::Code(KeyCode::Escape) => {
                if self.garage.is_camera_off_hero() {
                    self.garage.return_to_hero_view();
                } else {
                    self.garage.close_if_started();
                }
            }
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
                super::GarageView::Hangar => self.garage.open_tech_tree(),
                super::GarageView::TechTree => self.garage.close_tech_tree(),
            },
            // Before the first battle every other key is swallowed; afterwards it drives.
            _ => return !self.garage.has_started(),
        }
        true
    }

    /// Turn on garage disk persistence (selected vehicle + per-vehicle loadouts survive restarts).
    /// Called once from the real startup path; `ClientApp::new` stays pure so tests never touch
    /// the user's save file.
    pub(in crate::app) fn enable_garage_persistence(&mut self) {
        self.garage.enable_persistence(super::persistence::save_path());
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
        assert!(garage.orbit_pitch < 1.3, "pitch clamps short of vertical");
        garage.apply_zoom(1_000.0);
        assert!(garage.orbit_distance >= 4.0 - 1.0e-6, "distance clamps at the close boom");
        garage.apply_zoom(-1_000.0);
        assert!(garage.orbit_distance <= 20.0 + 1.0e-6, "distance clamps at the far boom");
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
