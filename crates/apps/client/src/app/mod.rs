mod battle_scars;
mod camera_link;
#[cfg(test)]
mod camera_tests;
#[cfg(test)]
mod fire_fx_tests;
mod garage;
mod garage_render;
#[cfg(test)]
mod hit_mark_tests;
mod input;
mod input_state;
#[cfg(test)]
mod input_tests;
mod lifecycle;
mod loop_step;
mod prediction;
mod render;
#[cfg(test)]
mod render_tests;
mod reticle;
mod vehicle_assets;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Context;
use game_core::{TankId, VehicleKind};
use renderer_wgpu::WindowRenderer;
use server::{LocalAuthoritativeServer, ServerTickConfig};
use sim::DEFAULT_SIMULATION_TICK_HZ;
use terrain::{BattlefieldMap, prokhorovka_hill_252_2};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;

use crate::aim::DesiredAim;
use crate::app::garage::GarageState;
pub use crate::app::garage::garage_overlay;
use crate::fx::FxSystem;
use crate::hit_indicator::HitIndicator;
use crate::predict::LocalPredictor;
use crate::{
    BattleCameraController, CameraObstacle, InterpolatedBattleState, VehicleAssetCatalog,
    WinitLoopDriver,
};

/// Which static scene the renderer currently holds. The garage and the battlefield share one
/// renderer; the active scene's geometry is swapped in on transition (see `render`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SceneKind {
    Garage,
    Battle,
}

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
    /// Ammo slot requested with 1/2/3 this frame, consumed once by the next fixed-tick batch.
    pending_ammo_select: Option<u8>,
    free_look: bool,
    /// Camera pitch captured when free look began, restored on release.
    free_look_return_pitch: Option<f32>,
    /// Fractional wheel motion below one notch, carried between scroll events.
    wheel_pending_lines: f32,
    /// Whether Shift is currently held — the garage uses Shift+click to cycle a module backward.
    shift: bool,
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
    vehicle_asset_catalog: VehicleAssetCatalog,
    /// Persistent render-side ECS projected from the snapshot buffer; the renderer/HUD read from
    /// this rather than rebuilding the scene from `Vec<TankSnapshot>` each frame.
    presentation: engine::PresentationWorld,
    last_render_time: Instant,
    hit_indicator: HitIndicator,
    /// Rolling dealt/taken damage feed for the left-edge battle log.
    damage_log: crate::hud::damage_log::DamageLog,
    /// Incoming hits awaiting their screen-bearing arcs.
    incoming_hits: crate::hud::hit_direction::IncomingHitFeed,
    /// Battle effects (muzzle flash, smoke, dust, impact bursts, tracers): one particle pool
    /// ticked per presented frame and drawn by the renderer's unlit FX pass.
    fx: FxSystem,
    /// Accumulated battle scars per tank (hit decals in hull/turret local frames). Persistent
    /// across snapshots — the snapshot replicates damage STATE, the scars record its history.
    tank_scars: HashMap<game_core::TankId, crate::vehicle::variation::VehicleVariation>,
    /// Craters and scorch marks where shells struck the ground: a budgeted world-space pool
    /// stamped onto the terrain through the same FX pass as the on-tank decals.
    terrain_scars: crate::fx::TerrainScars,
    /// Per-tank emission clock for the dead-engine smoke column (seconds since last puff).
    engine_smoke_accum_s: HashMap<game_core::TankId, f32>,
    /// Smoothed frames-per-second for the HUD readout (EMA over instantaneous frame rate).
    fps_estimate: f32,
    /// Static scene geometry currently uploaded to the renderer (garage hangar vs battlefield).
    current_scene: SceneKind,
    /// Last known framebuffer size, used to map cursor pixels into clip space for the garage UI.
    viewport: (u32, u32),
}

impl ClientApp {
    fn new() -> Self {
        Self::new_with_default_vehicle_artifacts()
    }

    fn new_without_vehicle_artifacts() -> Self {
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
            vehicle_asset_catalog: VehicleAssetCatalog::default(),
            presentation: engine::PresentationWorld::default(),
            last_render_time: Instant::now(),
            hit_indicator: HitIndicator::default(),
            damage_log: crate::hud::damage_log::DamageLog::default(),
            incoming_hits: crate::hud::hit_direction::IncomingHitFeed::default(),
            fx: FxSystem::default(),
            tank_scars: HashMap::new(),
            terrain_scars: crate::fx::TerrainScars::default(),
            engine_smoke_accum_s: HashMap::new(),
            fps_estimate: 0.0,
            // The renderer is created with the battlefield mesh (see `create_renderer`); the first
            // garage frame swaps in the hangar. Starting at `Garage` here would skip that swap.
            current_scene: SceneKind::Battle,
            viewport: (1280, 720),
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

    #[test]
    fn startup_can_preload_forge_artifacts_before_first_vehicle_render() {
        let root = std::env::temp_dir()
            .join(format!("wot_client_startup_forge_artifacts_{}", std::process::id()));
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("remove stale startup artifact root");
        }
        let vehicle_dir = root.join("t54-1951");
        vehicle_forge::ForgeArtifact::bake(
            game_core::VehicleKind::T54_1951,
            vehicle_forge::BakeProfile::Lod0,
        )
        .expect("bake startup artifact")
        .write_to_dir(&vehicle_dir)
        .expect("write startup artifact");

        let app = ClientApp::new_with_vehicle_artifact_root(Some(&root));

        assert_eq!(app.vehicle_asset_catalog.cached_vehicle_count(), 1);
        assert_eq!(app.vehicle_asset_catalog.material_count(), 1);

        std::fs::remove_dir_all(root).expect("remove startup artifact root");
    }
}
