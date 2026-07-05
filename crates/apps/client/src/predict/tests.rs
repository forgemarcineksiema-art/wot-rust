use net::TankSnapshot;

use super::*;

fn snapshot_at(position: [f32; 3]) -> TankSnapshot {
    snapshot_for_vehicle(game_core::VehicleKind::T55A, position)
}

fn snapshot_for_vehicle(vehicle: game_core::VehicleKind, position: [f32; 3]) -> TankSnapshot {
    let spec = vehicle.spec();
    TankSnapshot {
        tank_id: game_core::TankId(1),
        team: game_core::TeamId(1),
        vehicle,
        position,
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
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
    }
}

fn snapshot_with_aim(turret_yaw_rad: f32, gun_pitch_rad: f32) -> TankSnapshot {
    TankSnapshot {
        turret_yaw_rad,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad,
        ..snapshot_at([10.0, 0.0, 10.0])
    }
}

fn snapshot_with_vehicle_aim(
    vehicle: game_core::VehicleKind,
    turret_yaw_rad: f32,
    gun_pitch_rad: f32,
) -> TankSnapshot {
    TankSnapshot {
        turret_yaw_rad,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad,
        ..snapshot_for_vehicle(vehicle, [10.0, 0.0, 10.0])
    }
}

fn snapshot_with_damage(hit_points: u32, destroyed_modules_mask: u8) -> TankSnapshot {
    TankSnapshot { hit_points, destroyed_modules_mask, ..snapshot_at([10.0, 0.0, 10.0]) }
}

fn snapshot_with_gun_module_hp(gun_hp: u32) -> TankSnapshot {
    let mut snapshot = snapshot_at([10.0, 0.0, 10.0]);
    snapshot.module_hit_points[ModuleSlot::Gun.wire_index()] = gun_hp;
    snapshot
}

#[test]
fn prediction_seeds_then_drives_the_hull_forward() {
    let flat = HeightMap::flat(8, 8, 4.0, 0.0).unwrap();
    let mut predictor = LocalPredictor::new(&TankSpec::t55a());
    assert!(!predictor.is_seeded());

    predictor.sync_to(&snapshot_at([10.0, 0.0, 10.0]));
    assert!(predictor.is_seeded());

    for _ in 0..20 {
        predictor.step(TankCommand::drive(1.0, 0.0), &flat, &[], &[], 1.0 / 60.0);
    }

    // Yaw 0 means forward = +Z, so the predicted hull advances along +Z with no drift. The force
    // model launches at the track-grip limit, so a third of a second covers ~0.3 m.
    let pos = predictor.position();
    assert!(pos.z > 10.2, "predicted z = {}", pos.z);
    assert!((pos.x - 10.0).abs() < 1.0e-3, "predicted x = {}", pos.x);
}

#[test]
fn prediction_tracks_turret_and_gun_pitch_from_local_commands() {
    let flat = HeightMap::flat(8, 8, 4.0, 0.0).unwrap();
    let spec = TankSpec::t55a();
    let mut predictor = LocalPredictor::new(&spec);
    predictor.sync_to(&snapshot_with_aim(0.10, -0.02));

    let dt = 1.0 / 60.0;
    predictor.step(
        TankCommand { turret_yaw_delta: 1.0, gun_pitch_delta: 1.0, ..TankCommand::idle() },
        &flat,
        &[],
        &[],
        dt,
    );

    let full_rate_step = spec.turret_rotation_rad_s * dt;
    let expected_turret = 0.10 + full_rate_step;
    let expected_pitch = -0.02 + sim::GUN_ELEVATION_RATE_RAD_S * dt;
    assert!(
        (predictor.turret_yaw() - expected_turret).abs() < 1.0e-5,
        "predicted turret yaw = {}",
        predictor.turret_yaw()
    );
    assert!(
        (predictor.gun_pitch() - expected_pitch).abs() < 1.0e-5,
        "predicted gun pitch = {}",
        predictor.gun_pitch()
    );
}

