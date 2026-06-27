use game_core::TeamId;
use sim::{FixedTimestep, SimulationState};

use super::*;

/// The point of sharing one drive step: the locally predicted hull must track the authoritative
/// server tick-for-tick — pose *and* aim dispersion — so the player never sees an aim circle (or a
/// position) the server is not simulating.
#[test]
fn predictor_matches_the_server_pose_and_dispersion_tick_for_tick() {
    let flat = HeightMap::flat(64, 64, 4.0, 0.0).unwrap();
    let spec = TankSpec::t55a();
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
    };

    for tick in 0..60 {
        server.apply_commands_on_terrain(&[(tank_id, command)], step, &flat);
        predictor.step(command, &flat, &[], &[], step.dt_seconds());

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
