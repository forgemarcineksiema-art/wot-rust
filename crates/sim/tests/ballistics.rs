use game_core::{TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};
use terrain::HeightMap;

#[test]
fn shell_falls_under_gravity_after_firing() {
    let mut state = SimulationState::new();
    let id = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::new(50.0, 0.0, 50.0));
    let terrain = HeightMap::flat(64, 64, 4.0, 0.0).expect("flat terrain");
    let step = FixedTimestep::from_hz(60);

    state.apply_commands_on_terrain(
        &[(id, TankCommand { fire: true, ..TankCommand::idle() })],
        step,
        &terrain,
    );
    let launch = state.shells().first().expect("a shell was fired").velocity_mps.y;

    for _ in 0..10 {
        state.apply_commands_on_terrain(&[(id, TankCommand::idle())], step, &terrain);
    }
    let shell = state.shells().first().expect("shell still in flight");
    assert!(shell.velocity_mps.y < launch, "gravity must reduce vertical velocity");
}

#[test]
fn gun_elevation_clamps_to_its_arc() {
    let mut state = SimulationState::new();
    let id = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let step = FixedTimestep::from_hz(60);

    for _ in 0..600 {
        let command = TankCommand { gun_pitch_delta: 1.0, ..TankCommand::idle() };
        state.apply_commands(&[(id, command)], step);
    }
    let elevation = state.tank(id).expect("tank").gun_pitch_rad;
    assert!(
        (0.30..=0.40).contains(&elevation),
        "elevation should saturate near max, got {elevation}"
    );

    for _ in 0..600 {
        let command = TankCommand { gun_pitch_delta: -1.0, ..TankCommand::idle() };
        state.apply_commands(&[(id, command)], step);
    }
    let depression = state.tank(id).expect("tank").gun_pitch_rad;
    assert!((-0.16..=-0.10).contains(&depression), "depression should saturate, got {depression}");
}