#[test]
fn prediction_zeros_fixed_casemate_yaw_from_snapshots_and_commands() {
    let flat = HeightMap::flat(8, 8, 4.0, 0.0).unwrap();
    let spec = TankSpec::jagdtiger();
    let mut predictor = LocalPredictor::new(&spec);
    predictor.sync_to(&snapshot_with_vehicle_aim(game_core::VehicleKind::Jagdtiger, 0.35, -0.02));

    predictor.step(
        TankCommand { turret_yaw_delta: 1.0, gun_pitch_delta: 1.0, ..TankCommand::idle() },
        &flat,
        &[],
        &[],
        1.0 / 60.0,
    );

    assert_eq!(predictor.turret_yaw(), 0.0);
    assert!(predictor.gun_pitch() > -0.02);
}

#[test]
fn prediction_respects_authoritative_module_and_hull_damage() {
    let flat = HeightMap::flat(8, 8, 4.0, 0.0).unwrap();
    let spec = TankSpec::t55a();
    let mut predictor = LocalPredictor::new(&spec);
    let engine_destroyed = ModuleSlot::Engine.destroyed_mask_bit();
    let suspension_destroyed = ModuleSlot::Suspension.destroyed_mask_bit();
    let turret_destroyed = ModuleSlot::Turret.destroyed_mask_bit();
    predictor.sync_to(&snapshot_with_damage(
        0,
        engine_destroyed | suspension_destroyed | turret_destroyed,
    ));
    let before_position = predictor.position();
    let before_turret = predictor.turret_yaw();
    let before_pitch = predictor.gun_pitch();

    predictor.step(
        TankCommand {
            throttle: 1.0,
            steer: 1.0,
            turret_yaw_delta: 1.0,
            gun_pitch_delta: 1.0,
            ..TankCommand::idle()
        },
        &flat,
        &[],
        &[],
        1.0 / 60.0,
    );

    assert_eq!(predictor.position(), before_position);
    assert_eq!(predictor.turret_yaw(), before_turret);
    assert_eq!(predictor.gun_pitch(), before_pitch);
}

#[test]
fn prediction_blooms_dispersion_on_traverse_and_recovers_when_still() {
    let flat = HeightMap::flat(8, 8, 4.0, 0.0).unwrap();
    let mut predictor = LocalPredictor::new(&TankSpec::t55a());
    predictor.sync_to(&snapshot_at([10.0, 0.0, 10.0]));
    let settled = predictor.aim_dispersion_mrad();

    // Traversing the turret blooms the circle above the settled minimum.
    for _ in 0..10 {
        predictor.step(
            TankCommand { turret_yaw_delta: 1.0, ..TankCommand::idle() },
            &flat,
            &[],
            &[],
            1.0 / 60.0,
        );
    }
    let bloomed = predictor.aim_dispersion_mrad();
    assert!(
        bloomed > settled + 1.0e-4,
        "traverse should bloom dispersion ({settled} -> {bloomed})"
    );

    // Sitting still recovers it back toward the minimum.
    for _ in 0..120 {
        predictor.step(TankCommand::idle(), &flat, &[], &[], 1.0 / 60.0);
    }
    let recovered = predictor.aim_dispersion_mrad();
    assert!(recovered < bloomed, "stillness should recover dispersion ({bloomed} -> {recovered})");
}

#[test]
fn prediction_recovers_dispersion_against_partial_gun_module_damage() {
    let flat = HeightMap::flat(8, 8, 4.0, 0.0).unwrap();
    let spec = TankSpec::t55a();
    let mut predictor = LocalPredictor::new(&spec);
    predictor
        .sync_to(&snapshot_with_gun_module_hp(spec.module_health.hit_points(ModuleSlot::Gun) / 2));

    for _ in 0..180 {
        predictor.step(TankCommand::idle(), &flat, &[], &[], 1.0 / 60.0);
    }

    let healthy_minimum = spec.gun.dispersion_mrad;
    assert!(
        predictor.aim_dispersion_mrad() > healthy_minimum * 1.5,
        "partial gun damage should raise the recovered aim floor; got {} vs healthy {}",
        predictor.aim_dispersion_mrad(),
        healthy_minimum
    );
}
