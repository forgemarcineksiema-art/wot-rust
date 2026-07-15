use game_core::TankId;
use net::{Snapshot, TankSnapshot};

use super::ClientApp;

#[test]
fn remote_interpolation_alpha_is_phase_locked_to_the_snapshot_cadence() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();

    // Walk whole snapshot windows: the phase must restart at every ingested snapshot and reach
    // exactly 1 as the next one lands — never freezing short of it, never overshooting.
    let mut saw_reset = false;
    let mut max_alpha_before_reset = 0.0_f32;
    for _ in 0..12 {
        let before = app.remote_interpolation_alpha();
        app.run_fixed_ticks(1);
        let after = app.remote_interpolation_alpha();
        if after < before {
            saw_reset = true;
            max_alpha_before_reset = max_alpha_before_reset.max(before);
        }
        assert!((0.0..=1.0).contains(&after), "phase stays in [0,1], got {after}");
    }
    assert!(saw_reset, "snapshots must restart the phase");
    // With a zero sub-tick remainder the phase right before a snapshot is (window-1)/window.
    let window = app.render_state.snapshot_interval_ticks().expect("two snapshots seen") as f32;
    assert!(
        (max_alpha_before_reset - (window - 1.0) / window).abs() < 1.0e-4,
        "the phase walks the whole window tick by tick, got {max_alpha_before_reset}"
    );
}

#[test]
fn presentation_time_is_phase_locked_to_the_fixed_tick_clock_under_frame_jitter() {
    use std::time::Duration;

    use crate::{ClientLoopAction, ClientLoopEvent};

    // Two frame clocks delivering the identical simulated span: one steady, one jittery. The
    // presentation clock is a function of (fixed ticks run, sub-tick remainder) ONLY — the same
    // doctrine as TankMotion — so both runs must land on exactly the same time, and neither may
    // ever step backwards.
    let steady: Vec<Duration> = vec![Duration::from_millis(20); 30]; // 600 ms
    let jittery: Vec<Duration> = [3u64, 45, 2, 90, 10, 50, 100, 100, 60, 80, 25, 35]
        .into_iter()
        .map(Duration::from_millis)
        .collect(); // also 600 ms, wildly uneven

    let run = |frames: &[Duration]| {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        let mut last = app.presented_time_s();
        for &elapsed in frames {
            for action in app.loop_driver.handle_event(ClientLoopEvent::AboutToWait { elapsed }) {
                if let ClientLoopAction::RunFixedTicks(count) = action {
                    app.run_fixed_ticks(count);
                }
            }
            let now = app.presented_time_s();
            assert!(now >= last, "presentation time stepped backwards: {now} < {last}");
            last = now;
        }
        (app.client_tick, app.presented_time_s())
    };

    let (steady_ticks, steady_time) = run(&steady);
    let (jitter_ticks, jitter_time) = run(&jittery);

    assert_eq!(steady_ticks, jitter_ticks, "equal spans must run equal fixed ticks");
    assert!(
        (steady_time - jitter_time).abs() < 1.0e-6,
        "the frame-clock pattern leaked into the presentation clock: {steady_time} vs {jitter_time}"
    );
    // And the clock really advanced: 600 ms of span at 60 Hz.
    assert!((steady_time - 0.6).abs() < 1.0e-3, "expected ~0.6 s, got {steady_time}");
}

