//! The garage orbit/inspection camera and the `ClientApp` glue that turns cursor clicks into
//! selection, fitting edits, Battle, or camera drag. Kept apart from the state core in
//! [`super`] for reviewability; both operate on the same private [`GarageState`] fields.

#[cfg(test)]
use game_core::VehicleKind;
use winit::keyboard::PhysicalKey;

use super::{GarageHit, GarageState};
use crate::app::ClientApp;

/// The battle every test deploys into. One pinned seed for the whole suite, so a client test
/// measures the change under test instead of which roster the clock happened to deal it.
#[cfg(test)]
const TEST_BATTLE_SEED: u64 = 0x5748_4154_5F41_494D;

impl GarageState {
    /// The orbit drag, from a press on nothing in particular (the tests' word; a press on the
    /// scene goes through `press_scene`, which reads the hero first).
    #[cfg(test)]
    pub(super) fn begin_drag(&mut self) {
        self.drag = super::types::Drag::Camera;
        self.drag_travel_px = 0.0;
        self.hero_press = None;
    }

    /// `crate::app`-visible: focus loss must also drop the drag — an unfocused window never
    /// delivers the button release that would have ended it.
    pub(in crate::app) fn end_drag(&mut self) {
        self.drag = super::types::Drag::None;
        self.hero_press = None;
    }

    /// The cursor, in clip space, from the app. The interaction machine reads it at the
    /// frame's tick and at the press and the release (G6) — never here, per mouse event.
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
        self.refresh_garage_lock();
        self.refresh_garage_hints();
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

    /// G14: a destroyed hull is locked until its battle ends — the garage's BATTLE says how
    /// long. Read every garage frame from the battle the crew left.
    pub(in crate::app) fn refresh_garage_lock(&mut self) {
        let dead = self
            .render_state
            .latest_snapshot()
            .and_then(|s| s.tanks.iter().find(|t| t.tank_id == self.player_tank))
            .is_some_and(|t| t.hit_points == 0);
        let running = self.garage.has_started() && self.session.battle_outcome().is_none();
        let locked =
            (dead && running).then(|| self.session.battle_time_remaining_s().unwrap_or(0.0));
        self.garage.set_locked(locked);
    }

    /// G6: the legend and the tooltips print the table's keys — read every garage frame, so a
    /// rebinding on the keys page shows the moment the page closes.
    pub(in crate::app) fn refresh_garage_hints(&mut self) {
        self.garage.set_key_labels(super::hints::KeyLabels::from_table(&self.keybinds));
    }

    #[cfg(test)]
    pub(in crate::app) fn select_garage_vehicle(&mut self, vehicle: VehicleKind) {
        self.garage.select_vehicle(vehicle);
        // Mirrors the `GarageHit::Vehicle` click in `garage_primary_press`.
        self.queue_audio(audio::AudioEvent::UiClick { accent: false });
    }

    /// The primary button went down in the garage (G6): the machine takes the press; a press
    /// on the scene starts a drag (the camera, or the turret from a turret plate — G8); an open
    /// option list is modal and a press beside its rows closes it, swallowing the release. The
    /// controls themselves act on the RELEASE — `garage_primary_release` — the way every
    /// desktop has behaved for forty years.
    pub(in crate::app) fn garage_primary_press(&mut self) {
        let shift = self.input.shift;
        let hit = self.garage.hit_test(shift);
        self.garage.press_cursor();
        if self.garage.option_list().is_some() {
            if !matches!(hit, GarageHit::OptionRow(..)) {
                self.garage.close_option_list();
                self.garage.cancel_press();
            }
            return;
        }
        if hit == GarageHit::Scene {
            self.garage.press_scene();
        }
    }

    /// The primary button came up: a click on the control that was pressed, or the end of a
    /// drag — and a press on a module's plate that never travelled opens that module (G8).
    pub(in crate::app) fn garage_primary_release(&mut self) {
        let shift = self.input.shift;
        let released = self.garage.release_cursor();
        if let Some(hit) = self.garage.end_press() {
            // G11: under the inspector a click on a plate is the question, not a shop.
            if self.garage.inspector_on() {
                self.queue_audio(audio::AudioEvent::UiClick { accent: false });
                self.garage
                    .set_inspector_point(Some(super::inspector::InspectorPoint::from_hit(&hit)));
                return;
            }
            if let Some(slot) = hit.slot {
                self.queue_audio(audio::AudioEvent::UiClick { accent: false });
                self.garage.set_focused_slot(slot);
                self.garage.open_option_list(slot);
                self.garage.focus_module(slot);
            }
            return;
        }
        if released.is_none() {
            return;
        }
        let hit = self.garage.hit_test(shift);
        self.garage_act(hit, shift);
    }

