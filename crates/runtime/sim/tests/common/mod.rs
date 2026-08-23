//! Shared fixtures for sim integration tests.
#![allow(dead_code)]

use game_core::{TankId, TankSpec, TeamId};
use glam::Vec3;
use sim::SimulationState;

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