/// The battle scene's terrain/water meshes are baked at most once per app: a garage→battle
/// swap must reuse the cache, never rebake the full 1000 m battlefield inside the transition
/// frame (the iGPU battle-start freeze). Locked by pointer identity — a rebake would allocate
/// fresh Vecs.
#[test]
fn battle_scene_meshes_are_baked_once_and_reused_across_a_scene_swap() {
    let mut app = ClientApp::new();
    app.ensure_battle_scene_meshes();
    let baked = app.battle_scene_meshes.as_ref().expect("baked");
    assert!(!baked.ground_vertices.is_empty(), "the battlefield really produced geometry");
    let terrain_ptr = baked.ground_vertices.as_ptr();
    let water_ptr = baked.water_vertices.as_ptr();

    // A redundant bake is a no-op, and a scene swap toward battle reuses the same allocation.
    app.ensure_battle_scene_meshes();
    app.ensure_scene(super::SceneKind::Garage);
    app.ensure_scene(super::SceneKind::Battle);

    let after = app.battle_scene_meshes.as_ref().expect("still baked");
    assert_eq!(after.ground_vertices.as_ptr(), terrain_ptr, "terrain was rebaked");
    assert_eq!(after.water_vertices.as_ptr(), water_ptr, "water was rebaked");
}

#[test]
fn player_spec_and_reload_follow_snapshot_vehicle() {
    let mut app = ClientApp::new();
    let tank_id = app.player_tank;
    app.accept_and_sync(snapshot_for_vehicle(tank_id, 3, game_core::VehicleKind::TigerII));

    let (_, reload_max) = app.player_reload();

    assert_eq!(app.player_spec().kind, game_core::VehicleKind::TigerII);
    assert_eq!(reload_max, game_core::VehicleKind::TigerII.spec().gun.reload_seconds);
    assert_ne!(reload_max, game_core::TankSpec::t55a().gun.reload_seconds);
}

#[test]
fn startup_garage_blocks_fixed_tick_commands_until_confirmed() {
    let mut app = ClientApp::new();
    app.input.forward = true;

    app.run_fixed_ticks(1);
    assert_eq!(app.client_tick, 0, "startup garage should block driving commands");

    app.select_garage_vehicle(game_core::VehicleKind::TigerI);
    app.confirm_garage_selection();
    app.run_fixed_ticks(1);

    assert_eq!(app.client_tick, 1, "confirmed garage selection should start the drive loop");
    assert_eq!(
        app.player_snapshot().expect("player snapshot").vehicle,
        game_core::VehicleKind::TigerI
    );
}

#[test]
fn runtime_garage_confirm_deploys_the_new_vehicle_with_a_matching_predictor() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();

    app.open_garage();
    app.select_garage_vehicle(game_core::VehicleKind::Jagdtiger);
    app.confirm_garage_selection();

    // A runtime confirm abandons the old battle and deploys fresh, so the deterministic roster
    // may hand back the same TankId — what must change is the vehicle under the player and the
    // predictor spec driving it.
    assert_eq!(
        app.player_snapshot().expect("new player snapshot").vehicle,
        game_core::VehicleKind::Jagdtiger
    );
    assert_eq!(app.predictor_spec().kind, game_core::VehicleKind::Jagdtiger);
}

#[test]
fn local_render_tank_uses_predicted_turret_and_gun_pitch() {
    let mut app = ClientApp::new();
    let tank_id = app.player_tank;
    app.accept_and_sync(snapshot_with_aim(tank_id, 3, 0.0, 0.0));
    app.accept_and_sync(snapshot_with_aim(tank_id, 6, 0.3, 0.2));

    let command = sim::TankCommand {
        turret_yaw_delta: 1.0,
        gun_pitch_delta: 1.0,
        ..sim::TankCommand::idle()
    };
    app.step_prediction(&command);

    let tank = app.local_render_tank().expect("local render tank");
    assert!(
        (tank.turret_yaw_rad - app.predictor.turret_yaw()).abs() < 1.0e-5,
        "local turret yaw should be predicted, got {}",
        tank.turret_yaw_rad
    );
    assert!(
        (tank.gun_pitch_rad - app.predictor.gun_pitch()).abs() < 1.0e-5,
        "local gun pitch should be predicted, got {}",
        tank.gun_pitch_rad
    );
}

