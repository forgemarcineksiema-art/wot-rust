//! Fire is a wound: it takes hit points, it is credited to whoever lit it, and the crew can put
//! it out. Before this, `engine_fire` and `fuel_fire` were set by the combat resolution and read
//! by nobody in the simulation — a burning tank lost exactly nothing.

use game_core::{DamageCause, DamageEvent, TankId, TankSpec, TeamId};
use glam::Vec3;
use sim::{
    ENGINE_FIRE_HP_PER_S, FIRE_FIGHT_S, FUEL_FIRE_HP_PER_S, FixedTimestep, SimulationState,
    burn_rate_hp_per_s,
};

/// `SimulationState` clears its damage events at the top of every step, so anything that wants to
/// see a whole burn has to collect as it goes.
fn step_collecting(state: &mut SimulationState, out: &mut Vec<DamageEvent>) {
    state.apply_commands(&[], FixedTimestep::from_hz(60));
    out.extend(state.damage_events().iter().cloned());
}

/// Light `victim` on behalf of `arsonist` and run `seconds` of battle, returning every damage
/// event the burn produced.
fn burn_for(
    state: &mut SimulationState,
    victim: TankId,
    arsonist: TankId,
    engine: bool,
    fuel: bool,
    seconds: f32,
) -> Vec<DamageEvent> {
    {
        let tank = state.tank_mut(victim).expect("victim");
        tank.engine_fire = engine;
        tank.fuel_fire = fuel;
        tank.fire_source = Some(arsonist);
        tank.fire_s = 0.0;
    }
    let mut events = Vec::new();
    for _ in 0..(seconds * 60.0).round() as u32 {
        step_collecting(state, &mut events);
    }
    events
}

fn two_tanks() -> (SimulationState, TankId, TankId) {
    let mut state = SimulationState::new();
    let arsonist = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let victim = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 120.0));
    (state, arsonist, victim)
}

#[test]
fn a_burning_hull_loses_hit_points_and_the_burn_is_credited_to_whoever_lit_it() {
    let (mut state, arsonist, victim) = two_tanks();
    let before = state.tank(victim).expect("victim").hit_points;

    let events = burn_for(&mut state, victim, arsonist, true, false, 3.0);

    let after = state.tank(victim).expect("victim").hit_points;
    assert!(after < before, "a burning hull must lose hit points, {before} -> {after}");

    let fire_events: Vec<_> = events.iter().filter(|e| e.cause == DamageCause::Fire).collect();
    assert!(!fire_events.is_empty(), "the burn must surface as DamageCause::Fire events");
    assert!(
        fire_events.iter().all(|e| e.source == arsonist && e.target == victim),
        "every hit point the fire takes is credited to the crew that lit it"
    );
}

#[test]
fn the_crew_smothers_the_fire_and_the_burn_costs_the_rate_times_the_fight() {
    let (mut state, arsonist, victim) = two_tanks();
    let before = state.tank(victim).expect("victim").hit_points;

    // Long enough that the crew has certainly won: the burn must STOP, not keep eating the hull.
    burn_for(&mut state, victim, arsonist, true, false, FIRE_FIGHT_S + 4.0);

    let tank = state.tank(victim).expect("victim");
    assert!(!tank.engine_fire && !tank.fuel_fire, "the crew puts the fire out");
    assert_eq!(tank.fire_source, None, "an extinguished fire credits nobody");

    let taken = before - tank.hit_points;
    let expected = (ENGINE_FIRE_HP_PER_S * FIRE_FIGHT_S) as u32;
    assert!(
        taken.abs_diff(expected) <= 2,
        "a fought fire costs rate x fight and nothing more: took {taken}, expected ~{expected}"
    );
}

#[test]
fn fuel_burns_hotter_than_the_engine_deck() {
    let (mut state, arsonist, engine_victim) = two_tanks();
    let fuel_victim =
        state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(40.0, 0.0, 120.0));
    let start = state.tank(engine_victim).expect("victim").hit_points;

    {
        let tank = state.tank_mut(fuel_victim).expect("fuel victim");
        tank.fuel_fire = true;
        tank.fire_source = Some(arsonist);
    }
    burn_for(&mut state, engine_victim, arsonist, true, false, 4.0);

    let engine_taken = start - state.tank(engine_victim).expect("victim").hit_points;
    let fuel_taken = start - state.tank(fuel_victim).expect("fuel victim").hit_points;
    assert!(
        fuel_taken > engine_taken,
        "burning fuel is the fiercer wound: fuel {fuel_taken} vs engine {engine_taken}"
    );
    const { assert!(FUEL_FIRE_HP_PER_S > ENGINE_FIRE_HP_PER_S) };
    // Both alight is ONE vehicle on fire at the worse rate, never the sum.
    assert_eq!(burn_rate_hp_per_s(true, true), FUEL_FIRE_HP_PER_S);
    assert_eq!(burn_rate_hp_per_s(false, false), 0.0);
}

#[test]
fn fire_finishes_a_wounded_hull_and_the_kill_belongs_to_the_arsonist() {
    let (mut state, arsonist, victim) = two_tanks();
    state.tank_mut(victim).expect("victim").hit_points = 40;

    let events = burn_for(&mut state, victim, arsonist, false, true, 4.0);

    let tank = state.tank(victim).expect("victim");
    assert_eq!(tank.hit_points, 0, "fire must be able to kill");
    assert!(!tank.engine_fire && !tank.fuel_fire, "a burnt-out wreck is no longer alight");
    let killing_blow = events.iter().rfind(|e| e.target == victim).expect("a killing event");
    assert_eq!(killing_blow.cause, DamageCause::Fire);
    assert_eq!(killing_blow.source, arsonist, "the burn-out is the arsonist's kill");
}

/// The fire and the module patch are separate repairs on separate clocks, and the fire's is the
/// SHORTER one (12 s against 15 s). So the honest observable is this: the crew beats the flames
/// while the engine they came out of is still dead. Putting an engine right was never the same act
/// as putting its fire out, and the two must not share an owner.
#[test]
fn the_fire_goes_out_on_its_own_clock_while_the_engine_is_still_wrecked() {
    let (mut state, arsonist, victim) = two_tanks();
    state.tank_mut(victim).expect("victim").modules.damage(game_core::ModuleSlot::Engine, u32::MAX);

    const { assert!(FIRE_FIGHT_S < sim::MODULE_PATCH_S) };
    burn_for(&mut state, victim, arsonist, true, false, FIRE_FIGHT_S + 1.0);

    let tank = state.tank(victim).expect("victim");
    assert!(!tank.engine_fire, "the crew smothers the fire on the fire's clock");
    assert!(
        !tank.modules.is_functional(game_core::ModuleSlot::Engine),
        "and the engine is still wrecked — the patch runs on its own, longer clock"
    );
}

#[test]
fn nothing_burns_by_default() {
    let (mut state, _, victim) = two_tanks();
    let before = state.tank(victim).expect("victim").hit_points;
    let mut events = Vec::new();
    for _ in 0..600 {
        step_collecting(&mut state, &mut events);
    }
    assert_eq!(
        state.tank(victim).expect("victim").hit_points,
        before,
        "an untouched hull does not smoulder"
    );
    assert!(events.iter().all(|e| e.cause != DamageCause::Fire));
}
