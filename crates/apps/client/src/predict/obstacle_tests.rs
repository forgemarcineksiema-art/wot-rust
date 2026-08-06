use net::TankSnapshot;
use physics::ContactBody;

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
        rack_fire_remaining_s: None,
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
        predictor.step(drive, &flat, &cover, &[], &[], None, 1.0 / 60.0);
    }

    // The predictor shares the server's movement-and-cover step (`step_tank_on_world`), so it
    // stops where the server does -- the barn's near face -- keeping lockstep instead of
    // driving through cover and snapping back on the next snapshot.
    let pos = predictor.position();
    assert!(pos.z < 25.0, "cover should stop the predicted hull short of the barn (z = {})", pos.z);
    assert!(pos.z > 20.0, "the hull should still advance up to the barn (z = {})", pos.z);
}

/// THE NAME THIS TEST USED TO CARRY WAS A LIE, and the lie is worth recording because it is the
/// shape of the whole P1.5 defect.
///
/// It was called `prediction_is_blocked_by_other_tanks_like_the_server`, and it locked the
/// predictor stopping the instant two collision boxes touched — a hard "hold the previous
/// position" veto. The server stopped doing that the day contact started carrying momentum: it
/// solves an impulse instead, and until P1.2 it stopped a whole 0.12 m skin earlier than the
/// client did. Two models, one name, and a test that swore they agreed.
///
/// What the predictor does now is what the authority does: decide a velocity, exchange contacts
/// against it, spend what survived. Neighbours are immovable here because the client is not
/// authoritative over them.
#[test]
fn the_predictor_meets_another_hull_the_way_the_server_does() {
    let flat = HeightMap::flat(64, 64, 4.0, 0.0).unwrap();
    let spec = TankSpec::t54_1951();
    let neighbour = ContactBody {
        id: 2,
        position: Vec3::new(10.0, 0.0, 18.0),
        velocity: Vec3::ZERO,
        yaw_rad: 0.0,
        yaw_rate_rad_s: 0.0,
        footprint: physics::TankFootprint::from_hitbox(spec.hitbox),
        mass_kg: spec.mass_kg,
        movable: false,
    };
    let mut predictor = LocalPredictor::new(&spec);
    predictor.sync_to(&snapshot_at([10.0, 0.0, 10.0]));

    let drive = TankCommand::drive(1.0, 0.0);
    for _ in 0..240 {
        predictor.step(drive, &flat, &[], &[neighbour], &[], None, 1.0 / 60.0);
    }

    // Hulls MEET: the boxes come to rest touching, not a detection margin short of each other.
    let touching = 18.0 - 2.0 * spec.hitbox.half_length_m;
    let gap = touching - predictor.position().z;
    assert!(
        gap.abs() <= 0.03,
        "the predicted hull settled {gap:.4} m from contact (touching at z = {touching})"
    );
    // ...and it does not walk through the neighbour it is leaning on.
    assert!(predictor.position().z < 18.0);
}

/// The lock the program asked for: predictor and authority agree while pressed together, tick after
/// tick, rather than agreeing on average and arguing every frame.
#[test]
fn predictor_and_server_rest_in_the_same_place() {
    let flat = HeightMap::flat(64, 64, 4.0, 0.0).unwrap();
    let spec = TankSpec::t54_1951();

    // The authority: two hulls, the far one a wreck so it blocks without giving ground — the same
    // thing the predictor's immovable neighbour represents.
    let mut server = sim::SimulationState::new();
    let mover = server.spawn_tank(game_core::TeamId(1), spec.clone(), Vec3::new(10.0, 0.0, 10.0));
    let blocker = server.spawn_tank(game_core::TeamId(2), spec.clone(), Vec3::new(10.0, 0.0, 18.0));
    server.tank_mut(blocker).expect("blocker").hit_points = 0;

    let neighbour = ContactBody {
        id: 2,
        position: Vec3::new(10.0, 0.0, 18.0),
        velocity: Vec3::ZERO,
        yaw_rad: 0.0,
        yaw_rate_rad_s: 0.0,
        footprint: physics::TankFootprint::from_hitbox(spec.hitbox),
        mass_kg: spec.mass_kg,
        movable: false,
    };
    let mut predictor = LocalPredictor::new(&spec);
    predictor.sync_to(&snapshot_at([10.0, 0.0, 10.0]));

    let drive = TankCommand::drive(1.0, 0.0);
    let step = sim::FixedTimestep::from_hz(60);
    let mut worst = 0.0_f32;
    for tick in 0..300 {
        server.apply_commands(&[(mover, drive)], step);
        predictor.step(drive, &flat, &[], &[neighbour], &[], None, 1.0 / 60.0);
        // The approach itself is a race the two can legitimately run a hair apart; what must not
        // drift is where they END UP and how they behave once they are leaning on the thing.
        if tick > 200 {
            let authority = server.tank(mover).expect("mover").position.z;
            worst = worst.max((predictor.position().z - authority).abs());
        }
    }
    assert!(
        worst <= 0.001,
        "predictor and authority rest {worst:.5} m apart while pressed against the same hull"
    );
}
