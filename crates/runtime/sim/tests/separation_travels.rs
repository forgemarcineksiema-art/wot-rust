//! A HULL NEVER ARRIVES WHERE IT DID NOT DRIVE — P1.4 of `docs/contact-and-tracks-program.md`.
//!
//! The tick used to end with a positional pass: anything still overlapping was pushed apart by
//! writing hull positions directly. It was the last place in the simulation where a hull could
//! arrive somewhere it never travelled to — the drive knew nothing about it, the attitude and ride
//! height had been computed for where it used to be, and the opening audit measured a pivoting
//! pair squirting apart at 1.19 m/s with no velocity behind it.
//!
//! Measured at its worst, on the case that pass existed for: two hulls spawned three metres inside
//! each other were separated in ONE TICK by moving a hull **1.489 m** — eighty-nine metres per
//! second of travel that never happened, on a vehicle whose top speed is fourteen.
//!
//! It is gone. Separation is a velocity now, all of it, and these are the numbers that says so.

use game_core::{TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

fn step() -> FixedTimestep {
    FixedTimestep::from_hz(60)
}

/// The fastest a T-54 drives, per tick. Nothing a contact does may move a hull further than the
/// vehicle itself could — that is the whole difference between being pushed and being placed.
const FASTEST_HONEST_STEP_M: f32 = 13.89 / 60.0;

/// Two hulls spawned inside each other separate by DRIVING apart, not by being placed apart.
#[test]
fn a_spawn_overlap_is_walked_out_of_not_teleported_out_of() {
    for overlap in [0.10_f32, 1.50, 3.00] {
        let spec = TankSpec::t54_1951();
        let half_width = spec.hull_plan().half_width_m;
        let mut state = SimulationState::new();
        let a = state.spawn_tank_with_yaw(TeamId(1), spec.clone(), Vec3::ZERO, 0.0);
        let b = state.spawn_tank_with_yaw(
            TeamId(2),
            spec.clone(),
            Vec3::new(2.0 * half_width - overlap, 0.0, 0.0),
            0.0,
        );
        let idle = [(a, TankCommand::idle()), (b, TankCommand::idle())];

        let mut worst_step = 0.0_f32;
        let mut previous = state.tank(a).expect("tank").position;
        for _ in 0..600 {
            state.apply_commands(&idle, step());
            let now = state.tank(a).expect("tank").position;
            worst_step = worst_step.max((now - previous).length());
            previous = now;
        }
        let left = 2.0 * half_width
            - (state.tank(b).expect("tank").position.x - state.tank(a).expect("tank").position.x)
                .abs();
        println!(
            "spawn overlap {overlap:.2} m: worst step {worst_step:.5} m/tick, {left:.4} m left"
        );

        assert!(
            worst_step <= FASTEST_HONEST_STEP_M,
            "a {overlap:.2} m overlap moved a hull {worst_step:.4} m in one tick — faster than the \
             tank can drive, so it was placed there rather than pushed"
        );
        // ...and it does actually come apart, rather than easing so gently it never finishes.
        assert!(
            left <= 0.021,
            "a {overlap:.2} m overlap still has {left:.4} m of it after ten seconds"
        );
    }
}

/// A hull pivoting into its neighbour still moves it — the contact is a real exchange, not a
/// silently dropped one — and neither of them is thrown to do it.
#[test]
fn a_pivot_into_a_neighbour_pushes_it_without_throwing_either() {
    let spec = TankSpec::t54_1951();
    let half_width = spec.hull_plan().half_width_m;
    let mut state = SimulationState::new();
    let a = state.spawn_tank_with_yaw(TeamId(1), spec.clone(), Vec3::ZERO, 0.0);
    let b = state.spawn_tank_with_yaw(
        TeamId(2),
        spec.clone(),
        Vec3::new(2.0 * half_width + 0.02, 0.0, 0.0),
        0.0,
    );
    // A turns in place, into B. Its corner sweeps across the gap; B has to learn about it.
    let go = [(a, TankCommand::drive(0.0, 1.0)), (b, TankCommand::drive(0.0, 0.0))];

    let start_b = state.tank(b).expect("tank").position;
    let (mut worst_a, mut worst_b) = (0.0_f32, 0.0_f32);
    let mut prev_a = state.tank(a).expect("tank").position;
    let mut prev_b = start_b;
    for _ in 0..300 {
        state.apply_commands(&go, step());
        let (now_a, now_b) =
            (state.tank(a).expect("tank").position, state.tank(b).expect("tank").position);
        worst_a = worst_a.max((now_a - prev_a).length());
        worst_b = worst_b.max((now_b - prev_b).length());
        prev_a = now_a;
        prev_b = now_b;
    }
    let shoved = (state.tank(b).expect("tank").position - start_b).length();
    println!(
        "pivot: attacker {worst_a:.5} m/tick, victim {worst_b:.5} m/tick, victim moved \
         {shoved:.3} m in all"
    );

    assert!(shoved > 0.05, "the neighbour never learned it was being leaned on");
    for (who, step) in [("the pivoting hull", worst_a), ("its neighbour", worst_b)] {
        assert!(
            step <= FASTEST_HONEST_STEP_M,
            "{who} moved {step:.4} m in one tick — a shove, not a throw, is the promise"
        );
    }
}
