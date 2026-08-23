//! Locks for the crew-damage foundation (v46): a penetration crossing a crew STATION takes the
//! man out — deterministically, by geometry and an energy threshold, with no roll anywhere.
//!
//! The promises, one per test: the shell's path decides WHO is hit (and it is exactly the man
//! whose station it crossed); a spent round knocks nobody; a knock is a crew wound, never hull
//! damage of its own; the covered station works at a penalty the battle can feel (the loader's
//! reload); and first aid brings the man back weakened, on the clock, for the rest of the battle.

use std::f32::consts::PI;

use game_core::{
    CREW_FIRST_AID_S, CREW_WEAKENED_EFFECTIVENESS, CrewMemberState, CrewRole, TankId, TankSpec,
    TeamId,
};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

mod common;
use common::run_until_shells_clear;

/// One flank shot at the T-54 tower, aimed down the gunner's station line. Returns the state
/// after resolution for whatever the test wants to read.
fn tower_flank_shot(penetration_mm_at_100m: f32, z_offset: f32) -> (SimulationState, TankId) {
    let mut state = SimulationState::new();
    let shooter =
        state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(-55.0, 0.0, z_offset));
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::ZERO);
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.yaw_rad = PI / 2.0;
        // Depress onto the upper hull side at torso height (~1.54 m world): the tower crew's
        // seated capsules top out near 1.8, and a flat shot from the 1.78 m muzzle passes over
        // their heads.
        shooter.gun_pitch_rad = -0.004;
        shooter.aim_dispersion_mrad = 0.0;
        shooter.spec.gun.dispersion_mrad = 0.0;
        shooter.spec.gun.shell.penetration_mm_at_100m = penetration_mm_at_100m;
    }
    run_until_shells_clear(&mut state, shooter);
    (state, target)
}

#[test]
fn a_flank_penetration_through_the_gunners_station_knocks_exactly_the_gunner() {
    // The gunner's capsule stands at turret z 0.22..0.30 (t54.rs #18); the flat flank shot rides
    // the shooter's z.
    let (state, target) = tower_flank_shot(300.0, 0.26);
    let event = state.damage_events().last().expect("the shot resolved");
    assert!(event.penetrated, "the lock needs a penetration");
    assert_eq!(
        event.crew_hits_mask,
        CrewRole::Gunner.mask_bit(),
        "the shell crossed the gunner's station and nobody else's"
    );
    let tank = state.tank(target).expect("target");
    assert!(
        matches!(tank.crew.state(CrewRole::Gunner), CrewMemberState::Unconscious { .. }),
        "the gunner is down"
    );
    assert_eq!(tank.crew.state(CrewRole::Driver), CrewMemberState::Active);
    assert_eq!(tank.crew.state(CrewRole::Loader), CrewMemberState::Active);
}

#[test]
fn a_spent_round_dribbling_past_the_seat_knocks_nobody() {
    // 140 mm against the tower flank's measured 133 mm effective: it gets IN, with ~8 mm left —
    // below `CREW_KNOCK_ENERGY_MM` by the time it reaches the seat.
    let (state, target) = tower_flank_shot(140.0, 0.26);
    let event = state.damage_events().last().expect("the shot resolved");
    assert!(event.penetrated, "the comparison needs the round inside");
    assert_eq!(event.crew_hits_mask, 0, "a spent round knocks nobody");
    let tank = state.tank(target).expect("target");
    assert_eq!(tank.crew.state(CrewRole::Gunner), CrewMemberState::Active);
}

#[test]
fn a_crew_hit_is_a_crew_wound_not_extra_hull_damage() {
    let (with_hit, _) = tower_flank_shot(300.0, 0.26);
    let (without_hit, _) = tower_flank_shot(300.0, 0.62);
    let hit_event = with_hit.damage_events().last().expect("crew-line shot");
    let miss_event = without_hit.damage_events().last().expect("clear-line shot");
    assert_eq!(hit_event.crew_hits_mask, CrewRole::Gunner.mask_bit());
    assert_eq!(
        hit_event.damage_hp, miss_event.damage_hp,
        "the man in the path must not change what the shell does to the hull pool"
    );
}

#[test]
fn a_downed_loader_slows_the_reload_and_first_aid_brings_him_back_weakened() {
    let mut state = SimulationState::new();
    let tank_id = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let whole_reload = state.tank(tank_id).expect("tank").full_reload_seconds();

    state.tank_mut(tank_id).expect("tank").crew.knock(CrewRole::Loader);
    let covered_reload = state.tank(tank_id).expect("tank").full_reload_seconds();
    assert!(
        (covered_reload - whole_reload * 2.0).abs() < 1.0e-3,
        "a covered loader's station loads at half pace: {covered_reload} vs {whole_reload}"
    );

    // First aid runs in the ordinary per-tank pass; after the window the loader is back,
    // weakened for the rest of the battle.
    let dt = FixedTimestep::from_hz(60);
    let ticks = (CREW_FIRST_AID_S * 60.0) as u32 + 2;
    for _ in 0..ticks {
        state.apply_commands(&[(tank_id, TankCommand::idle())], dt);
    }
    let tank = state.tank(tank_id).expect("tank");
    assert_eq!(tank.crew.state(CrewRole::Loader), CrewMemberState::Weakened);
    let scarred_reload = tank.full_reload_seconds();
    assert!(
        (scarred_reload - whole_reload / CREW_WEAKENED_EFFECTIVENESS).abs() < 1.0e-3,
        "the scar is permanent and priced: {scarred_reload} vs whole {whole_reload}"
    );
}

#[test]
fn a_dead_hull_bandages_nobody() {
    let mut state = SimulationState::new();
    let tank_id = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    {
        let tank = state.tank_mut(tank_id).expect("tank");
        tank.crew.knock(CrewRole::Driver);
        tank.hit_points = 0;
    }
    let dt = FixedTimestep::from_hz(60);
    for _ in 0..((CREW_FIRST_AID_S * 60.0) as u32 + 2) {
        state.apply_commands(&[(tank_id, TankCommand::idle())], dt);
    }
    assert!(
        matches!(
            state.tank(tank_id).expect("tank").crew.state(CrewRole::Driver),
            CrewMemberState::Unconscious { .. }
        ),
        "a wreck's crew does not recover"
    );
}
