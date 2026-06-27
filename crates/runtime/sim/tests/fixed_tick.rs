use game_core::{TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

#[test]
fn fixed_tick_moves_tank_and_rotates_turret_deterministically() {
    let run = || {
        let mut state = SimulationState::new();
        let tank_id = state.spawn_tank(TeamId(1), TankSpec::medium_test_tank(), Vec3::ZERO);
        let command = TankCommand::drive_with_turret(1.0, 0.0, 0.5);
        state.apply_commands(&[(tank_id, command)], FixedTimestep::from_hz(20));
        (state.tick(), state.tank(tank_id).cloned().expect("tank should exist after spawn"))
    };

    let (tick, tank) = run();
    assert_eq!(tick, 1);
    assert!(tank.position.z > 0.0);
    assert!(tank.turret_yaw_rad > 0.0);
    // Re-running identical inputs must reproduce identical state (true determinism).
    assert_eq!(run(), (tick, tank));
}

#[test]
fn fixed_timestep_keeps_hz_as_source_of_truth() {
    let ts = FixedTimestep::from_hz(60);
    assert_eq!(ts.hz(), 60);
    assert!((ts.dt_seconds() - 1.0 / 60.0).abs() < 1e-9);
}
