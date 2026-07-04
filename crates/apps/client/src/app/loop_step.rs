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
        // Mouse look is applied per rendered frame (see `render_now`); the fixed step only
        // consumes the resulting aim, so the turret converges on the latest sight point. One
        // sight sweep feeds both the traverse and the elevation command.
        let solution = self.sight_solution();
        let turret_yaw_delta = self.turret_tracking_command_for(solution.as_ref());
        let gun_pitch_delta = self.gun_elevation_command_for(solution.as_ref());
        let mut fire = self.input.fire_pending;
        self.input.fire_pending = false;
        self.seed_prediction();
        for _ in 0..count {
            let command = TankCommand {
                throttle: self.input.throttle(),
                steer: -self.input.steer(),
                brake: self.input.brake_value(),
                turret_yaw_delta,
                gun_pitch_delta,
                fire,
            };
            fire = false;
            self.step_prediction(&command);
            let outcome = self.local_server.tick_with_input(ClientInputCommand {
                client_tick: self.client_tick,
                tank_id: self.player_tank,
                command,
            });
            self.client_tick += 1;
            if let Some(snapshot) = outcome.snapshot {
                self.accept_and_sync(snapshot);
            }
        }
    }
}