    /// A press and its release on the same spot: the click, for the locks.
    #[cfg(test)]
    pub(in crate::app) fn garage_click(&mut self) {
        self.garage_primary_press();
        self.garage_primary_release();
    }

    /// What a click does: selection, fitting, Battle, the map, the tabs, the chips, compare.
    /// Shift held while clicking a module slot cycles that slot backward.
    fn garage_act(&mut self, hit: GarageHit, shift: bool) {
        let view = self.garage.view();
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
        if !matches!(hit, GarageHit::Scene | GarageHit::Battle | GarageHit::Locked) {
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
            // G3: the compared hull.
            GarageHit::Compare(index) => self.garage.toggle_compare(index),
            // G9: a chip's ring, forward on a click, back on a shift-click.
            GarageHit::Chip(chip, dir) => self.garage.cycle_chip(chip, dir),
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
            // G14: the hull is still in a battle — the button says so; a click is a knock.
            GarageHit::Locked => self.queue_audio(audio::AudioEvent::UiReject),
            GarageHit::MapCycle(dir) => self.cycle_battle_map(dir),
            // G10: a tab opens its screen.
            GarageHit::Tab(tab) => self.open_garage_tab(tab),
            // G11: SHOOT ME answers the question with the loaded round.
            GarageHit::ShootMe => self.garage.toggle_shoot_me(),
            // The scene's press already took the camera (or the turret) in `garage_primary_press`;
            // in the tree view a press on nothing is a press on nothing.
            GarageHit::Scene => {}
        }
    }

    /// A tab on the garage's bar (G10): GARAGE and TECH TREE are the garage's own views,
    /// ARMOUR the hangar with the inspector on; BATTLES, REPLAYS, STATISTICS and SETTINGS open
    /// the shell's pages over the hall — Esc on those returns to the hall.
    pub(in crate::app) fn open_garage_tab(&mut self, tab: super::GarageTab) {
        use super::GarageTab;
        match tab {
            GarageTab::Garage => {
                self.garage.close_tech_tree();
                self.garage.set_inspector(false);
            }
            GarageTab::TechTree => self.garage.open_tech_tree(),
            GarageTab::Armour => {
                self.garage.close_tech_tree();
                self.garage.set_inspector(true);
            }
            GarageTab::Battles => {
                self.open_shell_page_from_tab(crate::app::shell::ShellPage::Battles)
            }
            GarageTab::Replays => {
                self.open_shell_page_from_tab(crate::app::shell::ShellPage::Replays)
            }
            GarageTab::Statistics => {
                self.open_shell_page_from_tab(crate::app::shell::ShellPage::Statistics)
            }
            GarageTab::Settings => {
                self.open_shell_page_from_tab(crate::app::shell::ShellPage::Settings)
            }
        }
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
        // The open garage owns the keyboard: a key it does not bind is swallowed, so a
        // keystroke never leaks through to drive the tank or switch ammo in the battle running
        // underneath. `on_keyboard` only routes here while the garage is open.
        if let Some(action) = self.keybinds.action(crate::app::keybinds::Context::Garage, key) {
            self.garage_action(action);
        }
        true
    }

