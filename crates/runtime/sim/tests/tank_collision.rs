use game_core::{DamageCause, ModuleSlot, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

#[test]
fn driving_tank_stops_before_overlapping_stationary_tank() {
    let mut state = SimulationState::new();
    let mover = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let blocker = state.spawn_tank(TeamId(2), TankSpec::t55a(), Vec3::new(0.0, 0.0, 8.0));
    let step = FixedTimestep::from_hz(60);

    for _ in 0..180 {
        state.apply_commands(&[(mover, TankCommand::drive(1.0, 0.0))], step);
    }

    let mover_tank = state.tank(mover).expect("mover");
    let blocker_tank = state.tank(blocker).expect("blocker");
    let horizontal_delta = Vec3::new(
        mover_tank.position.x - blocker_tank.position.x,
        0.0,
        mover_tank.position.z - blocker_tank.position.z,
    );

    assert!(
        horizontal_delta.length() >= 6.35,
        "tank hulls must stay separated, mover at {:?}, blocker at {:?}",
        mover_tank.position,
        blocker_tank.position
    );
    assert!(
        mover_tank.position.z <= blocker_tank.position.z - 6.35,
        "mover must not pass through the blocking tank"
    );
}

#[test]
fn high_speed_ramming_damages_both_tanks_and_running_gear() {
    let mut state = SimulationState::new();
    let rammer = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), TankSpec::t55a(), Vec3::new(0.0, 0.0, 8.0));
    let step = FixedTimestep::from_hz(60);
    let rammer_hp = state.tank(rammer).expect("rammer").hit_points;
    let target_hp = state.tank(target).expect("target").hit_points;

    for _ in 0..180 {
        state.apply_commands(&[(rammer, TankCommand::drive(1.0, 0.0))], step);
        if state.damage_events().iter().any(|event| event.cause == DamageCause::Ram) {
            break;
        }
    }

    assert!(
        state.damage_events().iter().any(|event| {
            event.source == rammer
                && event.target == target
                && event.cause == DamageCause::Ram
                && event.module == Some(ModuleSlot::Suspension)
                && event.damage_hp > 0
        }),
        "ramming should damage the impacted tank and its running gear"
    );
    assert!(
        state.damage_events().iter().any(|event| {
            event.source == target
                && event.target == rammer
                && event.cause == DamageCause::Ram
                && event.damage_hp > 0
        }),
        "ramming should also damage the rammer"
    );
    assert!(state.tank(rammer).expect("rammer").hit_points < rammer_hp);
    assert!(state.tank(target).expect("target").hit_points < target_hp);
}
