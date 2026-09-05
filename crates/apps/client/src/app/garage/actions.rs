//! The garage orbit/inspection camera and the `ClientApp` glue that turns cursor clicks into
//! selection, fitting edits, Battle, or camera drag. Kept apart from the state core in
//! [`super`] for reviewability; both operate on the same private [`GarageState`] fields.

#[cfg(test)]
use game_core::VehicleKind;
use winit::keyboard::{KeyCode, PhysicalKey};

use super::{GarageHit, GarageState};
use crate::app::ClientApp;

/// The battle every test deploys into. One pinned seed for the whole suite, so a client test
/// measures the change under test instead of which roster the clock happened to deal it.
#[cfg(test)]
const TEST_BATTLE_SEED: u64 = 0x5748_4154_5F41_494D;

impl GarageState {
    pub(super) fn begin_drag(&mut self) {
        self.dragging = true;
    }

    /// `crate::app`-visible: focus loss must also drop the drag — an unfocused window never
    /// delivers the button release that would have ended it.
    pub(in crate::app) fn end_drag(&mut self) {
        self.dragging = false;
    }

    pub(in crate::app) fn set_cursor(&mut self, clip: [f32; 2]) {
        self.cursor_clip = clip;
    }
}

impl ClientApp {
    pub(in crate::app) fn open_garage(&mut self) {
        // Every open with a battle running behind it is a RETURN from the field (the G key
        // and the pause menu both gate on `has_started`): the hero comes back dusty (J2)
        // and WEARING the fight (L1) — damage masks, thrown belt and hit decals, read from
        // the RAW latest snapshot (the interpolated one clobbers track state) plus the
        // client-side scar history. A clean machine captures clean and renders clean.
        if self.garage.has_started() {
            self.garage.dust_from_the_field();
            if let Some(tank) = self
                .render_state
                .latest_snapshot()
                .and_then(|s| s.tanks.iter().find(|t| t.tank_id == self.player_tank))
            {
                let scars = self.tank_scars.get(&self.player_tank);
                self.garage.wear_from_the_field(super::wear::FieldWear::from_battle(tank, scars));
            }
        }
        self.garage.open();
        // H1: the hall's daylight follows the PLAYER'S OWN CLOCK (standing user decision) —
        // refreshed on each open, so an evening session gets the evening hall. The state
        // itself never reads the wall clock; this is the one seam where the real world
        // enters, and the override (`L`) still wins over it.
        self.garage.set_auto_daylight(super::daylight_for_local_clock());
        self.input.clear_mouse_look();
        // Same rule as `open_pause_menu`: the battle does NOT pause behind the garage, and the
        // held keys will never deliver their release to it — without this, G with W held kept
        // the hull driving itself while its commander shopped for modules.
        self.input.release_driving();
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
            GarageHit::Battle => self.confirm_garage_selection(),
            GarageHit::MapCycle(dir) => self.cycle_battle_map(dir),
            GarageHit::OpenTechTree => self.garage.open_tech_tree(),
            GarageHit::CloseTechTree => self.garage.close_tech_tree(),
            GarageHit::Scene => self.garage.begin_drag(),
        }
    }

    pub(in crate::app) fn garage_primary_release(&mut self) {
        self.garage.end_drag();
    }

    /// Move the pre-battle map choice. The world it names starts baking on the next garage
    /// frame (see `poll_map_prebake`), so the player's travel to the Battle button is the
    /// budget the bake spends and the press itself costs a GPU upload.
    pub(in crate::app) fn cycle_battle_map(&mut self, dir: i8) {
        self.garage.cycle_map(dir);
        self.poll_map_prebake();
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
                    if !self.garage.is_open() {
                        // Mirrors `close_pause_menu`: back in the live battle the mouse is the
                        // gun again — recapture it, and drop the motion accumulated while it
                        // was a pointer so the turret does not jump on the first frame.
                        self.input.clear_mouse_look();
                        self.set_cursor_captured(true);
                    }
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
            PhysicalKey::Code(KeyCode::KeyM) => self.garage.cycle_map(1),
            // H1: the hall's daylight — Auto (the player's clock) → Morning → Day → Evening.
            PhysicalKey::Code(KeyCode::KeyL) => {
                self.garage.cycle_daylight();
                self.queue_audio(audio::AudioEvent::UiClick { accent: false });
            }
            // I1: the armor inspector — the gameplay armor volumes over the parked hero.
            PhysicalKey::Code(KeyCode::KeyI) => {
                self.garage.toggle_inspector();
                self.queue_audio(audio::AudioEvent::UiClick { accent: false });
            }
            // L2: repair — only a marked hero has anything to fix; the beat opens with the
            // heavier hand on the switch and closes with the shop's finishing clunk (the
            // completion sound queues from `tick_repair` in the render loop).
            PhysicalKey::Code(KeyCode::KeyR) => {
                if self.garage.start_repair() {
                    self.queue_audio(audio::AudioEvent::UiClick { accent: true });
                    // ...and the shop actually WORKS for the beat (R2): the wrench spans the
                    // same seconds the lift and the nameplate do, one source for all three.
                    self.queue_audio(audio::AudioEvent::RepairWork {
                        seconds: super::wear::REPAIR_BEAT_S,
                    });
                }
            }
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
        if self.session.battle_mode() == battle_host::BattleMode::Random7v7 {
            // The garage map row overrides the env/default resolution; AUTO (`None`) keeps
            // `runtime_from_env` intact — including the editor's Ctrl+P `.map.ron` playtest path.
            let mut battle_config = battle_host::RandomBattleConfig::runtime_from_env(spec.kind);
            if let Some(map) = self.garage.selected_map() {
                battle_config.map = map;
            }
            // A played battle is seeded from the wall clock (`BattleSeed::runtime`), which is
            // right for playing and poison for a test: every `confirm_garage_selection()` in the
            // suite drew a DIFFERENT roster, spawn assignment and set of bot routes. Tests that
            // then measured anything downstream of where the tanks stand — the sight point, the
            // aim bloom it commands — passed or failed by coin flip, and were read as
            // "load-sensitive" because a busy machine is where coins get flipped often enough to
            // notice (register F8, G12). Under test the battle is pinned.
            #[cfg(test)]
            {
                battle_config.seed = battle_host::BattleSeed::fixed(TEST_BATTLE_SEED);
            }
            let previous_map = self.session.map_id();
            self.session = crate::app::session::BattleSessionKind::Local(Box::new(
                battle_host::LocalAuthoritativeServer::new_random_7v7(
                    battle_host::ServerTickConfig::default(),
                    battle_config,
                ),
            ));
            // A different map means a different WORLD, and the client owns its copy of it: the
            // battlefield, the cover, the minimap, the camera leash and the baked scene all have
            // to move with the session or the eye and the predictor stay on the old map.
            if self.session.map_id() != previous_map {
                self.adopt_session_map();
            }
            self.weather_timeline = scene_build::weather_timeline::WeatherTimeline::new(
                self.session.map_id(),
                self.session.weather(),
            );
            self.weather_frame = self.weather_timeline.sample(0.0);
            self.client_tick = 0;
            self.damage_log = crate::hud::damage_log::DamageLog::default();
            self.incoming_hits = crate::hud::hit_direction::IncomingHitFeed::default();
            self.hit_indicator = crate::hit_indicator::HitIndicator::default();
            self.fx = crate::fx::FxSystem::default();
            self.tank_scars.clear();
            self.terrain_scars = crate::fx::TerrainScars::default();
            self.engine_smoke_accum_s.clear();
            // The per-tank presentation carry-overs die with the battle they belong to. Their
            // own upkeep drops entries whose tank left the snapshot — but a fresh roster REUSES
            // tank ids, so last battle's blown-off turret kept flying over the healthy tank that
            // inherited its id, wrecks kept their dented hulls, and shed track bands stayed
            // lying on a field they were never thrown onto.
            self.turret_popoffs.clear();
            self.wreck_hull_meshes.clear();
            self.track_ribbons.clear();
            self.wreck_age_s.clear();
            self.motion_fx.clear();
            self.cracked_shells.clear();
            self.track_marks = crate::fx::TrackMarks::default();
        }
        let snapshot = self.session.change_player_vehicle_with_spec_for_player(spec.clone());
        self.player_tank = self.session.player_tank();
        self.predictor.reset_to_spec(&spec);
        self.render_state = crate::InterpolatedBattleState::default();
        self.input.fire_pending = false;
        // The second press of a double-click on BATTLE arrives after the garage has closed and
        // would latch the trigger for the battle's first tick; shield it out (see the constant).
        self.input.deploy_fire_shield_ticks = crate::app::DEPLOY_FIRE_SHIELD_TICKS;
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
        let before = app.garage.draft().gun_name();
        app.garage_secondary_press();
        let after = app.garage.draft().gun_name();
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
        let stock = app.garage.draft().gun_name();

        app.garage_primary_press();

        assert_eq!(
            app.garage.option_list(),
            Some(FitSlot::Gun),
            "plain click opens the option list"
        );
        assert_eq!(
            app.garage.draft().gun_name(),
            stock,
            "opening the list must not change the installed gun"
        );
    }

    #[test]
    fn shift_click_express_cycles_backward_without_opening_a_list() {
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::T54_1951);
        app.garage.set_cursor(module_slot_center(1)); // Gun slot
        let stock = app.garage.draft().gun_name();

        // Shift+click is the express path: it cycles backward (from stock, wraps to the alternate
        // gun) and never opens the list.
        app.input.set_shift(true);
        app.garage_primary_press();

        assert_eq!(app.garage.option_list(), None, "the express cycle opens no list");
        assert_ne!(
            app.garage.draft().gun_name(),
            stock,
            "shift+click moves off the stock gun (express backward cycle)"
        );
    }

    #[test]
    fn clicking_an_option_row_installs_it_and_closes_the_list() {
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::T54_1951);
        let stock = app.garage.draft().gun_name();

        // Open the gun list, then click the alternate option row (row 1).
        app.garage.set_cursor(module_slot_center(1));
        app.garage_primary_press();
        assert_eq!(app.garage.option_list(), Some(FitSlot::Gun));

        app.garage
            .set_cursor(crate::app::garage::layout::option_row_center(FitSlot::Gun.index(), 1));
        app.garage_primary_press();

        assert_eq!(app.garage.option_list(), None, "picking a row closes the list");
        assert_ne!(app.garage.draft().gun_name(), stock, "the picked gun is installed");
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

        let mut app = ClientApp::new();
        app.garage.open_tech_tree();
        assert_eq!(app.garage.view(), GarageView::TechTree);

        app.garage.set_cursor(tree_node_center(VehicleKind::TigerI));
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
            Some(battle_host::RANDOM_BATTLE_TIME_LIMIT_S as f32),
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

    /// The map row is the pre-battle map choice: AUTO keeps whatever
    /// `RandomBattleConfig::runtime_from_env` resolves (the editor's Ctrl+P playtest path
    /// included), an explicit pick deploys the fresh battle on that map.
    #[test]
    fn the_garage_map_row_picks_the_battle_map_and_auto_keeps_the_env_resolution() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        let auto =
            battle_host::RandomBattleConfig::runtime_from_env(app.garage.selected_vehicle()).map;
        assert_eq!(app.session.map_id(), auto, "AUTO keeps the env/default resolution");

        // Click the map row until Ostrogorsk is set, then commit: the fresh battle runs there.
        app.open_garage();
        app.garage.set_cursor(crate::app::garage::layout::MAP_PICK_CENTER);
        while app.garage.selected_map() != Some(terrain::MapId::Ostrogorsk) {
            app.garage_primary_press();
        }
        app.confirm_garage_selection();
        assert_eq!(app.session.map_id(), terrain::MapId::Ostrogorsk);

        // Cycling back to AUTO restores the original resolution for the next battle.
        while app.garage.selected_map().is_some() {
            app.garage.cycle_map(1);
        }
        app.confirm_garage_selection();
        assert_eq!(app.session.map_id(), auto);
    }

    /// A map pick must move the WHOLE client world, not just the session's id. The battlefield
    /// the eye draws, the cover a shell stops on, and the heightmap the local predictor stands
    /// on all have to BE the map the server is simulating. When they lagged behind, three
    /// symptoms shipped together: every map looked identical (stale scene bake), remote tanks
    /// floated at server heights over the previous map's ground, and the player's own hull
    /// fought every correction because prediction and authority disagreed about the terrain.
    #[test]
    fn the_garage_map_pick_rebuilds_the_client_world_the_server_simulates() {
        let target = terrain::MapId::Ostrogorsk;
        let mut app = ClientApp::new();
        assert_ne!(app.session.map_id(), target, "the pick has to actually change the map");
        // The first battle's scene is baked and cached, exactly as a real deployment leaves it.
        app.ensure_battle_scene_meshes();

        app.open_garage();
        while app.garage.selected_map() != Some(target) {
            app.cycle_battle_map(1);
        }
        app.confirm_garage_selection();
        app.ensure_battle_scene_meshes();

        let expected = map_forge::battlefield(target);
        assert_eq!(app.session.map_id(), target);
        assert_eq!(
            map_forge::battlefield_hash(&app.battlefield),
            map_forge::battlefield_hash(&expected),
            "the client battlefield must be the map the server simulates"
        );
        assert_eq!(
            app.live_cover.blocking().len().min(1),
            expected.static_cover.len().min(1),
            "the blocking cover the predictor pushes against belongs to the new map"
        );
        assert_eq!(
            app.live_cover.phase_bytes().len(),
            expected.static_cover.len(),
            "a stale phase ledger makes every replicated cover state land on the wrong object"
        );
        let (ground_vertices, _) = crate::battlefield_ground_mesh(&expected);
        assert_eq!(
            app.battle_scene_meshes.as_ref().expect("ensured above").ground_vertices.len(),
            ground_vertices.len(),
            "the cached scene bake must be invalidated by a map change, not reused"
        );
        assert_eq!(
            app.minimap_static.relief.len(),
            crate::app::minimap_build::minimap_static_layers(&expected).relief.len(),
            "the minimap draws the map being played"
        );
    }

    /// Deploying from the garage starts a NEW battle, and last battle's wounds do not come with
    /// it. The per-tank presentation caches prune themselves by "is this id still in the
    /// snapshot" — a rule a fresh roster defeats, because it hands the same tank ids to healthy
    /// tanks. The blown-off turret then kept flying over its heir, the wreck kept its dented
    /// hull, and the thrown track band kept lying on a field it was never shed onto.
    #[test]
    fn a_fresh_battle_inherits_no_wreckage_from_the_one_it_replaces() {
        let mut app = ClientApp::new();
        // A BOT's id: tank ids restart at 1 with every battle, so the fresh roster hands this
        // very id to a different, healthy tank. (The player's own id is the one case that did
        // self-clean — deploying replaces the player's tank, which retires the old id.)
        let id = app
            .session
            .current_snapshot()
            .tanks
            .iter()
            .map(|tank| tank.tank_id)
            .find(|&id| id != app.player_tank)
            .expect("a 7v7 roster has bots");
        let mut popoff = crate::vehicle::turret_popoff::TurretPopoff::launch(
            id,
            VehicleKind::T54_1951,
            glam::Vec3::new(0.0, 2.0, 0.0),
            None,
        );
        popoff.tick(0.2);
        app.turret_popoffs.insert(id, popoff);
        app.wreck_hull_meshes.insert(id, renderer_api::MeshHandle(9_999));
        app.track_ribbons.push(crate::vehicle::track_ribbon::TrackRibbon::shed(
            id,
            VehicleKind::T54_1951,
            game_core::TrackSide::Left,
            glam::Vec3::new(400.0, 0.0, 400.0),
            0.0,
            None,
        ));

        app.open_garage();
        app.confirm_garage_selection();

        assert!(
            app.turret_popoffs.is_empty(),
            "no turret from the last battle flies over this one"
        );
        assert!(app.wreck_hull_meshes.is_empty(), "the fresh roster is not born dented");
        assert!(app.track_ribbons.is_empty(), "shed track bands do not survive the battle");
        assert!(app.wreck_age_s.is_empty(), "nobody deploys mid burn-out");
        assert!(app.motion_fx.is_empty(), "motion state belongs to the hull that earned it");
    }

    /// Stand in for the garage rendering at 60 FPS: `poll_map_prebake` is frame-driven, so a
    /// test that never draws a frame never lets the pick settle or the worker be harvested.
    fn run_garage_frames(app: &mut ClientApp, frames: usize) {
        for _ in 0..frames {
            app.poll_map_prebake();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// A settled map pick starts the world baking on a worker; the Battle press then CLAIMS that
    /// result instead of baking a second time. This is what turns a ~300 ms stall on the press
    /// into a GPU upload, and it has to stay wired — the fallback (bake in place) is silent, so
    /// only a test notices when the speculation stops being claimed.
    #[test]
    fn the_battle_press_claims_the_bake_the_settled_map_pick_started() {
        let target = terrain::MapId::Ostrogorsk;
        let mut app = ClientApp::new();
        app.open_garage();
        while app.garage.selected_map() != Some(target) {
            app.cycle_battle_map(1);
        }
        run_garage_frames(&mut app, 20);
        assert_eq!(
            app.map_prebake.as_ref().map(|prebake| prebake.map),
            Some(target),
            "a pick left standing must be baking"
        );

        app.confirm_garage_selection();

        assert!(app.map_prebake.is_none(), "the press consumes the bake it asked for");
        assert_eq!(
            map_forge::battlefield_hash(&app.battlefield),
            map_forge::battlefield_hash(&map_forge::battlefield(target)),
            "the claimed bake is the map that was picked"
        );
        assert!(
            app.battle_scene_meshes.is_some(),
            "the claimed bake arrives already meshed — nothing left to do on the press"
        );
    }

    /// Cycling the ring passes THROUGH maps on the way to the wanted one. Baking each stop
    /// would spend a core-second per click and hitch the garage that is drawing at that very
    /// moment, so a fly-past must schedule nothing at all.
    #[test]
    fn a_fly_past_through_the_map_ring_bakes_none_of_the_maps_it_crosses() {
        let mut app = ClientApp::new();
        app.open_garage();
        for _ in 0..terrain::MapId::SHIPPED.len() {
            app.cycle_battle_map(1);
        }
        assert!(app.map_prebake.is_none(), "no stop crossed at click speed earns a bake");
    }

    /// The world already loaded is never speculated on: `adopt_session_map` does not even run
    /// for an unchanged map, so a bake for it would be pure waste on the cores the garage is
    /// using to draw.
    #[test]
    fn the_map_already_loaded_is_never_speculatively_baked() {
        let mut app = ClientApp::new();
        let loaded = app.session.map_id();
        app.open_garage();
        while app.garage.selected_map() != Some(loaded) {
            app.cycle_battle_map(1);
        }
        run_garage_frames(&mut app, 20);
        assert!(
            app.map_prebake.as_ref().is_none_or(|prebake| prebake.map != loaded),
            "the world already loaded must never be baked again"
        );
    }

    fn player_rack_total(app: &ClientApp) -> u32 {
        app.session
            .latest_snapshot()
            .tanks
            .iter()
            .find(|tank| tank.tank_id == app.player_tank)
            .expect("player tank in snapshot")
            .ammo_counts
            .iter()
            .map(|&count| u32::from(count))
            .sum()
    }

    /// The second press of a double-click on BATTLE lands after the garage has closed, in the
    /// battle view — it used to latch the trigger and fire the battle's very first tick. The
    /// deploy shield swallows the residue; a press after the window still fires normally.
    #[test]
    fn the_second_press_of_a_double_click_on_battle_never_fires_the_first_shot() {
        use winit::event::MouseButton;

        let mut app = ClientApp::new();
        app.confirm_garage_selection(); // press 1: BATTLE — closes the garage
        let full = player_rack_total(&app);

        app.on_battle_mouse_press(MouseButton::Left); // press 2, milliseconds later
        app.run_fixed_ticks(crate::app::DEPLOY_FIRE_SHIELD_TICKS + 6);
        assert_eq!(player_rack_total(&app), full, "the double-click residue fires no shot");

        // The shield is a window, not a dead trigger.
        app.on_battle_mouse_press(MouseButton::Left);
        app.run_fixed_ticks(6);
        assert_eq!(player_rack_total(&app), full - 1, "an intentional press still fires");
    }

    /// Escape from the garage back into a live battle hands the mouse back to the gun. It used
    /// to leave the cursor free over a running battle — mouse look dead until a click, and that
    /// click then fired.
    #[test]
    fn escape_from_the_garage_over_a_live_battle_recaptures_the_cursor() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.open_garage();
        assert!(!app.cursor_captured, "the garage menu keeps the cursor free");

        app.garage_keyboard(PhysicalKey::Code(KeyCode::Escape));

        assert!(!app.garage.is_open(), "escape at hero rest closes the garage");
        assert!(app.cursor_captured, "back in the live battle the mouse is the gun again");
    }

    /// G over a live battle is a modal takeover exactly like the ESC menu: the held drive keys
    /// will never deliver their release to the battle, so they are dropped — the hull coasts to
    /// a stop instead of driving itself while its commander shops for modules.
    #[test]
    fn opening_the_garage_over_a_live_battle_stops_the_hull() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.input.forward = true;
        app.input.fire_pending = true;

        app.on_battle_keyboard(PhysicalKey::Code(KeyCode::KeyG), true);

        assert!(app.garage.is_open(), "G over a live battle opens the garage");
        assert_eq!(app.input.throttle(), 0.0, "held drive keys are released, not latched");
        assert!(!app.input.fire_pending, "a pending shot does not survive into the menu");
    }

    /// Whether an unfocused window ever sees the releases of what was held at the alt-tab is the
    /// platform's business (winit on Windows synthesizes them; others may not); the app drops
    /// every latch itself: the orbit drag glued to a button nobody holds, the hull driving itself
    /// on return. The battle-side latches (free look, Shift hold) are locked in `input_tests`.
    #[test]
    fn losing_focus_drops_every_latch_whose_release_will_never_arrive() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.open_garage();
        app.garage.begin_drag();
        app.input.forward = true;
        app.input.fire_pending = true;

        app.on_focus_change(false);

        assert!(!app.garage.is_dragging(), "alt-tab ends the orbit drag");
        assert_eq!(app.input.throttle(), 0.0, "no key release will arrive; drop the latch");
        assert!(!app.input.fire_pending, "a queued shot does not go off on return");
        assert!(!app.cursor_captured);

        // Refocus over the open garage keeps the pointer free — it is still a menu.
        app.on_focus_change(true);
        assert!(!app.cursor_captured, "the garage menu keeps the cursor free on refocus");
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

    /// Two clients driven identically must live the SAME battle. A played battle is seeded from
    /// the wall clock, so under test every deploy used to deal a different roster and every
    /// measurement downstream of where the tanks stand — the sight point, the bloom it commands —
    /// became a coin flip that read as "flaky under load" (register F8, G12).
    #[test]
    fn deploying_under_test_is_the_same_battle_every_time() {
        let fingerprint = || {
            let mut app = ClientApp::new();
            app.confirm_garage_selection();
            app.run_fixed_ticks(60);
            let snapshot = app.session.latest_snapshot();
            snapshot
                .tanks
                .iter()
                .map(|tank| {
                    format!(
                        "{}:{:?}:{:.4},{:.4}:{:.4}",
                        tank.tank_id.0,
                        tank.vehicle,
                        tank.position[0],
                        tank.position[2],
                        tank.turret_yaw_rad
                    )
                })
                .collect::<Vec<_>>()
                .join("|")
        };

        let first = fingerprint();
        let second = fingerprint();
        assert!(!first.is_empty(), "the deploy produces a roster");
        assert_eq!(first, second, "same input, same battle — roster, spawns and all");
    }
}
