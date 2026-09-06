//! Inny Poziom G7, lock (3): the brake dip is AUTHORITATIVE. A T-54 braking hard dips its nose
//! in the sim — the gun, measured from the sim's own hull pose, loses world elevation with it —
//! and a Centurion, whose vertical stabilizer (A12) cancels the hull's pitch change on the gun
//! each tick, holds its elevation through the same dip. The dive is no longer a client picture
//! the armour and the gun could not see.

use game_core::math::gun_direction_world;
use game_core::{TankSpec, TeamId, VehicleKind};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

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
fn braking_dips_the_authoritative_nose_and_the_unstabilized_gun_with_it() {
    let (pitch, before, lowest) = brake_from_speed(TankSpec::t54_1951());
    assert!(pitch < -0.004, "a braking T-54 dips its nose in the sim: {pitch} rad");
    assert!(
        lowest < before - 0.003,
        "and its gun, measured from the sim's hull pose, loses elevation: {before} -> {lowest}"
    );
}

#[test]
fn a_stabilized_gun_holds_its_elevation_through_the_brake_dip() {
    let spec = VehicleKind::Centurion.spec();
    assert!(spec.vertical_stabilizer > 0.99, "the Centurion carries the stabilizer");
    let (pitch, before, lowest) = brake_from_speed(spec);
    assert!(pitch < -0.003, "the Centurion's hull dips too: {pitch} rad");
    assert!(
        (before - lowest).abs() < 5.0e-4,
        "but the stabilizer holds the gun: {before} -> {lowest}"
    );
}
