mod audio_link;
mod battle_scars;
mod camera_link;
#[cfg(test)]
mod camera_tests;
#[cfg(test)]
mod fire_fx_tests;
mod frame_scene;
mod garage;
mod garage_render;
#[cfg(test)]
mod hit_mark_tests;
mod ingest;
mod input;
mod input_state;
#[cfg(test)]
mod input_tests;
mod lifecycle;
mod loop_step;
mod minimap_build;
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
use server::{LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};
use sim::DEFAULT_SIMULATION_TICK_HZ;
use terrain::BattlefieldMap;
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

/// The battle scene's baked CPU meshes — see `ClientApp::battle_scene_meshes`.
pub(crate) struct BattleSceneMeshes {
    pub(crate) terrain_vertices: Vec<renderer_api::SceneVertex>,
    pub(crate) terrain_indices: Vec<u32>,
    pub(crate) water_vertices: Vec<renderer_api::WaterVertex>,
    pub(crate) water_indices: Vec<u32>,
}

impl ClientApp {
    /// Bake the battle scene's meshes if they are not cached yet. Idempotent; the heavy CPU
    /// work (the full 1000 m terrain + cover + backdrop bake) runs at most once per app.
    pub(crate) fn ensure_battle_scene_meshes(&mut self) {
        if self.battle_scene_meshes.is_some() {
            return;
        }
        let (terrain_vertices, terrain_indices) = crate::battlefield_scene_mesh(&self.battlefield);
        let (water_vertices, water_indices) =
            crate::scene::water::battlefield_water_mesh(&self.battlefield);
        self.battle_scene_meshes = Some(BattleSceneMeshes {
            terrain_vertices,
            terrain_indices,
            water_vertices,
            water_indices,
        });
    }
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
    /// Fixed ticks run since the last ingested snapshot. Together with the sub-tick remainder it
    /// is the remote interpolation phase — the same clock the snapshots are produced on, so the
    /// remote blend can neither freeze at 1.0 nor jump (which wall-clock integration did).
    ticks_since_snapshot: u32,
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
    /// Flying-turret animation per decapitated wreck (ammo-rack detonation, protocol v20). Started
    /// when a tank first appears in `Snapshot.detached_turrets`; the turret and gun render objects
    /// of that wreck are then driven from this deterministic arc instead of the snapshot pose.
    turret_popoffs: HashMap<game_core::TankId, crate::vehicle::turret_popoff::TurretPopoff>,
    /// Per-instance dented hull mesh for each wreck, built once from its recorded penetrations.
    /// The wreck's hull render object is swapped to this handle so a knocked-out tank reads beaten
    /// and dented, not pristine-but-tinted. Presentation only (see `vehicle::wreck_deform`).
    wreck_hull_meshes: HashMap<game_core::TankId, renderer_api::MeshHandle>,
    /// Last-seen static-cover phase bytes (protocol v21). When a snapshot's cover states differ,
    /// the battle scene is rebuilt (collapsed buildings become rubble, cleared foliage vanishes)
    /// and re-uploaded, and a dust burst fires at each freshly-destroyed object.
    cover_phase_bytes: Vec<u8>,
    /// Set when `cover_phase_bytes` changed: the next frame rebuilds and re-uploads the scene.
    scene_cover_dirty: bool,
    /// Smoothed frames-per-second for the HUD readout (EMA over instantaneous frame rate).
    fps_estimate: f32,
    /// The minimap's static layers (terrain relief + cover boxes), computed once per
    /// battlefield instead of resampled every frame. Rebuild alongside `battlefield` if a
    /// future map rotation swaps it mid-session.
    minimap_relief: Vec<f32>,
    minimap_cover: Vec<crate::hud::minimap::MinimapBox>,
    /// Local battle result banner state, derived from the authoritative server outcome.
    battle_outcome: Option<crate::hud::BattleHudOutcome>,
    /// Seconds since the player's latest kill, driving the reticle confirmation; `None` when the
    /// confirmation has played out (see `hud/kill_marker.rs`).
    kill_confirm_age_s: Option<f32>,
    /// Reload seconds remaining at the previous presented frame, for the ready-flash crossing.
    prev_reload_remaining_s: f32,
    /// Seconds since the reload finished, driving the gun-ready flash at the reticle.
    reload_ready_age_s: Option<f32>,
    /// Static scene geometry currently uploaded to the renderer (garage hangar vs battlefield).
    current_scene: SceneKind,
    /// The battle scene's CPU meshes (terrain+cover+backdrop, water), baked lazily ONCE — the
    /// battlefield never changes within a `ClientApp`. Rebaking them synchronously inside the
    /// first battle frame of every garage→battle swap froze that frame for hundreds of
    /// milliseconds on integrated-GPU laptops, and the accumulator then dumped a burst of
    /// catch-up ticks into the next one. A few MB of CPU residency buys a swap that costs only
    /// the GPU upload.
    battle_scene_meshes: Option<BattleSceneMeshes>,
    /// Last known framebuffer size, used to map cursor pixels into clip space for the garage UI.
    viewport: (u32, u32),
    /// Camera mode at the previous presented frame; a change clicks the optics cue.
    prev_camera_mode: Option<crate::BattleCameraMode>,
    /// The platform audio stream; `None` headless or without an output device (silent game).
    audio: Option<crate::audio_out::AudioOutput>,
    /// Sounds produced since the last presented frame, flushed to the device with the frame —
    /// the audio twin of the FX queue (see `app/audio_link.rs`).
    pending_audio: Vec<audio::AudioEvent>,
}

