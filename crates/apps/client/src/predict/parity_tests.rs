use game_core::TeamId;
use net::TankSnapshot;
use sim::{FixedTimestep, SimulationState};

use super::*;

/// The point of sharing one drive step: the locally predicted hull must track the authoritative
/// server tick-for-tick — pose *and* aim dispersion — so the player never sees an aim circle (or a
/// position) the server is not simulating.
#[test]
fn predictor_matches_the_server_pose_and_dispersion_tick_for_tick() {
    let flat = HeightMap::flat(64, 64, 4.0, 0.0).unwrap();
    let spec = TankSpec::t54_1951();
    let step = FixedTimestep::from_hz(60);

    let mut server = SimulationState::new();
    let tank_id = server.spawn_tank(TeamId(1), spec.clone(), Vec3::new(10.0, 0.0, 10.0));
    let mut predictor = LocalPredictor::new(&spec);
    predictor.sync_to(&TankSnapshot::from(server.tank(tank_id).expect("spawned tank")));

    // Drive, steer, and traverse at once so movement, aiming, and bloom all exercise the step.
    let command = TankCommand {
        throttle: 1.0,
        steer: 0.3,
        turret_yaw_delta: 0.7,
        gun_pitch_delta: 0.4,
        brake: 0.0,
        fire: false,
        select_ammo: None,
    };

    for tick in 0..60 {
        server.apply_commands_on_terrain(&[(tank_id, command)], step, &flat);
        predictor.step(command, &flat, &[], &[], &[], None, step.dt_seconds());

        let tank = server.tank(tank_id).expect("tank");
        assert!(
            (predictor.position() - tank.position).length() < 1.0e-2,
            "tick {tick}: predicted {:?} vs server {:?}",
            predictor.position(),
            tank.position
        );
        assert!((predictor.yaw() - tank.yaw_rad).abs() < 1.0e-3, "tick {tick}: yaw");
        assert!(
            (predictor.turret_yaw() - tank.turret_yaw_rad).abs() < 1.0e-4,
            "tick {tick}: turret"
        );
        assert!((predictor.gun_pitch() - tank.gun_pitch_rad).abs() < 1.0e-4, "tick {tick}: pitch");
        assert!(
            (predictor.aim_dispersion_mrad() - tank.aim_dispersion_mrad).abs() < 1.0e-3,
            "tick {tick}: dispersion {} vs {}",
            predictor.aim_dispersion_mrad(),
            tank.aim_dispersion_mrad
        );
    }
}

/// Vertical dynamics run the same shared code: launching off a ledge, the ballistic arc, and the
/// landing must match the server tick-for-tick — including the hull height mid-flight.
#[test]
fn predictor_matches_the_server_through_a_launch_flight_and_landing() {
    // A 4 m plateau ending at z = 24 on a 1 m grid, then flat ground: tall enough for a real
    // multi-tick flight, small enough to keep the run inside the map.
    let mut samples = Vec::with_capacity(61 * 61);
    for z in 0..61 {
        for x in 0..61 {
            let _ = x;
            samples.push(if z < 24 { 4.0 } else { 0.0 });
        }
    }
    let map = HeightMap::new(61, 61, 1.0, samples).expect("test heightmap dimensions are fixed");
    let spec = TankSpec::t54_1951();
    let step = FixedTimestep::from_hz(60);

    let mut server = SimulationState::new();
    let tank_id = server.spawn_tank(TeamId(1), spec.clone(), Vec3::new(30.0, 4.0, 10.0));
    let mut predictor = LocalPredictor::new(&spec);
    predictor.sync_to(&TankSnapshot::from(server.tank(tank_id).expect("spawned tank")));

    let command = TankCommand::drive(1.0, 0.0);
    let mut flew = false;
    for tick in 0..600 {
        server.apply_commands_on_terrain(&[(tank_id, command)], step, &map);
        predictor.step(command, &map, &[], &[], &[], None, step.dt_seconds());

        let tank = server.tank(tank_id).expect("tank");
        assert!(
            (predictor.position() - tank.position).length() < 1.0e-3,
            "tick {tick}: predicted {:?} vs server {:?}",
            predictor.position(),
            tank.position
        );
        // The hull is airborne once its height is off both plateau and ground level.
        if tank.position.y > 0.05 && tank.position.y < 3.95 {
            flew = true;
        }
    }
    assert!(flew, "the run must include a real flight, not just the plateau and the floor");
    let tank = server.tank(tank_id).expect("tank");
    assert!(tank.position.y < 0.05, "the run must end landed on the low ground");
}
