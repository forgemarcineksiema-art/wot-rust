//! A wreck is a physical object, not a frozen pose.
//!
//! The drive step is skipped for dead hulls, and the vertical resolution used to be skipped with
//! it — so a tank killed in mid-flight hung at the altitude it died at for the rest of the
//! battle, blocking shells and hulls (`StaticCoverKind::Wreck`) from a hole in the sky.

use game_core::{TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};
use terrain::HeightMap;

fn flat_field() -> HeightMap {
    HeightMap::flat(64, 64, 4.0, 0.0).expect("flat terrain")
}

#[test]
fn a_hull_killed_in_mid_air_falls_instead_of_hanging_there() {
    let terrain = flat_field();
    let mut state = SimulationState::new();
    let id = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(100.0, 0.0, 100.0));
    {
        let tank = state.tank_mut(id).expect("tank");
        tank.position.y = 20.0;
        tank.hit_points = 0;
    }

    // NOTHING is commanded: a wreck answers to no input, so the settle must run for it anyway —
    // the same rule drowning follows for hulls nobody is driving.
    let step = FixedTimestep::from_hz(60);
    for _ in 0..300 {
        state.apply_commands_on_terrain(&[], step, &terrain);
    }

    let landed = state.tank(id).expect("tank").position.y;
    assert!(landed.abs() < 0.05, "the wreck must come down to the ground, got y {landed}");
}

#[test]
fn a_wreck_already_resting_on_its_support_does_not_move() {
    let terrain = flat_field();
    let mut state = SimulationState::new();
    let id = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(100.0, 0.0, 100.0));
    let step = FixedTimestep::from_hz(60);
    // One live tick grounds it exactly where the support envelope puts it...
    state.apply_commands_on_terrain(&[(id, TankCommand::idle())], step, &terrain);
    let resting = state.tank(id).expect("tank").position;
    state.tank_mut(id).expect("tank").hit_points = 0;

    for _ in 0..120 {
        state.apply_commands_on_terrain(&[(id, TankCommand::idle())], step, &terrain);
    }

    // ...and killing it must not nudge it by a millimetre. This is what keeps the settle free of
    // replay drift: the overwhelming common case (a hull dies on the ground it stood on) is a
    // bit-identical no-op.
    assert_eq!(state.tank(id).expect("tank").position, resting, "a resting wreck must not drift");
}