impl ClientApp {
    fn new() -> Self {
        Self::new_with_default_vehicle_artifacts()
    }

    fn new_without_vehicle_artifacts() -> Self {
        Self::from_battle_config(RandomBattleConfig::runtime_from_env(VehicleKind::default()))
    }

    /// A deterministic app for tests that drive real battle ticks: a runtime-seeded battle
    /// makes such tests flaky by construction (an unlucky roster can reach the player inside
    /// the test window and perturb whatever is being asserted).
    #[cfg(test)]
    pub(crate) fn new_seeded(seed: u64) -> Self {
        Self::from_battle_config(RandomBattleConfig::new(
            server::BattleSeed::fixed(seed),
            VehicleKind::default(),
        ))
    }

    fn from_battle_config(config: RandomBattleConfig) -> Self {
        let local_server =
            LocalAuthoritativeServer::new_random_7v7(ServerTickConfig::default(), config);
        let player_tank = local_server.player_tank();
        let mut render_state = InterpolatedBattleState::default();
        render_state.accept_authoritative_snapshot(local_server.latest_snapshot_for_player());
        let player_spec = render_state
            .latest_snapshot()
            .and_then(|snapshot| snapshot.tanks.iter().find(|tank| tank.tank_id == player_tank))
            .map_or_else(|| VehicleKind::default().spec(), |tank| tank.vehicle.spec());
        // The authoritative server names the map; the client regenerates the identical
        // battlefield locally (the world never crosses the wire — see `terrain::MapId`).
        let battlefield = local_server.map_id().battlefield();
        let camera_obstacles =
            battlefield.static_cover.iter().map(CameraObstacle::from_static_cover).collect();
        let mut predictor = LocalPredictor::new(&player_spec);
        predictor.set_water(battlefield.water);
        let (minimap_relief, minimap_cover) =
            crate::app::minimap_build::minimap_static_layers(&battlefield);
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
            ticks_since_snapshot: 0,
            input: InputState::default(),
            predictor,
            vehicle_asset_catalog: VehicleAssetCatalog::default(),
            presentation: engine::PresentationWorld::default(),
            last_render_time: Instant::now(),
            hit_indicator: HitIndicator::default(),
            damage_log: crate::hud::damage_log::DamageLog::default(),
            incoming_hits: crate::hud::hit_direction::IncomingHitFeed::default(),
            fx: FxSystem::default(),
            tank_scars: HashMap::new(),
            turret_popoffs: HashMap::new(),
            terrain_scars: crate::fx::TerrainScars::default(),
            engine_smoke_accum_s: HashMap::new(),
            wreck_hull_meshes: HashMap::new(),
            cover_phase_bytes: Vec::new(),
            scene_cover_dirty: false,
            fps_estimate: 0.0,
            minimap_relief,
            minimap_cover,
            battle_outcome: None,
            kill_confirm_age_s: None,
            prev_reload_remaining_s: 0.0,
            reload_ready_age_s: None,
            // The renderer is created with the battlefield mesh (see `create_renderer`); the first
            // garage frame swaps in the hangar. Starting at `Garage` here would skip that swap.
            current_scene: SceneKind::Battle,
            battle_scene_meshes: None,
            viewport: (1280, 720),
            prev_camera_mode: None,
            audio: None,
            pending_audio: Vec::new(),
        }
    }
}

/// Run the desktop client with winit, the local server, and the real wgpu renderer.
pub fn run() -> anyhow::Result<()> {
    let event_loop = EventLoop::new().context("failed to create winit event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ClientApp::new();
    app.enable_garage_persistence();
    app.prebake_playable_vehicle_assets();
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
    fn new_app_accepts_the_player_filtered_server_snapshot() {
        let app = ClientApp::new();
        let client_view = app.render_state.latest_snapshot().expect("initial client snapshot");
        let server_view = app.local_server.latest_snapshot_for_player();

        assert_eq!(client_view, &server_view);
        assert!(client_view.tanks.iter().any(|tank| tank.tank_id == app.player_tank));
    }

    #[test]
    fn new_app_uses_random_7v7_local_battle() {
        let app = ClientApp::new();
        let full_snapshot = app.local_server.latest_snapshot();

        assert_eq!(full_snapshot.tanks.len(), 14);
        assert_eq!(
            full_snapshot.tanks.iter().filter(|tank| tank.team == game_core::TeamId(1)).count(),
            7
        );
        assert_eq!(
            full_snapshot.tanks.iter().filter(|tank| tank.team == game_core::TeamId(2)).count(),
            7
        );
        assert!(app.render_state.latest_snapshot().is_some_and(|snapshot| {
            snapshot.tanks.iter().any(|tank| tank.tank_id == app.player_tank)
        }));
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