    /// The garage's actions (P7: the table's word for the key).
    fn garage_action(&mut self, action: crate::app::keybinds::Action) {
        use crate::app::keybinds::Action as A;
        match action {
            // Arrow keys cycle the roster. The old 1-5 vehicle digits are retired: with a scroll
            // window, a window-relative digit selects a different tank than the label implies.
            A::GaragePrev => self.garage.cycle(-1),
            A::GarageNext => self.garage.cycle(1),
            A::GarageConfirm => self.confirm_garage_selection(),
            // Escape peels back one layer at a time: first an open option list, then a module-focus
            // framing (return to hero), then — camera already at rest — closes the garage.
            A::GarageBack => {
                if self.garage.option_list().is_some() {
                    self.garage.close_option_list();
                } else if self.garage.is_camera_off_hero() {
                    self.garage.return_to_hero_view();
                } else if self.garage.has_started() {
                    self.garage.close_if_started();
                    // Mirrors `close_pause_menu`: back in the live battle the mouse is the gun
                    // again — recapture it, and drop the motion accumulated while it was a
                    // pointer so the turret does not jump on the first frame.
                    self.input.clear_mouse_look();
                    self.set_cursor_captured(true);
                } else {
                    // P8: a cold garage's Esc raises its own menu — SETTINGS, KEY BINDINGS, QUIT.
                    self.open_menu(crate::hud::shell::MenuKind::Garage);
                }
            }
            // Keyboard loadout editing: focus + cycle + ammo + crew.
            A::FocusPrev => self.garage.focus_adjacent(-1),
            A::FocusNext => self.garage.focus_adjacent(1),
            A::CycleFocusedPrev => {
                self.garage.cycle_focused(-1);
                self.garage_reject_feedback();
            }
            A::CycleFocusedNext => {
                self.garage.cycle_focused(1);
                self.garage_reject_feedback();
            }
            A::GarageAmmo1 => self.garage.set_ammo(0),
            A::GarageAmmo2 => self.garage.set_ammo(1),
            A::GarageAmmo3 => self.garage.set_ammo(2),
            A::GarageMap => self.garage.cycle_map(1),
            // H1: the hall's daylight — Auto (the player's clock) → Morning → Day → Evening.
            A::Daylight => {
                self.garage.cycle_daylight();
                let light = self.garage.daylight_override();
                self.edit_settings(|settings| settings.set_daylight(light));
                self.queue_audio(audio::AudioEvent::UiClick { accent: false });
            }
            // I1: the armor inspector — the gameplay armor volumes over the parked hero.
            A::Inspector => {
                self.garage.toggle_inspector();
                self.queue_audio(audio::AudioEvent::UiClick { accent: false });
            }
            // L2: repair — only a marked hero has anything to fix; the beat opens with the
            // heavier hand on the switch and closes with the shop's finishing clunk (the
            // completion sound queues from `tick_repair` in the render loop).
            A::Repair => {
                if self.garage.start_repair() {
                    self.queue_audio(audio::AudioEvent::UiClick { accent: true });
                    // ...and the shop actually WORKS for the beat (R2): the wrench spans the
                    // same seconds the lift and the nameplate do, one source for all three.
                    self.queue_audio(audio::AudioEvent::RepairWork {
                        seconds: super::wear::REPAIR_BEAT_S,
                    });
                }
            }
            A::TechTree => match self.garage.view() {
                super::GarageView::Hangar => self.garage.open_tech_tree(),
                super::GarageView::TechTree => self.garage.close_tech_tree(),
            },
            // Another context's word: not this router's.
            _ => {}
        }
    }

    /// Turn on garage disk persistence (selected vehicle + per-vehicle loadouts survive restarts).
    /// Called once from the real startup path; `ClientApp::new` stays pure so tests never touch
    /// the user's save file.
    pub(in crate::app) fn enable_garage_persistence(&mut self) {
        self.garage.enable_persistence(super::persistence::save_path());
    }

