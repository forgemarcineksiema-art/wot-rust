//! Render interpolation and authoritative-lockstep tests for the local predictor: the sub-tick
//! blend that keeps a 60 Hz prediction smooth under a faster present clock, and the guarantee
//! that prediction does not drift from the server (which would snap the hull back -- "cofa").
use super::*;

fn snapshot_at(position: [f32; 3]) -> TankSnapshot {
    TankSnapshot {
        tank_id: game_core::TankId(1),
        team: game_core::TeamId(1),
        vehicle: game_core::VehicleKind::PrototypeMedium,
        position,
        yaw_rad: 0.0,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: 1000,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 2.5,
        module_hit_points: game_core::VehicleKind::PrototypeMedium
            .spec()
            .module_health
            .hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
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

#[test]
fn interpolated_pose_blends_previous_tick_toward_current() {
    let flat = HeightMap::flat(8, 8, 4.0, 0.0).unwrap();
    let mut predictor = LocalPredictor::new(&TankSpec::t55a());
    predictor.sync_to(&snapshot_at([10.0, 0.0, 10.0]));

    // One tick of forward drive establishes a non-trivial previous -> current gap.
    predictor.step(TankCommand::drive(1.0, 0.0), &flat, &[], &[], 1.0 / 60.0);

    let start = predictor.interpolated_pose(0.0);
    let end = predictor.interpolated_pose(1.0);
    let mid = predictor.interpolated_pose(0.5);

    // alpha = 1.0 lands exactly on the current simulated tick.
    assert!((end.position.z - predictor.position().z).abs() < 1.0e-6);
    // alpha = 0.0 sits on the previous tick (the seed pose, z = 10.0).
    assert!((start.position.z - 10.0).abs() < 1.0e-6, "start z = {}", start.position.z);
    // The hull moved forward, and the midpoint is strictly between the two ticks.
    assert!(end.position.z > start.position.z, "expected forward motion");
    assert!(
        mid.position.z > start.position.z && mid.position.z < end.position.z,
        "midpoint {} should sit between {} and {}",
        mid.position.z,
        start.position.z,
        end.position.z
    );
    let expected_mid = 0.5 * (start.position.z + end.position.z);
    assert!((mid.position.z - expected_mid).abs() < 1.0e-5);
}

#[test]
fn seeding_anchors_previous_pose_so_the_first_frame_does_not_fly_in() {
    let mut predictor = LocalPredictor::new(&TankSpec::t55a());
    predictor.sync_to(&snapshot_at([10.0, 0.0, 10.0]));

    // Before any tick runs, every sub-tick blend must already sit on the seed pose -- a
    // non-anchored previous would lerp the hull in from the world origin on frame one.
    for alpha in [0.0, 0.5, 1.0] {
        let pose = predictor.interpolated_pose(alpha);
        assert!((pose.position - Vec3::new(10.0, 0.0, 10.0)).length() < 1.0e-6);
    }
}

#[test]
fn interpolated_turret_takes_the_shortest_arc_across_the_pi_wrap() {
    let flat = HeightMap::flat(8, 8, 4.0, 0.0).unwrap();
    let spec = TankSpec::t55a();
    let mut predictor = LocalPredictor::new(&spec);
    // Previous turret just below +PI, current just past -PI after a wrap: a naive lerp would
    // sweep the long way around; shortest-arc must stay near the +/-PI seam.
    predictor.sync_to(&snapshot_with_aim(3.10, 0.0));
    predictor.step(TankCommand::idle(), &flat, &[], &[], 1.0 / 60.0);
    // Force a wrapped current turret by syncing again past the seam without touching previous.
    predictor.sync_to(&snapshot_with_aim(-3.10, 0.0));

    let mid = predictor.interpolated_pose(0.5).turret_yaw_rad;
    // The shortest arc between 3.10 and -3.10 passes through +/-PI (~3.14), not through 0.
    assert!(mid.abs() > 3.0, "expected midpoint near the seam, got {mid}");
}

#[test]
fn predictor_stays_in_lockstep_with_the_authoritative_server_under_steering() {
    use net::ClientInputCommand;
    use server::{LocalAuthoritativeServer, ServerTickConfig};
    use terrain::prokhorovka_hill_252_2;

    let mut server = LocalAuthoritativeServer::new(ServerTickConfig::default());
    let player = server.player_tank();
    let battlefield = prokhorovka_hill_252_2();

    let seed = server
        .latest_snapshot()
        .tanks
        .into_iter()
        .find(|tank| tank.tank_id == player)
        .expect("player in seed snapshot");
    let mut predictor = LocalPredictor::new(&TankSpec::t55a());
    predictor.sync_to(&seed);

    // Hard, sustained steering is the worst case for prediction/authoritative divergence.
    // We deliberately never re-sync so any per-tick drift accumulates and is measurable --
    // in the live client a 20 Hz `sync_to` would yank this gap closed as a visible "cofa".
    let command = TankCommand::drive(1.0, 0.6);
    let dt = 1.0 / 60.0;
    let mut max_drift = 0.0f32;
    for client_tick in 0..600 {
        predictor.step(command, &battlefield.heightmap, &battlefield.static_cover, &[], dt);
        let outcome =
            server.tick_with_input(ClientInputCommand { client_tick, tank_id: player, command });
        if let Some(snapshot) = outcome.snapshot {
            let auth = snapshot.tanks.iter().find(|tank| tank.tank_id == player).unwrap();
            let drift = (predictor.position() - Vec3::from_array(auth.position)).length();
            max_drift = max_drift.max(drift);
        }
    }

    assert!(max_drift < 0.01, "predictor drifted {max_drift} m from the server (snap-back/cofa)");
}
