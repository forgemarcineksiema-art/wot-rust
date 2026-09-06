use game_core::{TankSpec, TeamId};
use glam::Vec3;
use sim::SimulationState;

#[test]
fn spawn_tank_with_yaw_sets_authoritative_hull_yaw() {
    let mut state = SimulationState::new();

    let tank = state.spawn_tank_with_yaw(
        TeamId(1),
        TankSpec::t54_1951(),
        Vec3::new(10.0, 2.0, 20.0),
        1.25,
    );

    let spawned = state.tank(tank).expect("spawned tank");
    assert_eq!(spawned.position, Vec3::new(10.0, 2.0, 20.0));
    assert!((spawned.yaw_rad - 1.25).abs() < 1.0e-6);
}