    pub(in crate::app) fn confirm_garage_selection(&mut self) {
        // G14: a hull locked in a running battle does not deploy again until it ends.
        if self.garage.is_locked() {
            self.queue_audio(audio::AudioEvent::UiReject);
            return;
        }
        // The commit deserves a heavier hand on the switch than browsing.
        self.queue_audio(audio::AudioEvent::UiClick { accent: true });
        let spec = self.garage.confirm();
        let display_name = spec.name.clone();
        // Committing from the garage in a random battle ABANDONS it and deploys into a fresh one
        // (new seed, full roster, full clock). Replacing the player's tank inside the running
        // battle was a free heal: G mid-fight, confirm, and the hull came back factory-new while
        // everyone else stayed shot up. It also closes the loop after VICTORY/DEFEAT/DRAW — the
        // garage's Battle button IS the next battle.
        if self.session.battle_mode() != battle_host::BattleMode::PracticeDuel {
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
            // M3 (`docs/game-modes.md`): the garage's BATTLE is the AI battle — 15v15, the
            // player and marked bots, offline. The button keeps its word until M7b puts the
            // online queue beside it (one re-bless of the garage's frame, not two).
            self.session = crate::app::session::BattleSessionKind::Local(Box::new(
                battle_host::LocalAuthoritativeServer::new_ai_battle(
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
            // P3: the inbox starts with the battle (the record below, once the seat is known).
            self.intel = crate::app::battle_intel::BattleIntel::default();
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
        // P3: a battle's record starts with the battle, in the crew's own seat.
        self.ledger = crate::app::ledger::BattleLedger::new(self.player_tank);
        self.results_shown = false;
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
        // The very first deploy has no world in hand (the window opened on the garage without
        // baking one): claim the bake that ran behind the hall, so the first battle frame
        // uploads instead of baking. No finished bake means that frame bakes in place — the
        // cost the old startup paid before the window even opened.
        self.claim_prebaked_world_for_current_map();
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
    use super::super::elements::GarageElement as E;
    use super::*;
    use winit::keyboard::KeyCode;

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
        assert!(app.garage.set_cursor_on(E::ModuleSlot(1))); // Gun slot
        let before = app.garage.draft().gun_name();
        app.garage_secondary_press();
        let after = app.garage.draft().gun_name();
        assert_ne!(before, after, "right-click cycles the gun backward");
    }

    #[test]
    fn right_click_on_battle_does_not_fire() {
        let mut app = ClientApp::new();
        assert!(app.garage.set_cursor_on(E::BattleButton));
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
        assert!(app.garage.set_cursor_on(E::ModuleSlot(1))); // Gun slot
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
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::T54_1951);
        let before = app.garage.draft().ammo_counts()[0];

        assert!(app.garage.set_cursor_on(E::AmmoMinus(0)));
        app.garage_click();
        assert_eq!(app.garage.draft().ammo_counts()[0], before - 1, "plain click moves one round");

        app.input.set_shift(true);
        app.garage_click();
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
        assert!(app.garage.set_cursor_on(E::AmmoSlot(1)));
        app.garage_secondary_press();
        assert_eq!(app.garage.draft().ammo_index(), before, "right-click does not touch ammo");
    }

    #[test]
    fn plain_click_on_a_swappable_slot_opens_its_option_list_without_changing_the_fit() {
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::T54_1951);
        assert!(app.garage.set_cursor_on(E::ModuleSlot(1))); // Gun slot (a real choice on the T-54)
        let stock = app.garage.draft().gun_name();

        app.garage_click();

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
        assert!(app.garage.set_cursor_on(E::ModuleSlot(1))); // Gun slot
        let stock = app.garage.draft().gun_name();

        // Shift+click is the express path: it cycles backward (from stock, wraps to the alternate
        // gun) and never opens the list.
        app.input.set_shift(true);
        app.garage_click();

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
        assert!(app.garage.set_cursor_on(E::ModuleSlot(1)));
        app.garage_click();
        assert_eq!(app.garage.option_list(), Some(FitSlot::Gun));

        assert!(app.garage.set_cursor_on(E::OptionRow(1)));
        app.garage_click();

        assert_eq!(app.garage.option_list(), None, "picking a row closes the list");
        assert_ne!(app.garage.draft().gun_name(), stock, "the picked gun is installed");
    }

    #[test]
    fn clicking_outside_an_open_list_dismisses_it_without_acting() {
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::T54_1951);
        assert!(app.garage.set_cursor_on(E::ModuleSlot(1)));
        app.garage_click();
        assert_eq!(app.garage.option_list(), Some(FitSlot::Gun));

        // A click in empty scene space dismisses the list and must not start a camera drag.
        app.garage.set_cursor([0.0, 0.0]);
        app.garage_click();
        assert_eq!(app.garage.option_list(), None, "clicking away closes the list");
        assert!(!app.garage.is_dragging(), "the dismiss click does not start an orbit drag");
    }

    #[test]
    fn selecting_vehicle_from_tech_tree_returns_to_hangar() {
        use super::super::GarageView;

        let mut app = ClientApp::new();
        app.garage.open_tech_tree();
        assert_eq!(app.garage.view(), GarageView::TechTree);

        let tiger =
            VehicleKind::PLAYABLE.iter().position(|k| *k == VehicleKind::TigerI).expect("playable");
        assert!(app.garage.set_cursor_on(E::TreeNode(tiger as u8)));
        app.garage_click();

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

        let format = game_core::BattleFormat::FifteenVsFifteen;
        assert_eq!(app.session.authoritative_tick(), 0, "a FRESH battle, not a respawn");
        assert_eq!(app.session.latest_snapshot().tanks.len(), format.total_seats(), "full roster");
        assert_eq!(app.session.battle_mode(), battle_host::BattleMode::AiBattle);
        assert_eq!(app.session.battle_outcome(), None);
        assert_eq!(
            app.session.battle_time_remaining_s(),
            Some(format.time_limit_s() as f32),
            "the battle clock starts full again — the AI battle's fifteen minutes"
        );
    }

    #[test]
    fn confirming_garage_selection_keeps_random_7v7_roster() {
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::IS3);

        app.confirm_garage_selection();

        let full_snapshot = app.session.latest_snapshot();
        assert_eq!(
            full_snapshot.tanks.len(),
            game_core::BattleFormat::FifteenVsFifteen.total_seats()
        );
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
        assert!(app.garage.set_cursor_on(E::MapRow));
        while app.garage.selected_map() != Some(terrain::MapId::Ostrogorsk) {
            app.garage_click();
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

    /// At startup nothing is baked — the window opens on the garage first — so AUTO speculates
    /// on the SESSION's map behind the hall, and the Battle press claims that world instead of
    /// baking it in the first battle frame.
    #[test]
    fn startup_bakes_the_session_map_behind_the_garage_and_the_deploy_claims_it() {
        let mut app = ClientApp::new();
        assert!(app.battle_scene_meshes.is_none(), "premise: the window opens unbaked");
        assert!(app.garage.selected_map().is_none(), "premise: AUTO");
        let session_map = app.session.map_id();
        run_garage_frames(&mut app, 12);
        assert_eq!(
            app.map_prebake.as_ref().map(|prebake| prebake.map),
            Some(session_map),
            "AUTO with no world in hand bakes the session's map"
        );

        app.confirm_garage_selection();
        assert!(
            app.battle_scene_meshes.is_some(),
            "the press claims the speculative world (waiting for the worker if it must)"
        );
        assert!(app.scene_upload_dirty, "and the first battle frame uploads it");
        assert!(app.map_prebake.is_none(), "the claim consumed the bake");
    }

    /// The world already in hand is never speculated on: `adopt_session_map` does not even run
    /// for an unchanged map, so a bake for it would be pure waste on the cores the garage is
    /// using to draw.
    #[test]
    fn the_map_already_loaded_is_never_speculatively_baked() {
        let mut app = ClientApp::new();
        let loaded = app.session.map_id();
        // A real deployment leaves the world baked and cached; only then is there nothing to do.
        app.ensure_battle_scene_meshes();
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

        let mut app = ClientApp::new();
        app.garage.open_tech_tree();
        assert!(app.garage.set_cursor_on(E::TreeBack));
        app.garage_click();
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

    /// G8: a click on a module's plate on the hero — the cursor's ray through the camera, the
    /// trace's own volumes — focuses that module and opens its list when it has a choice; a
    /// press that travels is the orbit drag and opens nothing; the floor beside the hero is
    /// the camera's alone.
    #[test]
    fn clicking_a_module_on_the_hero_opens_its_slot() {
        use crate::app::garage::hero_pick::hero_point_where;

        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::BENCHMARK);
        let mut opened = 0;
        for slot in [FitSlot::Gun, FitSlot::Hull, FitSlot::Engine, FitSlot::Suspension] {
            let Some(clip) =
                hero_point_where(&mut app.garage, |hit| hit.slot == Some(slot) && !hit.turret)
            else {
                continue;
            };
            app.garage.set_cursor(clip);
            app.garage.close_option_list();
            app.garage_primary_press();
            assert!(
                app.garage.is_dragging(),
                "a press on the hero holds the camera until it travels"
            );
            app.garage_primary_release();
            assert_eq!(app.garage.focused_slot(), slot, "the click focuses the module");
            let has_choice = app.garage.draft().has_choice(slot);
            assert_eq!(app.garage.option_list(), has_choice.then_some(slot), "{slot:?}");
            opened += usize::from(has_choice);
            // A press that travels orbits the camera and opens nothing.
            app.garage.close_option_list();
            let eye = app.garage.orbit_camera().eye;
            app.garage_primary_press();
            app.garage.apply_drag(40.0, 0.0);
            app.garage_primary_release();
            assert_eq!(app.garage.option_list(), None, "a drag is not a click");
            assert_ne!(app.garage.orbit_camera().eye, eye, "the drag orbited the camera");
        }
        assert!(opened >= 1, "at least one module with a real choice was clicked open");
        // The roof of the hall: nothing to click, the camera to drag.
        app.garage.set_cursor([0.95, 0.95]);
        app.garage_primary_press();
        assert!(app.garage.is_dragging());
        app.garage_primary_release();
        assert_eq!(app.garage.option_list(), None);
        assert!(!app.garage.is_dragging());
    }

    /// G8: a press on a turret plate and a drag turn the turret — the camera, the selection,
    /// the loadout and the focus stay exactly where they were; a drag from a hull plate turns
    /// the camera and leaves the turret; a new hull parks its turret straight.
    #[test]
    fn dragging_the_turret_turns_it_and_nothing_else() {
        use crate::app::garage::hero_pick::hero_point_where;

        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::BENCHMARK);
        let turret = hero_point_where(&mut app.garage, |hit| hit.turret).expect("a turret plate");
        app.garage.set_cursor(turret);
        let before = app.garage.clone();
        app.garage_primary_press();
        app.garage.apply_drag(80.0, 30.0);
        app.garage_primary_release();
        assert!(app.garage.hero_turret_yaw().abs() > 0.1, "the turret turned");
        assert_eq!(app.garage.orbit_camera().eye, before.orbit_camera().eye, "the camera did not");
        assert_eq!(app.garage.selected_vehicle(), before.selected_vehicle());
        assert_eq!(app.garage.draft(), before.draft());
        assert_eq!(app.garage.focused_slot(), before.focused_slot());
        assert_eq!(app.garage.option_list(), None);
        assert!(!app.garage.is_dragging());
        // The render parks the turret where the drag left it.
        let mut snapshot =
            crate::app::garage_render::garage_preview_snapshot(VehicleKind::BENCHMARK);
        snapshot.turret_yaw_rad = app.garage.hero_turret_yaw();
        assert_ne!(snapshot.turret_yaw_rad, 0.0);
        // A hull plate: the camera's drag, the turret untouched.
        let hull = hero_point_where(&mut app.garage, |hit| !hit.turret && hit.slot.is_some())
            .expect("a hull plate");
        app.garage.set_cursor(hull);
        let yaw = app.garage.hero_turret_yaw();
        app.garage_primary_press();
        app.garage.apply_drag(80.0, 0.0);
        app.garage_primary_release();
        assert_eq!(app.garage.hero_turret_yaw(), yaw, "a hull drag leaves the turret");
        assert_ne!(app.garage.orbit_camera().eye, before.orbit_camera().eye, "and orbits");
        // A fresh hull parks straight.
        app.garage.select_vehicle(VehicleKind::PLAYABLE[1]);
        assert_eq!(app.garage.hero_turret_yaw(), 0.0);
    }

    /// G10: the seven tabs — GARAGE and TECH TREE the garage's own views, ARMOUR the hangar
    /// with the inspector on, BATTLES / REPLAYS / STATISTICS / SETTINGS the shell's pages over
    /// the hall — and Esc on a page a tab opened returns to the hall with no menu between; a
    /// page the menu opened still goes back to the menu (P8).
    #[test]
    fn every_tab_opens_its_screen_and_escape_returns_to_the_hall() {
        use super::super::GarageView;
        use super::super::types::GarageTab;
        use crate::app::shell::ShellPage;
        use crate::hud::shell::{MenuItem, MenuKind};

        let mut app = ClientApp::new();
        let click_tab = |app: &mut ClientApp, tab: GarageTab| {
            assert!(app.garage.set_cursor_on(E::tab(tab)), "{tab:?} is on the bar");
            app.garage_click();
        };
        click_tab(&mut app, GarageTab::TechTree);
        assert_eq!(app.garage.view(), GarageView::TechTree);
        assert_eq!(app.garage.active_tab(), GarageTab::TechTree);
        click_tab(&mut app, GarageTab::Armour);
        assert_eq!(app.garage.view(), GarageView::Hangar);
        assert!(app.garage.inspector_on(), "ARMOUR is the hangar with the inspector on");
        assert_eq!(app.garage.active_tab(), GarageTab::Armour);
        click_tab(&mut app, GarageTab::Garage);
        assert!(!app.garage.inspector_on());
        assert_eq!(app.garage.active_tab(), GarageTab::Garage);
        for (tab, page) in [
            (GarageTab::Battles, ShellPage::Battles),
            (GarageTab::Replays, ShellPage::Replays),
            (GarageTab::Statistics, ShellPage::Statistics),
            (GarageTab::Settings, ShellPage::Settings),
        ] {
            click_tab(&mut app, tab);
            assert_eq!(app.shell_page(), Some(page), "{tab:?} opens its page");
            app.on_battle_keyboard(PhysicalKey::Code(KeyCode::Escape), true);
            assert!(!app.shell_open(), "{tab:?}: Esc returns to the hall, no menu between");
            assert!(app.garage.is_open());
        }
        // The menu's own pages keep their way back: to the menu.
        app.open_menu(MenuKind::Garage);
        app.click_menu_item(MenuItem::Battles);
        assert_eq!(app.shell_page(), Some(ShellPage::Battles));
        app.on_battle_keyboard(PhysicalKey::Code(KeyCode::Escape), true);
        assert_eq!(app.shell_page(), Some(ShellPage::Menu(MenuKind::Garage)));
    }

    /// G11: with the inspector on, a click on a plate is the question — the readout names the
    /// zone, its steel and the steel at the ray's angle, the marker sits on the plate — and
    /// SHOOT ME answers it with the loaded round's penetration and the resolver's verdict; a
    /// new hull or the inspector going off drops the question.
    #[test]
    fn a_click_under_the_inspector_asks_the_plate_and_shoot_me_answers_with_the_own_round() {
        use crate::app::garage::hero_pick::hero_point_where;
        use crate::app::garage::screen::build_screen_list;
        use crate::hud::damage_log::zone_name;
        use game_core::ArmorZone;
        use ui_kit::draw_list::Payload;

        let text_of = |app: &ClientApp, id: E| -> Option<String> {
            let ui = app.garage.ui();
            match build_screen_list(&app.garage, &ui, None).find(id).map(|e| e.payload.clone()) {
                Some(Payload::Text { text, .. }) => Some(text),
                _ => None,
            }
        };
        let mut app = ClientApp::new();
        app.garage.select_vehicle(VehicleKind::BENCHMARK);
        app.garage_keyboard(PhysicalKey::Code(KeyCode::KeyI));
        assert!(app.garage.inspector_on());
        assert_eq!(
            text_of(&app, E::InspectorLine(0)).as_deref(),
            Some(crate::ui_strings::garage::INSPECTOR_HINT)
        );
        assert!(text_of(&app, E::InspectorLine(1)).is_none(), "no verdict before SHOOT ME");
        let glacis = hero_point_where(&mut app.garage, |hit| hit.zone == ArmorZone::UpperGlacis)
            .expect("the glacis faces the lens");
        app.garage.set_cursor(glacis);
        app.garage_click();
        let point = app.garage.inspector_point().expect("the question");
        assert_eq!(point.zone, ArmorZone::UpperGlacis);
        assert_eq!(
            app.garage.option_list(),
            None,
            "under the inspector a click asks, it does not shop"
        );
        let first = text_of(&app, E::InspectorLine(0)).expect("the plate's line");
        assert!(first.starts_with(zone_name(ArmorZone::UpperGlacis)), "{first}");
        assert!(first.ends_with(crate::ui_strings::garage::INSPECTOR_EFFECTIVE), "{first}");
        let (marker, _, lamp) =
            app.garage.inspector_marker(&ui_kit::theme::Theme::standard()).expect("the marker");
        assert!((marker - point.hit_position).length() < 1.0e-6, "the marker sits on the plate");
        // SHOOT ME: the loaded round, its penetration at 100 m, the resolver's word.
        assert!(app.garage.set_cursor_on(E::InspectorShootMe));
        app.garage_click();
        assert!(app.garage.shoot_me());
        let reading = app.garage.inspector_reading(point, &app.garage.own_round());
        let second = text_of(&app, E::InspectorLine(1)).expect("the verdict's line");
        assert!(second.starts_with(&reading.round), "{second}");
        assert!(second.ends_with(reading.verdict_word()), "{second}");
        assert!(
            second.contains(&format!("{} ", reading.penetration_mm.round() as i32)),
            "{second}"
        );
        let (_, _, verdict_rgb) =
            app.garage.inspector_marker(&ui_kit::theme::Theme::standard()).expect("the marker");
        assert_ne!(verdict_rgb, lamp, "the marker wears the verdict once SHOOT ME answers");
        // A new hull drops the question; the inspector going off drops it too.
        app.garage.select_vehicle(VehicleKind::PLAYABLE[1]);
        assert!(app.garage.inspector_point().is_none());
        assert!(app.garage.inspector_on());
        let plate = hero_point_where(&mut app.garage, |hit| hit.slot.is_some()).expect("a plate");
        app.garage.set_cursor(plate);
        app.garage_click();
        assert!(app.garage.inspector_point().is_some());
        app.garage_keyboard(PhysicalKey::Code(KeyCode::KeyI));
        assert!(!app.garage.inspector_on() && app.garage.inspector_point().is_none());
    }
}

#[cfg(test)]
mod lock_tests {
    use super::super::elements::GarageElement as E;
    use super::*;

