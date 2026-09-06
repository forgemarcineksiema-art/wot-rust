//! Inny Poziom G7, the lock written BEFORE the hull moves: a hull at rest in terrain-free mode
//! is bit-exact level — pitch, roll and height all zero to the bit — at the 60 Hz tick AND at
//! the 20 Hz the drive replays run at. Every terrain-free replay fixture (`drive_forward_v1`,
//! `fire_penetration_v1`, `perforation_v1`) hangs its exact numbers on this; a sprung hull
//! that sags, creeps or rings at rest by one ulp would move all three, so the spring's rest
//! must be an exact fixed point of its integrator at both rates.

use game_core::TeamId;
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

fn rest_after(hz: u32, ticks: u32) -> (u32, u32, u32) {
    let step = FixedTimestep::from_hz(hz);
    let mut sim = SimulationState::new();
    let spec = game_core::TankSpec::t54_1951();
    let id = sim.spawn_tank(TeamId(1), spec, Vec3::ZERO);
    for _ in 0..ticks {
        sim.apply_commands(&[(id, TankCommand::idle())], step);
    }
    let tank = sim.tank(id).expect("tank");
    (tank.hull_pitch_rad.to_bits(), tank.hull_roll_rad.to_bits(), tank.position.y.to_bits())
}

#[test]
fn a_resting_hull_is_bit_exact_level_at_sixty_and_twenty_hertz() {
    for hz in [60, 20] {
        let (pitch, roll, y) = rest_after(hz, 240);
        assert_eq!(pitch, 0.0_f32.to_bits(), "pitch at rest is exactly zero at {hz} Hz");
        assert_eq!(roll, 0.0_f32.to_bits(), "roll at rest is exactly zero at {hz} Hz");
        assert_eq!(y, 0.0_f32.to_bits(), "height at rest is exactly zero at {hz} Hz");
    }
}

/// And a hull that was disturbed comes back to that exact rest, not to a neighbourhood of it:
/// a tilt injected by hand is walked back to the bit-exact level the fixtures expect.
#[test]
fn a_disturbed_hull_returns_to_the_bit_exact_rest() {
    let step = FixedTimestep::from_hz(60);
    let mut sim = SimulationState::new();
    let spec = game_core::TankSpec::t54_1951();
    let id = sim.spawn_tank(TeamId(1), spec, Vec3::ZERO);
    sim.tank_mut(id).expect("tank").hull_pitch_rad = -0.2;
    sim.tank_mut(id).expect("tank").hull_roll_rad = 0.15;
    for _ in 0..600 {
        sim.apply_commands(&[(id, TankCommand::idle())], step);
    }
    let tank = sim.tank(id).expect("tank");
    assert_eq!(tank.hull_pitch_rad.to_bits(), 0.0_f32.to_bits(), "pitch back to exact level");
    assert_eq!(tank.hull_roll_rad.to_bits(), 0.0_f32.to_bits(), "roll back to exact level");
}
