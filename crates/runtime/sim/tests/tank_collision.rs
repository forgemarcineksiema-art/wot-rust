use game_core::{DamageCause, ModuleSlot, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

#[test]
fn driving_tank_stops_before_overlapping_stationary_tank() {
    let mut state = SimulationState::new();
    let mover = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let blocker = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 8.0));
    let step = FixedTimestep::from_hz(60);

    for _ in 0..600 {
        state.apply_commands(&[(mover, TankCommand::drive(1.0, 0.0))], step);
    }

    let mover_tank = state.tank(mover).expect("mover");
    let blocker_tank = state.tank(blocker).expect("blocker");
    let horizontal_delta = Vec3::new(
        mover_tank.position.x - blocker_tank.position.x,
        0.0,
        mover_tank.position.z - blocker_tank.position.z,
    );

    // Nose-to-tail contact distance: both hulls' half lengths, with a small settling margin.
    let contact = 2.0 * mover_tank.spec.hitbox.half_length_m - 0.05;
    assert!(
        horizontal_delta.length() >= contact,
        "tank hulls must stay separated, mover at {:?}, blocker at {:?}",
        mover_tank.position,
        blocker_tank.position
    );
    assert!(
        mover_tank.position.z <= blocker_tank.position.z - contact,
        "mover must not pass through the blocking tank"
    );
}

/// The user's design, locked: LIGHT contact is parking, not ramming — a slow roll into a
/// neighbour deals nothing to hull or suspension. And teammates NEVER grind each other:
/// a friendly shove at any speed is physics only.
#[test]
fn a_gentle_bump_and_a_friendly_ram_deal_nothing() {
    // Gentle: barely rolling into an enemy 7 m ahead — under the ram threshold at contact.
    let mut state = SimulationState::new();
    let mover = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 7.0));
    let step = FixedTimestep::from_hz(60);
    let hp_before = state.tank(target).expect("target").hit_points;
    for _ in 0..90 {
        state.apply_commands(&[(mover, TankCommand::drive(0.25, 0.0))], step);
    }
    assert!(
        !state.damage_events().iter().any(|event| event.cause == DamageCause::Ram),
        "a parking nudge is not a ram"
    );
    assert_eq!(state.tank(target).expect("target").hit_points, hp_before);

    // Friendly: a full-speed charge into a TEAMMATE still deals nothing.
    let mut state = SimulationState::new();
    let mover = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let ally = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 30.0));
    let ally_hp = state.tank(ally).expect("ally").hit_points;
    for _ in 0..600 {
        state.apply_commands(&[(mover, TankCommand::drive(1.0, 0.0))], step);
    }
    assert!(
        !state.damage_events().iter().any(|event| event.cause == DamageCause::Ram),
        "teammates never grind each other down"
    );
    assert_eq!(state.tank(ally).expect("ally").hit_points, ally_hp, "the ally is unhurt");
}

#[test]
fn high_speed_ramming_damages_both_tanks_and_running_gear() {
    let mut state = SimulationState::new();
    let rammer = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    // A 30 m runway: the honest ram threshold (~16 km/h) needs a real charge, not a nudge.
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 30.0));
    let step = FixedTimestep::from_hz(60);
    let rammer_hp = state.tank(rammer).expect("rammer").hit_points;
    let target_hp = state.tank(target).expect("target").hit_points;

    for _ in 0..600 {
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