    /// G14: a destroyed hull is locked until its battle ends, and the garage says so — BATTLE
    /// wears IN BATTLE with the clock, a click on it knocks, ENTER does not deploy; the moment
    /// the battle ends the hull is free and BATTLE commits again.
    #[test]
    fn a_destroyed_vehicle_is_locked_until_its_battle_ends_and_says_so() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.run_fixed_ticks(5);
        let player = app.player_tank;
        let player_team = app.player_team();
        let enemies: Vec<game_core::TankId> = app
            .session
            .roster()
            .iter()
            .filter(|entry| entry.team != player_team)
            .map(|entry| entry.tank_id)
            .collect();
        let crate::app::session::BattleSessionKind::Local(server) = &mut app.session else {
            panic!("the desktop battle")
        };
        server.knock_out_for_test(player);
        app.run_fixed_ticks(3);
        app.open_garage();
        assert!(app.garage.is_locked(), "a dead crew's hull is locked in its battle");
        let ui = app.garage.ui();
        let list = crate::app::garage::screen::build_screen_list(&app.garage, &ui, None);
        assert_eq!(
            list.find(E::BattleButton).expect("battle").state,
            ui_kit::draw_list::WidgetState::Disabled
        );
        let ticks_before = app.session.authoritative_tick();
        app.pending_audio.clear();
        assert!(app.garage.set_cursor_on(E::BattleButton));
        app.garage_click();
        assert!(
            app.pending_audio.contains(&audio::AudioEvent::UiReject),
            "a click on the lock knocks"
        );
        assert!(app.garage.is_open(), "and deploys nothing");
        app.confirm_garage_selection();
        assert!(
            app.garage.is_open() && app.session.authoritative_tick() == ticks_before,
            "ENTER neither"
        );
        // The battle ends: every enemy gone; the next garage frame frees the hull.
        let crate::app::session::BattleSessionKind::Local(server) = &mut app.session else {
            panic!("the desktop battle")
        };
        for enemy in &enemies {
            server.knock_out_for_test(*enemy);
        }
        app.garage.close_for_test();
        app.run_fixed_ticks(3);
        assert!(app.battle_outcome.is_some());
        app.open_garage();
        assert!(!app.garage.is_locked(), "the battle is over: the hull is free");
        app.confirm_garage_selection();
        assert!(!app.garage.is_open(), "BATTLE commits again");
    }
}
