//! The client loop's step side: dispatching the loop driver's actions and running the fixed
//! 60 Hz simulation ticks (input → prediction → local server → snapshot ingest). Split from
//! `app/mod.rs` so the module root stays a declaration of state, not behavior.

use net::ClientInputCommand;
use sim::TankCommand;
use winit::event_loop::ActiveEventLoop;

use super::ClientApp;
use crate::ClientLoopAction;

impl ClientApp {
    pub(super) fn handle_actions(
        &mut self,
        event_loop: &ActiveEventLoop,
        actions: Vec<ClientLoopAction>,
    ) {
        for action in actions {
            match action {
                ClientLoopAction::CaptureInput => {}
                ClientLoopAction::RunFixedTicks(count) => self.run_fixed_ticks(count),
                ClientLoopAction::RequestRedraw => {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
                ClientLoopAction::RenderFrame => self.render_now(),
                ClientLoopAction::Resize { width, height } => {
                    self.viewport = (width.max(1), height.max(1));
                    if let Some(renderer) = &mut self.renderer {
                        renderer.resize(width, height);
                    }
                }
                ClientLoopAction::Exit => event_loop.exit(),
            }
        }
    }

    pub(super) fn run_fixed_ticks(&mut self, count: u32) {
        if !self.garage.has_started() {
            self.input.fire_pending = false;
            return;
        }
        let mut fire = self.input.fire_pending;
        self.input.fire_pending = false;
        // Like the fire latch: the switch request rides exactly one command (the batch's first).
        let mut select_ammo = self.input.pending_ammo_select.take();
        for _ in 0..count {
            if !self.session.prepare_player_tick() {
                // Keep pumping/rendering, camera look and G-to-garage alive, but stop inventing
                // authoritative motion. Retire one-shot edges rather than firing them next match.
                fire = false;
                select_ammo = None;
                let outcome = self.session.take_pending_remote_tick();
                self.apply_armor_breach_deltas(outcome.armor_breaches);
                if let Some(snapshot) = outcome.snapshot {
                    if let Some(reconciliation) = outcome.reconciliation {
                        self.accept_remote_and_sync(snapshot, reconciliation);
                    } else {
                        self.accept_and_sync(snapshot);
                    }
                }
                self.predictor.freeze_motion();
                self.refresh_battle_outcome();
                continue;
            }
            self.seed_prediction();
            if fire {
                self.register_fire_intent_feedback();
            }
            // Mouse look is applied per rendered frame (see `render_now`); the fixed step only
            // consumes the resulting aim. The sight sweep runs EVERY tick — the sight point moves
            // as the predicted hull advances, and a catch-up batch reusing one stale solution
            // overshoots the turret each batch, wobbling the reticle exactly when FPS dips.
            let solution = self.sight_solution();
            let turret_yaw_delta = self.turret_tracking_command_for(solution.as_ref());
            let gun_pitch_delta = self.gun_elevation_command_for(solution.as_ref());
            let command = TankCommand {
                throttle: self.input.throttle(),
                steer: -self.input.steer(),
                brake: self.input.brake_value(),
                turret_yaw_delta,
                gun_pitch_delta,
                fire,
                select_ammo: select_ammo.take(),
            };
            fire = false;
            self.step_prediction(&command);
            let outcome = self.session.tick_with_player_input(ClientInputCommand {
                client_tick: self.client_tick,
                tank_id: self.player_tank,
                command,
            });
            self.client_tick += 1;
            self.ticks_since_snapshot = self.ticks_since_snapshot.saturating_add(1);
            self.apply_armor_breach_deltas(outcome.armor_breaches);
            if let Some(snapshot) = outcome.snapshot {
                if let Some(reconciliation) = outcome.reconciliation {
                    self.accept_remote_and_sync(snapshot, reconciliation);
                } else {
                    self.accept_and_sync(snapshot);
                }
            }
            self.refresh_battle_outcome();
        }
    }

    fn refresh_battle_outcome(&mut self) {
        self.battle_outcome = self
            .session
            .battle_outcome()
            .map(|outcome| match outcome.winning_team() {
                Some(team) if team == self.player_team() => crate::hud::BattleHudOutcome::Victory,
                Some(_) => crate::hud::BattleHudOutcome::Defeat,
                None => crate::hud::BattleHudOutcome::Draw,
            })
            .or_else(|| {
                self.session.remote_terminal_reason().map(|reason| match reason {
                    super::session::RemoteTerminalReason::BattleOver => {
                        crate::hud::BattleHudOutcome::BattleOver
                    }
                    super::session::RemoteTerminalReason::TimedOut
                    | super::session::RemoteTerminalReason::Refused
                    | super::session::RemoteTerminalReason::CombatEventOverflow
                    | super::session::RemoteTerminalReason::CombatEventGap
                    | super::session::RemoteTerminalReason::Transport
                    | super::session::RemoteTerminalReason::SnapshotStalled
                    | super::session::RemoteTerminalReason::InputBacklog
                    | super::session::RemoteTerminalReason::MapMismatch => {
                        crate::hud::BattleHudOutcome::ConnectionLost
                    }
                })
            });
    }
}

impl super::ClientApp {
    /// The audible/visible answer to every fire click, the instant it is issued. The server
    /// held or refused silently before: a click landing a hair early (inside the sim's
    /// FIRE_BUFFER_S) is now HELD server-side — acknowledge it with the soft mechanical click;
    /// a genuinely refused click (still reloading, empty slot, dead gun/rack) gets the UiReject
    /// knock plus the red reticle pulse, so a shot that does not happen is REFUSED, never
    /// swallowed. Classification reads the latest authoritative snapshot — presentation only,
    /// the server still decides.
    fn register_fire_intent_feedback(&mut self) {
        let Some(tank) = self.player_snapshot() else {
            return;
        };
        if tank.hit_points == 0 {
            return;
        }
        let gun_out = tank.destroyed_modules_mask
            & (game_core::ModuleSlot::Gun.destroyed_mask_bit()
                | game_core::ModuleSlot::AmmoRack.destroyed_mask_bit())
            != 0;
        let selected = (tank.selected_ammo as usize).min(game_core::MAX_AMMO_SLOTS - 1);
        let slot_empty = tank.ammo_counts[selected] == 0;
        let reloading_long = tank.reload_remaining_s > sim::FIRE_BUFFER_S;
        let held = tank.reload_remaining_s > 0.0 && !reloading_long;
        if gun_out || slot_empty || reloading_long {
            self.fire_denied_age_s = Some(0.0);
            self.queue_audio(audio::AudioEvent::UiReject);
        } else if held {
            // Inside the buffer window: the sim holds the click and fires on the ready tick —
            // acknowledge the trigger squeeze so the held shot feels intentional.
            self.queue_audio(audio::AudioEvent::UiClick { accent: false });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ClientApp;

    #[test]
    fn battle_outcome_freezes_prediction_and_discards_one_shot_edges() {
        let mut app = ClientApp::new_seeded(0x51_0A_D0);
        app.confirm_garage_selection();
        app.input.forward = true;
        app.run_fixed_ticks(120);
        assert!(app.predictor.speed_mps() > 0.5, "precondition: held W moved the local hull");

        let stop_tick = match &mut app.session {
            super::super::session::BattleSessionKind::Local(server) => {
                let stop_tick = server.authoritative_tick() + 1;
                server.override_battle_time_limit_ticks(Some(stop_tick));
                stop_tick
            }
            super::super::session::BattleSessionKind::Remote(_) => {
                unreachable!("seeded app is local")
            }
        };
        app.run_fixed_ticks(2);
        assert!(app.session.authoritative_tick() >= stop_tick);
        assert!(app.battle_outcome.is_some(), "the terminal result must reach the HUD");

        let frozen_position = app.predictor.position();
        let frozen_client_tick = app.client_tick;
        app.input.fire_pending = true;
        app.input.pending_ammo_select = Some(2);
        app.run_fixed_ticks(60);

        assert_eq!(app.predictor.position(), frozen_position);
        assert_eq!(app.predictor.speed_mps(), 0.0);
        assert_eq!(
            app.client_tick, frozen_client_tick,
            "no post-outcome command may be sequenced or sent"
        );
        assert!(!app.input.fire_pending);
        assert!(app.input.pending_ammo_select.is_none());
    }
}
