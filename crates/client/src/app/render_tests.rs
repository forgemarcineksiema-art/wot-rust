use game_core::TankId;
use net::{Snapshot, TankSnapshot};

use super::ClientApp;

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
fn runtime_garage_confirm_changes_player_tank_id_and_predictor_spec() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    let old_player = app.player_tank;

    app.open_garage();
    app.select_garage_vehicle(game_core::VehicleKind::Jagdtiger);
    app.confirm_garage_selection();

    assert_ne!(app.player_tank, old_player);
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
            turret_yaw_rad: 0.0,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.0,
            hit_points: spec.hit_points,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: spec.gun.dispersion_mrad,
            module_hit_points: spec.module_health.hit_points_by_slot(),
            destroyed_modules_mask: 0,
        }],
        shells: Vec::new(),
        damage_events: Vec::new(),
        shell_impacts: Vec::new(),
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