#[test]
fn interpolated_local_tank_blends_position_between_prediction_ticks() {
    let mut app = ClientApp::new();
    let tank_id = app.player_tank;
    app.accept_and_sync(snapshot_at(tank_id, 3, [0.0, 0.0, 0.0]));
    app.accept_and_sync(snapshot_at(tank_id, 6, [0.0, 0.0, 0.0]));

    app.step_prediction(&sim::TankCommand::drive(1.0, 0.0));

    let start = app.interpolated_local_tank(0.0).expect("tank at alpha 0").position;
    let mid = app.interpolated_local_tank(0.5).expect("tank at alpha 0.5").position;
    let end = app.interpolated_local_tank(1.0).expect("tank at alpha 1").position;

    assert!((end[2] - app.predictor.position().z).abs() < 1.0e-6);
    assert!(end[2] > start[2], "the hull advanced along +Z over the tick");
    assert!(
        mid[2] > start[2] && mid[2] < end[2],
        "alpha 0.5 must sit strictly between the two ticks ({} vs {}..{})",
        mid[2],
        start[2],
        end[2]
    );
}

#[test]
fn hud_speed_uses_local_prediction_speed_in_kmh() {
    let mut app = ClientApp::new();
    let tank_id = app.player_tank;
    app.accept_and_sync(snapshot_at(tank_id, 3, [0.0, 0.0, 0.0]));
    app.accept_and_sync(snapshot_at(tank_id, 6, [0.0, 0.0, 0.0]));

    app.step_prediction(&sim::TankCommand::drive(1.0, 0.0));

    let speed_kmh = app.player_speed_kmh();
    assert!(speed_kmh > 0.0, "predicted drive tick should produce a HUD speed");
    assert!(speed_kmh < 1.0, "one 60 Hz tick from rest should still be a small km/h value");
}

