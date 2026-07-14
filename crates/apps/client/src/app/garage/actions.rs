//! The garage orbit/inspection camera and the `ClientApp` glue that turns cursor clicks into
//! selection, fitting edits, Battle, or camera drag. Kept apart from the state core in
//! [`super`] for reviewability; both operate on the same private [`GarageState`] fields.

#[cfg(test)]
use game_core::VehicleKind;
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
        // Mirrors the `GarageHit::Vehicle` click in `garage_primary_press`.
        self.queue_audio(audio::AudioEvent::UiClick { accent: false });
    }

    /// Route a left-button press in the garage to selection, fitting, Battle, or orbiting.
    /// Shift held while clicking a module slot cycles that slot backward.
    pub(in crate::app) fn garage_primary_press(&mut self) {
        let shift = self.input.shift;
        let view = self.garage.view();
        let hit = self.garage.hit_test(shift);

        // An open option list is modal: a click either picks a row (installing it) or dismisses the
        // list — nothing behind it acts on the same press.
        if self.garage.option_list().is_some() {
            if let GarageHit::OptionRow(slot, index) = hit {
                self.queue_audio(audio::AudioEvent::UiClick { accent: false });
                self.garage.select_option(slot, index);
                self.garage_reject_feedback();
            } else {
                self.garage.close_option_list();
            }
            return;
        }

        // Every acted-on control answers with the switch click; orbiting the camera is not a
        // control, and Battle lands its own accented click in `confirm_garage_selection`.
        if !matches!(hit, GarageHit::Scene | GarageHit::Battle) {
            self.queue_audio(audio::AudioEvent::UiClick { accent: false });
        }
        match hit {
            GarageHit::Vehicle(index) => {
                self.garage.select_index(index);
                // Selecting a vehicle from the tech tree returns to the hangar view.
                if view == super::GarageView::TechTree {
                    self.garage.close_tech_tree();
                }
            }
            GarageHit::CarouselScroll(dir) => self.garage.scroll_carousel(dir),
            // Plain click opens the informed option list for the slot; Shift+click keeps the express
            // backward cycle (no list). Both fly the camera to frame the module.
            GarageHit::ModuleCycle(slot, dir) => {
                if dir < 0 {
                    self.garage.cycle_module(slot, dir);
                    self.garage_reject_feedback();
                } else {
                    self.garage.open_option_list(slot);
                }
                self.garage.focus_module(slot);
            }
            // Unreachable while no list is open (rows only hit-test when one is), but kept exhaustive.
            GarageHit::OptionRow(slot, index) => {
                self.garage.select_option(slot, index);
                self.garage_reject_feedback();
            }
            GarageHit::AmmoSelect(index) => self.garage.set_ammo(index),
            // The rack count editor: plain click moves one round, Shift moves five.
            GarageHit::AmmoAdjust(index, dir) => {
                let step = if shift { 5 } else { 1 };
                self.garage.adjust_ammo_count(index, dir as i32 * step);
            }
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
        // Right-click dismisses an open option list; otherwise it is the express backward cycle.
        if self.garage.option_list().is_some() {
            self.garage.close_option_list();
            return;
        }
        if let GarageHit::ModuleCycle(slot, _) = self.garage.hit_test(true) {
            self.garage.cycle_module(slot, -1);
            self.garage_reject_feedback();
        }
    }

    /// After a fitting edit, answer a compatibility rejection with the dull knock — the red
    /// flash's audible half (`rejected_slot` used to light silently). Every edit path resets
    /// `rejected_slot` before acting, so `Some` here means THIS action was refused.
    fn garage_reject_feedback(&mut self) {
        if self.garage.rejected_slot().is_some() {
            self.queue_audio(audio::AudioEvent::UiReject);
        }
    }

    /// Garage keyboard bindings: selection, loadout editing, crew, tech tree, Battle. Takes the
    /// `PhysicalKey` alone (the only field the garage reads) so the routing is unit-testable — a
    /// winit `KeyEvent` cannot be constructed outside winit. Always returns `true` while the garage
    /// is open (its only caller), swallowing unbound keys so none leak to driving.
    pub(in crate::app) fn garage_keyboard(&mut self, key: PhysicalKey) -> bool {
        match key {
            // Arrow keys cycle the roster. The old 1-5 vehicle digits are retired: with a scroll
            // window, a window-relative digit selects a different tank than the label implies.
            PhysicalKey::Code(KeyCode::ArrowLeft) => self.garage.cycle(-1),
            PhysicalKey::Code(KeyCode::ArrowRight) => self.garage.cycle(1),
            PhysicalKey::Code(KeyCode::Enter) => self.confirm_garage_selection(),
            // Escape peels back one layer at a time: first an open option list, then a module-focus
            // framing (return to hero), then — camera already at rest — closes the garage.
            PhysicalKey::Code(KeyCode::Escape) => {
                if self.garage.option_list().is_some() {
                    self.garage.close_option_list();
                } else if self.garage.is_camera_off_hero() {
                    self.garage.return_to_hero_view();
                } else {
                    self.garage.close_if_started();
                }
            }
            // Keyboard loadout editing: focus + cycle + ammo + crew.
            PhysicalKey::Code(KeyCode::BracketLeft) => self.garage.focus_adjacent(-1),
            PhysicalKey::Code(KeyCode::BracketRight) => self.garage.focus_adjacent(1),
            PhysicalKey::Code(KeyCode::KeyQ) => {
                self.garage.cycle_focused(-1);
                self.garage_reject_feedback();
            }
            PhysicalKey::Code(KeyCode::KeyE) => {
                self.garage.cycle_focused(1);
                self.garage_reject_feedback();
            }
            PhysicalKey::Code(KeyCode::KeyZ) => self.garage.set_ammo(0),
            PhysicalKey::Code(KeyCode::KeyX) => self.garage.set_ammo(1),
            PhysicalKey::Code(KeyCode::KeyC) => self.garage.set_ammo(2),
            PhysicalKey::Code(KeyCode::Minus) => self.garage.adjust_proficiency(-1),
            PhysicalKey::Code(KeyCode::Equal) => self.garage.adjust_proficiency(1),
            PhysicalKey::Code(KeyCode::KeyT) => match self.garage.view() {
                super::GarageView::Hangar => self.garage.open_tech_tree(),
                super::GarageView::TechTree => self.garage.close_tech_tree(),
            },
            // The open garage owns the keyboard: swallow every key it does not itself bind, so a
            // keystroke never leaks through to drive the tank or switch ammo in the battle running
            // underneath (mid-battle the sim keeps ticking behind the overlay; before, an unbound
            // key like W or Space fell through to `on_driving_keyboard` once `has_started`).
            // `on_keyboard` only routes here while the garage is open, so swallowing all is correct.
            _ => return true,
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
        // The commit deserves a heavier hand on the switch than browsing.
        self.queue_audio(audio::AudioEvent::UiClick { accent: true });
        let spec = self.garage.confirm();
        let display_name = spec.name.clone();
        // Committing from the garage in a random battle ABANDONS it and deploys into a fresh one
        // (new seed, full roster, full clock). Replacing the player's tank inside the running
        // battle was a free heal: G mid-fight, confirm, and the hull came back factory-new while
        // everyone else stayed shot up. It also closes the loop after VICTORY/DEFEAT/DRAW — the
        // garage's Battle button IS the next battle.
        if self.session.battle_mode() == server::BattleMode::Random7v7 {
            self.session = crate::app::session::BattleSessionKind::Local(Box::new(
                server::LocalAuthoritativeServer::new_random_7v7(
                    server::ServerTickConfig::default(),
                    server::RandomBattleConfig::runtime_from_env(spec.kind),
                ),
            ));
            self.client_tick = 0;
            self.damage_log = crate::hud::damage_log::DamageLog::default();
            self.incoming_hits = crate::hud::hit_direction::IncomingHitFeed::default();
            self.hit_indicator = crate::hit_indicator::HitIndicator::default();
            self.fx = crate::fx::FxSystem::default();
            self.tank_scars.clear();
            self.terrain_scars = crate::fx::TerrainScars::default();
            self.engine_smoke_accum_s.clear();
        }
        let snapshot = self.session.change_player_vehicle_with_spec_for_player(spec.clone());
        self.player_tank = self.session.player_tank();
        self.predictor.reset_to_spec(&spec);
        self.render_state = crate::InterpolatedBattleState::default();
        self.input.fire_pending = false;
        self.input.clear_mouse_look();
        self.battle_outcome = None;
        self.kill_confirm_age_s = None;
        self.reload_ready_age_s = None;
        self.prev_reload_remaining_s = 0.0;
        self.accept_and_sync(snapshot);
        // F6: the fresh battle's roster bakes now, behind the garage curtain — not when the
        // first enemy crests a ridge.
        self.preload_battle_vehicle_assets();
        self.set_cursor_captured(true);
        if let Some(window) = &self.window {
            window.set_title(&format!("{} - {display_name}", crate::ui_strings::WINDOW_TITLE));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::FitSlot;
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
    fn a_rejected_fit_knocks_back_audibly_and_an_accepted_one_does_not() {
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::T54_1951);

        // An accepted express cycle answers with the plain click only.
        app.pending_audio.clear();
        app.garage.set_cursor(module_slot_center(1)); // Gun slot
        app.garage_secondary_press();
        assert!(
            !app.pending_audio.contains(&audio::AudioEvent::UiReject),
            "an accepted swap must not knock"
        );

        // Force an incompatible fit (turret caliber limit under the gun) via the keyboard cycle:
        // the red flash now has its audible half.
        app.garage.force_turret_caliber_limit_for_test(99.0);
        app.pending_audio.clear();
        app.garage_keyboard(PhysicalKey::Code(KeyCode::KeyE));
        assert!(
            app.pending_audio.contains(&audio::AudioEvent::UiReject),
            "a rejected fit must answer with the reject knock: {:?}",
            app.pending_audio
        );
    }

    #[test]
    fn clicking_the_ammo_zones_edits_the_rack_and_shift_steps_by_five() {
        use crate::app::garage::layout::ammo_adjust_centers;
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::T54_1951);
        let (minus, _plus) = ammo_adjust_centers(0);
        let before = app.garage.draft().ammo_counts()[0];

        app.garage.set_cursor(minus);
        app.garage_primary_press();
        assert_eq!(app.garage.draft().ammo_counts()[0], before - 1, "plain click moves one round");

        app.input.set_shift(true);
        app.garage_primary_press();
        assert_eq!(app.garage.draft().ammo_counts()[0], before - 6, "shift+click moves five");
        assert_eq!(
            app.garage.draft().ammo_index(),
            0,
            "editing the fill never switches the loaded round"
        );
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
    fn plain_click_on_a_swappable_slot_opens_its_option_list_without_changing_the_fit() {
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::T54_1951);
        app.garage.set_cursor(module_slot_center(1)); // Gun slot (a real choice on the T-54)
        let stock = app.garage.draft().gun_barrel_length();

        app.garage_primary_press();

        assert_eq!(
            app.garage.option_list(),
            Some(FitSlot::Gun),
            "plain click opens the option list"
        );
        assert_eq!(
            app.garage.draft().gun_barrel_length(),
            stock,
            "opening the list must not change the installed gun"
        );
    }

    #[test]
    fn shift_click_express_cycles_backward_without_opening_a_list() {
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::T54_1951);
        app.garage.set_cursor(module_slot_center(1)); // Gun slot
        let stock = app.garage.draft().gun_barrel_length();

        // Shift+click is the express path: it cycles backward (from stock, wraps to the alternate
        // gun) and never opens the list.
        app.input.set_shift(true);
        app.garage_primary_press();

        assert_eq!(app.garage.option_list(), None, "the express cycle opens no list");
        assert_ne!(
            app.garage.draft().gun_barrel_length(),
            stock,
            "shift+click moves off the stock gun (express backward cycle)"
        );
    }

    #[test]
    fn clicking_an_option_row_installs_it_and_closes_the_list() {
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::T54_1951);
        let stock = app.garage.draft().gun_barrel_length();

        // Open the gun list, then click the alternate option row (row 1).
        app.garage.set_cursor(module_slot_center(1));
        app.garage_primary_press();
        assert_eq!(app.garage.option_list(), Some(FitSlot::Gun));

        app.garage
            .set_cursor(crate::app::garage::layout::option_row_center(FitSlot::Gun.index(), 1));
        app.garage_primary_press();

        assert_eq!(app.garage.option_list(), None, "picking a row closes the list");
        assert_ne!(app.garage.draft().gun_barrel_length(), stock, "the picked gun is installed");
    }

    #[test]
    fn clicking_outside_an_open_list_dismisses_it_without_acting() {
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::T54_1951);
        app.garage.set_cursor(module_slot_center(1));
        app.garage_primary_press();
        assert_eq!(app.garage.option_list(), Some(FitSlot::Gun));

        // A click in empty scene space dismisses the list and must not start a camera drag.
        app.garage.set_cursor([0.0, 0.0]);
        app.garage_primary_press();
        assert_eq!(app.garage.option_list(), None, "clicking away closes the list");
        assert!(!app.garage.is_dragging(), "the dismiss click does not start an orbit drag");
    }

    #[test]
    fn selecting_vehicle_from_tech_tree_returns_to_hangar() {
        use super::super::GarageView;
        use crate::app::garage::layout::tree_node_center;
        use game_core::Era;

        let mut app = ClientApp::new();
        app.garage.open_tech_tree();
        assert_eq!(app.garage.view(), GarageView::TechTree);

        // Click the Tiger I node (first Era II node = PLAYABLE index 1).
        app.garage.set_cursor(tree_node_center(Era::LateWar, 0));
        app.garage_primary_press();

        assert_eq!(app.garage.view(), GarageView::Hangar, "returns to hangar");
        assert_eq!(app.garage.selected_vehicle(), VehicleKind::TigerI);
    }

    /// Confirming from the garage mid-battle used to REPLACE the player's tank inside the running
    /// battle: a factory-new hull (free heal) dropped into a half-shot-up roster, and there was
    /// no way to ever start a new battle. Locked here: the commit abandons the old battle and
    /// deploys into a fresh one — tick 0, full 14-tank roster, full battle clock, no outcome.
    #[test]
    fn confirming_mid_battle_deploys_into_a_fresh_battle_not_a_free_heal() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.run_fixed_ticks(30);
        assert!(app.session.authoritative_tick() >= 30, "the first battle is running");

        app.open_garage();
        app.confirm_garage_selection();

        assert_eq!(app.session.authoritative_tick(), 0, "a FRESH battle, not a respawn");
        assert_eq!(app.session.latest_snapshot().tanks.len(), 14, "full fresh roster");
        assert_eq!(app.session.battle_outcome(), None);
        assert_eq!(
            app.session.battle_time_remaining_s(),
            Some(server::RANDOM_BATTLE_TIME_LIMIT_S as f32),
            "the battle clock starts full again"
        );
    }

    #[test]
    fn confirming_garage_selection_keeps_random_7v7_roster() {
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::IS3);

        app.confirm_garage_selection();

        let full_snapshot = app.session.latest_snapshot();
        assert_eq!(full_snapshot.tanks.len(), 14);
        assert!(full_snapshot.tanks.iter().any(|tank| {
            tank.tank_id == app.player_tank
                && tank.team == game_core::TeamId(1)
                && tank.vehicle == VehicleKind::IS3
        }));
        assert!(app.render_state.latest_snapshot().is_some_and(|snapshot| {
            snapshot.tanks.iter().any(|tank| tank.tank_id == app.player_tank)
        }));
    }

    /// Regression: after the first battle, unbound keys (W, Space, ammo digits) leaked through the
    /// open garage into `on_driving_keyboard` and drove/fired the tank that keeps ticking behind the
    /// overlay. The open garage must swallow every key it does not itself bind.
    #[test]
    fn open_garage_swallows_unbound_driving_keys_so_they_never_reach_the_battle() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.run_fixed_ticks(5);
        app.open_garage();
        assert!(app.garage.is_open() && app.garage.has_started(), "garage open over a live battle");

        for key in [KeyCode::KeyW, KeyCode::KeyS, KeyCode::Space, KeyCode::Digit1] {
            assert!(
                app.garage_keyboard(PhysicalKey::Code(key)),
                "the open garage must swallow {key:?}, not leak it to driving/firing"
            );
        }
        // A key the garage DOES bind still reports handled (sanity that the swallow didn't mask real
        // bindings): Enter commits to battle.
        assert!(app.garage_keyboard(PhysicalKey::Code(KeyCode::BracketRight)));
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
