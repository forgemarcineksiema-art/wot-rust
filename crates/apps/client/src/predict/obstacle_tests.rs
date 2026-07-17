use net::TankSnapshot;

use super::*;

fn snapshot_at(position: [f32; 3]) -> TankSnapshot {
    let spec = game_core::VehicleKind::T54_1951.spec();
    TankSnapshot {
        tank_id: game_core::TankId(1),
        team: game_core::TeamId(1),
        vehicle: game_core::VehicleKind::T54_1951,
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
        track_hp: [game_core::TRACK_HP_MAX; 2],
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
        armor_breaches: Default::default(),
        track_break_t: [None, None],
        engine_fire: false,
        fuel_fire: false,
    }
}

#[test]
fn prediction_is_blocked_by_static_cover_like_the_server() {
    use terrain::StaticCoverKind;

    let flat = HeightMap::flat(64, 64, 4.0, 0.0).unwrap();
    let cover = vec![StaticCoverObject {
        id: "barn".to_string(),
        name: "barn".to_string(),
        kind: StaticCoverKind::FarmBuilding,
        center: [10.0, 1.5, 30.0],
        half_extents_m: [5.0, 2.5, 4.0],
    }];
    let mut predictor = LocalPredictor::new(&TankSpec::t54_1951());
    predictor.sync_to(&snapshot_at([10.0, 0.0, 10.0]));

    let drive = TankCommand::drive(1.0, 0.0); // straight north into the barn
    for _ in 0..240 {
        predictor.step(drive, &flat, &cover, &[], 1.0 / 60.0);
    }

    // The predictor shares the server's movement-and-cover step (`step_tank_on_world`), so it
    // stops where the server does -- the barn's near face -- keeping lockstep instead of
    // driving through cover and snapping back on the next snapshot.
    let pos = predictor.position();
    assert!(pos.z < 25.0, "cover should stop the predicted hull short of the barn (z = {})", pos.z);
    assert!(pos.z > 20.0, "the hull should still advance up to the barn (z = {})", pos.z);
}

#[test]
fn prediction_is_blocked_by_other_tanks_like_the_server() {
    let flat = HeightMap::flat(64, 64, 4.0, 0.0).unwrap();
    let spec = TankSpec::t54_1951();
    let obstacles =
        [physics::TankObstacle::from_hitbox(Vec3::new(10.0, 0.0, 18.0), 0.0, spec.hitbox)];
    let mut predictor = LocalPredictor::new(&spec);
    predictor.sync_to(&snapshot_at([10.0, 0.0, 10.0]));

    let drive = TankCommand::drive(1.0, 0.0);
    for _ in 0..240 {
        predictor.step(drive, &flat, &[], &obstacles, 1.0 / 60.0);
    }

    // The hulls meet nose-to-tail: the obstacle's near face minus our own half length.
    let contact_z = 18.0 - 2.0 * spec.hitbox.half_length_m;
    let pos = predictor.position();
    assert!(
        pos.z <= contact_z + 0.05,
        "tank obstacle should stop the predicted hull (z = {}, contact at {contact_z})",
        pos.z
    );
    assert!(pos.z > contact_z - 0.5, "the hull should still advance to contact (z = {})", pos.z);
}
