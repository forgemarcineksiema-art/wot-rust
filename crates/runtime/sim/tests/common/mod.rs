//! Shared fixtures for sim integration tests.
#![allow(dead_code)]

use game_core::{TankId, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};
use terrain::HeightMap;

/// Pitch that puts a rear shot through the T-54 bustle clips (`damage_layout/t54.rs`).
pub fn pitch_at_t54_bustle(state: &SimulationState, shooter: TankId, target: TankId) -> f32 {
    let target = state.tank(target).expect("target");
    let shooter = state.tank(shooter).expect("shooter");
    let local = Vec3::new(0.0, 0.74, -0.55);
    let bustle = target.position
        + Vec3::Y * target.spec.hitbox.center_y_m
        + target.hull_pose().basis() * local;
    let muzzle = shooter.muzzle_world_position();
    let delta = bustle - muzzle;
    delta.y.atan2(delta.x.hypot(delta.z))
}

/// An arsonist and a victim, far enough apart that nothing else interferes.
pub fn two_tanks() -> (SimulationState, TankId, TankId) {
    let mut state = SimulationState::new();
    let arsonist = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let victim = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 120.0));
    (state, arsonist, victim)
}

/// A single trigger pull on an otherwise idle tank.
pub fn fire_command() -> TankCommand {
    TankCommand { fire: true, ..TankCommand::idle() }
}

/// Fires once and steps until the first damage event lands (the shot resolved against armor).
pub fn run_until_shell_resolved(state: &mut SimulationState, shooter: TankId) {
    let step = FixedTimestep::from_hz(60);
    state.apply_commands(&[(shooter, fire_command())], step);
    for _ in 0..30 {
        state.apply_commands(&[], step);
        if !state.damage_events().is_empty() {
            return;
        }
    }
    panic!("shell should resolve against target");
}

/// Fires once and steps until the shell is GONE — resolution by absence, for tests that read
/// the post-impact state rather than the damage event itself. Not the same wait as
/// [`run_until_shell_resolved`]: two helpers that shared a name until the burn made the
/// difference visible.
pub fn run_until_shells_clear(state: &mut SimulationState, shooter: TankId) {
    let dt = FixedTimestep::from_hz(60);
    state.apply_commands(&[(shooter, TankCommand { fire: true, ..TankCommand::idle() })], dt);
    assert_eq!(state.shells().len(), 1, "the shot must leave the barrel");
    for _ in 0..600 {
        state.apply_commands(&[], dt);
        if state.shells().is_empty() {
            return;
        }
    }
    panic!("shell should resolve against the target");
}

/// The 4 m-cell flat 96x96 field the destruction tests shoot over.
pub fn flat_field() -> HeightMap {
    HeightMap::flat(96, 96, 4.0, 0.0).expect("flat terrain")
}

/// The commander's eye over a ground point on a compiled map: the hitbox top, exactly as
/// `sim::spotting::observer_eye` stacks it over the benchmark hull standing there. Shared
/// by the map-promise suites (Bystra hull-down, Orliny watchtower) — one rule, one body.
pub fn commander_eye(map: &terrain::BattlefieldMap, x: f32, z: f32) -> Vec3 {
    let spec = TankSpec::t54_1951();
    let ground = map.heightmap.sample_height(x, z).expect("probe on the map");
    Vec3::new(x, ground + spec.hitbox.center_y_m + spec.hitbox.half_height_m, z)
}