/// Identity view + an eye at the tanks: everything sits inside the shadow-reach keep gate, so
/// only the sniper rule decides visibility here (the cull itself is tested in `frame_scene`).
const IDENTITY_VIEW: [[f32; 4]; 4] =
    [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

#[test]
fn sniper_mode_hides_the_player_vehicle_but_not_other_tanks() {
    let mut app = ClientApp::new();
    let tank_id = app.player_tank;
    app.accept_and_sync(snapshot_at(tank_id, 3, [5.0, 0.0, 7.0]));
    let tanks = app.project_render_tanks(0.0);
    assert!(tanks.iter().any(|tank| tank.id == tank_id), "baseline must include the player");

    let third_person = app.visible_render_tanks(tanks.clone(), IDENTITY_VIEW, [5.0, 0.0, 7.0]);
    assert!(third_person.iter().any(|tank| tank.id == tank_id));

    app.camera_controller.set_mode(crate::BattleCameraMode::Sniper);
    let sniper = app.visible_render_tanks(tanks.clone(), IDENTITY_VIEW, [5.0, 0.0, 7.0]);
    // The sniper eye sits inside the player's own turret; the hull must not fill the lens.
    assert!(!sniper.iter().any(|tank| tank.id == tank_id));
    assert_eq!(sniper.len(), tanks.len() - 1, "only the player's vehicle is hidden");
}

#[test]
fn render_tanks_are_projected_into_the_persistent_presentation_world() {
    let mut app = ClientApp::new();
    let tank_id = app.player_tank;
    app.accept_and_sync(snapshot_at(tank_id, 3, [5.0, 0.0, 7.0]));

    let projected = app.project_render_tanks(0.0);
    let rendered = app.render_tanks(0.0);

    assert_eq!(projected.len(), rendered.len());
    assert!(!projected.is_empty(), "the seeded player tank should be projected");
    assert!(projected.iter().any(|tank| tank.id == tank_id));
    assert_eq!(app.presentation.tank_count(), rendered.len());

    let reprojected = app.project_render_tanks(0.0);
    assert_eq!(reprojected.len(), rendered.len());
    assert_eq!(app.presentation.tank_count(), rendered.len());
}

#[test]
fn new_app_scene_matches_the_renderer_terrain_so_the_first_garage_frame_swaps_in_the_hangar() {
    // `create_renderer` uploads the battlefield mesh; `current_scene` must agree, or the lazy
    // `ensure_scene` swap would think the hangar is already loaded and never upload it.
    let app = ClientApp::new();
    assert_eq!(app.current_scene, super::SceneKind::Battle);
}

#[test]
#[ignore = "visual preview: writes target/garage_preview.png"]
fn render_garage_preview_png() {
    use renderer_api::{CameraProjectionPolicy, SceneLighting, view_projection_matrix};
    use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
    use std::fs::File;
    use std::io::BufWriter;

    use super::garage::GarageState;
    use crate::{VehicleAssetCatalog, render_frame_from_objects, tank_vehicle_render_objects};
    use scene_build::hangar::{TURNTABLE_TOP_M, hangar_scene_mesh};

    let width = 1280u32;
    let height = 720u32;
    let aspect = width as f32 / height as f32;

    let mut garage = GarageState::default();
    // The T-54 has a swappable gun; cycle to the alternate so the longer barrel shows in the
    // silhouette and the Modules panel arrows are exercised.
    garage.select_vehicle(game_core::VehicleKind::T54_1951);
    garage.cycle_module(super::garage::FitSlot::Gun, 1);
    let kind = garage.selected_vehicle();
    let spec = kind.spec();
    let snapshot = TankSnapshot {
        tank_id: TankId(0),
        team: game_core::TeamId(1),
        vehicle: kind,
        position: [0.0, TURNTABLE_TOP_M, 0.0],
        yaw_rad: 0.6,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: spec.hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 0.0,
        module_hit_points: spec.module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        track_hp: [game_core::TRACK_HP_MAX; 2],
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
        armor_breaches: Default::default(),
        track_break_t: [None, None],
        engine_fire: false,
        fuel_fire: false,
    };

    let (terrain_vertices, terrain_indices) = hangar_scene_mesh();
    let mut catalog = VehicleAssetCatalog::default();
    let mut objects = tank_vehicle_render_objects(&mut catalog, &snapshot, [0.34, 0.42, 0.30]);
    let barrel_scale = garage.gun_silhouette_scale();
    if let Some(gun) = objects.get_mut(2) {
        let scaled = glam::Mat4::from_cols_array_2d(&gun.transform)
            * glam::Mat4::from_scale(glam::Vec3::new(1.0, 1.0, barrel_scale));
        gun.transform = scaled.to_cols_array_2d();
    }
    let render_frame = render_frame_from_objects(objects);

    let camera = garage.orbit_camera();
    let projection = CameraProjectionPolicy::webgpu_default();
    let view_proj = view_projection_matrix(
        &camera,
        aspect,
        projection.near_plane_m(),
        projection.far_plane_m(),
    );

    let ctx = GpuContext::headless().expect("headless gpu");
    let target = OffscreenTarget::new(&ctx, width, height).expect("offscreen target");
    let mut renderer =
        SceneRenderer::for_offscreen(&ctx, &terrain_vertices, &terrain_indices).expect("renderer");
    renderer.scene_lighting = SceneLighting::garage_studio();
    let (font_w, font_h, font_coverage) = crate::hud_font_atlas();
    renderer.set_hud_font_atlas(&ctx, font_w, font_h, font_coverage);
    for (handle, mesh) in catalog.take_pending_vehicle_meshes() {
        renderer.register_vehicle_mesh(&ctx, handle, &mesh);
    }
    for (handle, maps) in catalog.take_pending_vehicle_materials() {
        renderer.register_vehicle_material(&ctx, handle, &maps);
    }
    renderer.set_vehicle_render_frame(&ctx, &render_frame);
    renderer.set_hud(&ctx, &garage.overlay_vertices(aspect));
    renderer.render(&ctx, target.render_target(), view_proj, camera.eye).expect("render");

    let pixels = target.read_rgba8(&ctx).expect("read pixels");
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../target/garage_preview.png");
    let file = File::create(path).expect("create png");
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().expect("header").write_image_data(&pixels).expect("data");
    println!("wrote {path} ({width}x{height})");
}

/// One-tank snapshot with every pose field zeroed; tests override what they exercise.
fn snapshot_for_vehicle(
    tank_id: TankId,
    server_tick: u64,
    vehicle: game_core::VehicleKind,
) -> Snapshot {
    let spec = vehicle.spec();
    Snapshot {
        server_tick,
        tanks: vec![TankSnapshot {
            tank_id,
            team: game_core::TeamId(1),
            vehicle,
            position: [0.0, 0.0, 0.0],
            yaw_rad: 0.0,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: 0.0,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.0,
            hit_points: spec.hit_points,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: spec.gun.dispersion_mrad,
            module_hit_points: spec.module_health.hit_points_by_slot(),
            destroyed_modules_mask: 0,
            track_damage_mask: 0,
            track_hp: [game_core::TRACK_HP_MAX; 2],
            ammo_counts: game_core::AmmoLoadout::default().counts,
            selected_ammo: 0,
            spotted_by_teams_mask: 0,
            armor_breaches: Default::default(),
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
        }],
        shells: Vec::new(),
        damage_events: Vec::new(),
        shell_impacts: Vec::new(),
        detached_turrets: Vec::new(),
        cover_states: Vec::new(),
        craters: Vec::new(),
        cover_scars: Vec::new(),
    }
}

fn snapshot_at(tank_id: TankId, server_tick: u64, position: [f32; 3]) -> Snapshot {
    let mut snapshot =
        snapshot_for_vehicle(tank_id, server_tick, game_core::VehicleKind::PrototypeMedium);
    snapshot.tanks[0].position = position;
    snapshot
}

fn snapshot_with_aim(
    tank_id: TankId,
    server_tick: u64,
    turret_yaw_rad: f32,
    gun_pitch_rad: f32,
) -> Snapshot {
    let mut snapshot = snapshot_at(tank_id, server_tick, [10.0, 0.0, 10.0]);
    snapshot.tanks[0].turret_yaw_rad = turret_yaw_rad;
    snapshot.tanks[0].gun_pitch_rad = gun_pitch_rad;
    snapshot
}

/// F7's contract: a cover collapse rebuilds the statics on a WORKER thread — the render
/// thread only flags, harvests and uploads. The world keeps the pre-collapse mesh until the
/// bake lands (a building settling a beat late is invisible; a 25 ms hitch is not).
#[test]
fn a_cover_collapse_rebuilds_the_statics_off_the_render_thread() {
    let mut app = super::ClientApp::new();
    app.confirm_garage_selection();
    app.ensure_battle_scene_meshes();
    let before = app.battle_scene_meshes.as_ref().expect("baked").statics_vertices.len();

    // Collapse the first cover object and flag the scene dirty (what ingest does).
    assert!(!app.battlefield.static_cover.is_empty(), "the map has cover");
    app.cover_phase_bytes = vec![0u8; app.battlefield.static_cover.len()];
    app.cover_phase_bytes[0] = 1; // rubble
    app.scene_cover_dirty = true;

    // First call SPAWNS the bake and returns immediately — the old mesh still stands.
    app.rebuild_cover_scene_if_dirty();
    assert!(!app.scene_cover_dirty, "the flag is consumed by the spawn");
    assert!(app.scene_rebuild_rx.is_some(), "a worker bake is in flight");
    assert_eq!(
        app.battle_scene_meshes.as_ref().expect("baked").statics_vertices.len(),
        before,
        "the render thread keeps drawing the pre-collapse world while the worker bakes"
    );

    // Poll like the render loop does; the worker's result lands within a few seconds.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.scene_rebuild_rx.is_some() {
        assert!(std::time::Instant::now() < deadline, "the worker bake must complete");
        std::thread::sleep(std::time::Duration::from_millis(10));
        app.rebuild_cover_scene_if_dirty();
    }
    assert_ne!(
        app.battle_scene_meshes.as_ref().expect("baked").statics_vertices.len(),
        before,
        "the collapsed building's rubble replaced the intact mesh"
    );
}
