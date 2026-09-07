//! Inny Poziom G7, lock (3), amended by the one program's J4: the brake dip is AUTHORITATIVE and
//! the GUN HOLDS THROUGH IT. A T-54 braking hard dips its nose in the sim — the armour tilts with
//! it — but its mount holds the dive share of that pitch, so the gun keeps its world elevation
//! (the owner, 2026-09-07: „stabilizować udział nurka dla każdego działa jak w WoT”). What the
//! historical stabilizer still buys the Centurion is the TERRAIN share: over a crest the T-54's
//! gun rides the hull, the Centurion's does not.

use game_core::math::gun_direction_world;
use game_core::{TankSpec, TeamId, VehicleKind};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};
use terrain::HeightMap;

/// Drive to speed, then brake; return the hull's lowest pitch and the gun's world elevation
/// before braking and at its lowest during it.
fn brake_from_speed(spec: TankSpec) -> (f32, f32, f32) {
    let step = FixedTimestep::from_hz(60);
    let mut sim = SimulationState::new();
    let id = sim.spawn_tank(TeamId(1), spec, Vec3::ZERO);
    for _ in 0..240 {
        sim.apply_commands(&[(id, TankCommand::drive(1.0, 0.0))], step);
    }
    let elevation = |sim: &SimulationState| {
        let tank = sim.tank(id).expect("tank");
        gun_direction_world(tank.hull_pose(), tank.turret_yaw_rad, tank.gun_pitch_rad).y
    };
    let before = elevation(&sim);
    let (mut lowest_pitch, mut lowest_elevation) = (0.0_f32, before);
    let brake = TankCommand { brake: 1.0, ..TankCommand::idle() };
    for _ in 0..90 {
        sim.apply_commands(&[(id, brake)], step);
        lowest_pitch = lowest_pitch.min(sim.tank(id).expect("tank").hull_pitch_rad);
        lowest_elevation = lowest_elevation.min(elevation(&sim));
    }
    (lowest_pitch, before, lowest_elevation)
}

#[test]
fn braking_dips_the_authoritative_nose_and_every_gun_holds_through_the_dip() {
    for kind in VehicleKind::PLAYABLE {
        let (pitch, before, lowest) = brake_from_speed(kind.spec());
        assert!(pitch < -0.003, "a braking {kind:?} dips its nose in the sim: {pitch} rad");
        assert!(
            (before - lowest).abs() < 5.0e-4,
            "{kind:?}: the mount holds the dive share, the gun keeps its elevation: \
             {before} -> {lowest}"
        );
    }
}

/// Drive a hull up a constant 10 % grade from level ground: the hull pitches nose-up by the
/// terrain's share, which is exactly what a historical stabilizer holds and nothing else does.
fn climb_onto_a_grade(spec: TankSpec) -> (f32, f32, f32) {
    let step = FixedTimestep::from_hz(60);
    let (width, height, cell) = (80usize, 80usize, 5.0f32);
    let samples: Vec<f32> = (0..width * height)
        .map(|index| {
            let z = (index / width) as f32 * cell;
            ((z - 100.0) * 0.10).max(0.0)
        })
        .collect();
    let heightmap = HeightMap::new(width, height, cell, samples).expect("ramp heightmap");
    let mut sim = SimulationState::new();
    let id = sim.spawn_tank(TeamId(1), spec, Vec3::new(200.0, 0.0, 40.0));
    let elevation = |sim: &SimulationState| {
        let tank = sim.tank(id).expect("tank");
        gun_direction_world(tank.hull_pose(), tank.turret_yaw_rad, tank.gun_pitch_rad).y
    };
    for _ in 0..120 {
        sim.apply_commands_on_terrain(&[(id, TankCommand::drive(0.6, 0.0))], step, &heightmap);
    }
    let on_the_flat = elevation(&sim);
    for _ in 0..900 {
        sim.apply_commands_on_terrain(&[(id, TankCommand::drive(0.6, 0.0))], step, &heightmap);
    }
    let pitch = sim.tank(id).expect("tank").hull_pitch_rad;
    (pitch, on_the_flat, elevation(&sim))
}

#[test]
fn the_terrain_share_rides_an_unstabilized_gun_and_a_stabilized_one_holds() {
    let (pitch, flat, on_grade) = climb_onto_a_grade(TankSpec::t54_1951());
    assert!(pitch > 0.05, "the T-54 sits nose-up on the grade: {pitch} rad");
    assert!(
        on_grade > flat + 0.03,
        "an unstabilized gun rides the terrain pitch: {flat} -> {on_grade}"
    );

    let spec = VehicleKind::Centurion.spec();
    assert!(spec.vertical_stabilizer > 0.99, "the Centurion carries the stabilizer");
    let (pitch, flat, on_grade) = climb_onto_a_grade(spec);
    assert!(pitch > 0.05, "the Centurion's hull sits nose-up too: {pitch} rad");
    assert!(
        (on_grade - flat).abs() < 2.0e-3,
        "but its stabilizer holds the gun over the grade: {flat} -> {on_grade}"
    );
}
