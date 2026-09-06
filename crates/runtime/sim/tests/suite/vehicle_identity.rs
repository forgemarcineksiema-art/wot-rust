use game_core::{TankSpec, TeamId, VehicleKind};
use glam::Vec3;
use sim::SimulationState;

#[test]
fn spawned_tanks_carry_their_vehicle_kind() {
    let mut state = SimulationState::new();
    let rotating = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let casemate = state.spawn_tank(TeamId(2), TankSpec::jagdtiger(), Vec3::new(0.0, 0.0, 40.0));

    assert_eq!(state.tank(rotating).expect("t54 spawned").spec.kind, VehicleKind::T54_1951);
    assert_eq!(state.tank(casemate).expect("jagdtiger spawned").spec.kind, VehicleKind::Jagdtiger);
}
