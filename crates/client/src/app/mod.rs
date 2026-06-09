#[cfg(test)]
mod camera_tests;
mod garage;
#[cfg(test)]
mod hit_mark_tests;
mod input;
mod lifecycle;
mod prediction;
mod render;
#[cfg(test)]
mod render_tests;
mod reticle;

use std::sync::Arc;
use std::time::Instant;

use anyhow::Context;
use game_core::{TankId, VehicleKind};
use net::ClientInputCommand;
use renderer_wgpu::WindowRenderer;
use server::{LocalAuthoritativeServer, ServerTickConfig};
use sim::{DEFAULT_SIMULATION_TICK_HZ, TankCommand};
use terrain::{BattlefieldMap, prokhorovka_hill_252_2};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::Window;

use crate::aim::DesiredAim;
use crate::app::garage::GarageState;
use crate::hit_indicator::HitIndicator;
use crate::predict::LocalPredictor;
use crate::{
    BattleCameraController, CameraObstacle, ClientLoopAction, InterpolatedBattleState,
    VehicleMeshCatalog, WinitLoopDriver,
};

#[derive(Default)]
pub(crate) struct InputState {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    brake: bool,
    mouse_dx: f32,
    mouse_dy: f32,
    fire_pending: bool,
    free_look: bool,
}

pub(crate) struct ClientApp {
    window: Option<Arc<Window>>,
    renderer: Option<WindowRenderer>,
    loop_driver: WinitLoopDriver,
    last_loop_time: Instant,
    local_server: LocalAuthoritativeServer,
    render_state: InterpolatedBattleState,
    camera_controller: BattleCameraController,
    camera_obstacles: Vec<CameraObstacle>,
    desired_aim: DesiredAim,
    garage: GarageState,
    battlefield: BattlefieldMap,
    player_tank: TankId,
    client_tick: u64,
    input: InputState,
    predictor: LocalPredictor,
    vehicle_mesh_catalog: VehicleMeshCatalog,
    /// Persistent render-side ECS projected from the snapshot buffer; the renderer/HUD read from
    /// this rather than rebuilding the scene from `Vec<TankSnapshot>` each frame.
    presentation: engine::PresentationWorld,
    last_render_time: Instant,
    hit_indicator: HitIndicator,
    /// Smoothed frames-per-second for the HUD readout (EMA over instantaneous frame rate).
    fps_estimate: f32,
}

impl ClientApp {
    fn new() -> Self {
        let local_server = LocalAuthoritativeServer::new(ServerTickConfig::default());
        let player_tank = local_server.player_tank();
        let mut render_state = InterpolatedBattleState::default();
        render_state.accept_authoritative_snapshot(local_server.latest_snapshot());
        let player_spec = render_state
            .latest_snapshot()
            .and_then(|snapshot| snapshot.tanks.iter().find(|tank| tank.tank_id == player_tank))
            .map_or_else(|| VehicleKind::default().spec(), |tank| tank.vehicle.spec());
        let battlefield = prokhorovka_hill_252_2();
        let camera_obstacles =
            battlefield.static_cover.iter().map(CameraObstacle::from_static_cover).collect();
        Self {
            window: None,
            renderer: None,
            loop_driver: WinitLoopDriver::new(DEFAULT_SIMULATION_TICK_HZ),
            last_loop_time: Instant::now(),
            local_server,
            render_state,
            camera_controller: BattleCameraController::default(),
            camera_obstacles,
            desired_aim: DesiredAim::default(),
            garage: GarageState::default(),
            battlefield,
            player_tank,
            client_tick: 0,
            input: InputState::default(),
            predictor: LocalPredictor::new(&player_spec),
            vehicle_mesh_catalog: VehicleMeshCatalog::default(),
            presentation: engine::PresentationWorld::default(),
            last_render_time: Instant::now(),
            hit_indicator: HitIndicator::default(),
            fps_estimate: 0.0,
        }
    }

    fn handle_actions(&mut self, event_loop: &ActiveEventLoop, actions: Vec<ClientLoopAction>) {
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
                    if let Some(renderer) = &mut self.renderer {
                        renderer.resize(width, height);
                    }
                }
                ClientLoopAction::Exit => event_loop.exit(),
            }
        }
    }

    fn run_fixed_ticks(&mut self, count: u32) {
        if !self.garage.has_started() {
            self.input.fire_pending = false;
            return;
        }
        // Mouse look is applied per rendered frame (see `render_now`); the fixed step only
        // consumes the resulting aim, so the turret converges on the latest sight point.
        let turret_yaw_delta = self.turret_tracking_command();
        let gun_pitch_delta = self.gun_elevation_command();
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

/// Run the desktop client with winit, the local server, and the real wgpu renderer.
pub fn run() -> anyhow::Result<()> {
    let event_loop = EventLoop::new().context("failed to create winit event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ClientApp::new();
    event_loop.run_app(&mut app).context("winit app failed")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_ticks_do_not_consume_mouse_look_so_it_stays_per_frame() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        // First step seeds prediction and anchors the camera orbit to the hull facing.
        app.run_fixed_ticks(1);
        let before = app.camera_controller.orbit_yaw_rad();

        app.input.mouse_dx = 120.0;
        app.run_fixed_ticks(1);

        // The fixed step must leave the accumulated mouse delta untouched: look is applied
        // once per rendered frame in `render_now`, decoupled from the 60 Hz tick cadence.
        assert_eq!(app.input.mouse_dx, 120.0, "fixed ticks must not consume mouse look");
        assert!((app.camera_controller.orbit_yaw_rad() - before).abs() < 1.0e-9);

        // Applying look (as render does) consumes the delta and rotates the camera.
        app.apply_mouse_look();
        assert!((app.camera_controller.orbit_yaw_rad() - before).abs() > 1.0e-4);
        assert_eq!(app.input.mouse_dx, 0.0);
    }
}
