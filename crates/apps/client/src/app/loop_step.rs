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
        self.seed_prediction();
        for _ in 0..count {
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
            let outcome = self.local_server.tick_with_player_input(ClientInputCommand {
                client_tick: self.client_tick,
                tank_id: self.player_tank,
                command,
            });
            self.client_tick += 1;
            self.ticks_since_snapshot = self.ticks_since_snapshot.saturating_add(1);
            if let Some(snapshot) = outcome.snapshot {
                self.accept_and_sync(snapshot);
            }
            self.refresh_battle_outcome();
        }
    }

    fn refresh_battle_outcome(&mut self) {
        self.battle_outcome =
            self.local_server.battle_outcome().map(|outcome| match outcome.winning_team() {
                Some(team) if team == self.player_team() => crate::hud::BattleHudOutcome::Victory,
                Some(_) => crate::hud::BattleHudOutcome::Defeat,
                None => crate::hud::BattleHudOutcome::Draw,
            });
    }
}
