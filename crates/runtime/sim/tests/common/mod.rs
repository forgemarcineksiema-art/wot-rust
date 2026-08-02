//! Shared fixtures for the fire-family integration tests.

use game_core::{TankId, TankSpec, TeamId};
use glam::Vec3;
use sim::SimulationState;

/// An arsonist and a victim, far enough apart that nothing else interferes.
pub fn two_tanks() -> (SimulationState, TankId, TankId) {
    let mut state = SimulationState::new();
    let arsonist = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let victim = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 120.0));
    (state, arsonist, victim)
}
