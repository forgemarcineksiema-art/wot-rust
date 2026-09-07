//! The ram bill is the charge, not a hidden die (the one program's X7).
//!
//! The bill used to read the solver's per-tick impulse. The speculative contact shuts whatever
//! gap is left in the touch tick, so that impulse was the charge minus a sub-tick's worth of
//! travel — measured, ~268 HP or ~55 HP for the same charge by 8 cm of spawn distance. Now the
//! bill reads the PEAK closing speed of the whole approach (`physics::ContactImpact`), reported
//! once when the approach ends. A centimetre of spawn distance is a centimetre, not a die.

use std::f32::consts::FRAC_PI_2;

use game_core::{DamageCause, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

fn step() -> FixedTimestep {
    FixedTimestep::from_hz(60)
}

/// Charge a T-54 bow-first into the broadside of a parked enemy T-54 `distance_m` away and
/// report what the ram cost each of them (charger, target), in hit points.
fn tbone_bill(distance_m: f32) -> (u32, u32) {
    let mut state = SimulationState::new();
    let charger = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, distance_m));
    state.tank_mut(target).expect("target").yaw_rad = FRAC_PI_2;
    let mut billed = false;
    for _ in 0..900 {
        state.apply_commands(&[(charger, TankCommand::drive(1.0, 0.0))], step());
        if state.damage_events().iter().any(|event| event.cause == DamageCause::Ram) {
            billed = true;
            break;
        }
    }
    assert!(billed, "a t-bone from {distance_m} m must be billed");
    let full = TankSpec::t54_1951().hit_points;
    (
        full - state.tank(charger).expect("charger").hit_points,
        full - state.tank(target).expect("target").hit_points,
    )
}

/// The row's lock: a 1 cm sweep of spawn distance, 30.00…30.23 m, gives ≤ 5 % spread in the
/// bill — the sub-tick phase of the impact no longer decides what a collision costs.
#[test]
fn a_centimetre_of_spawn_distance_moves_the_ram_bill_by_a_centimetre_not_a_die() {
    let mut bills = Vec::new();
    for step_cm in 0..=23 {
        let distance_m = 30.0 + step_cm as f32 * 0.01;
        let (charger, target) = tbone_bill(distance_m);
        println!("{distance_m:.2} m: charger {charger} HP, target {target} HP");
        bills.push((charger, target));
    }
    for (name, pick) in [("charger", 0usize), ("target", 1usize)] {
        let values: Vec<f32> =
            bills.iter().map(|b| if pick == 0 { b.0 } else { b.1 } as f32).collect();
        let (min, max) =
            values.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        assert!(mean > 0.0, "the {name} must be billed at all");
        let spread = (max - min) / mean;
        assert!(
            spread <= 0.05,
            "the {name}'s bill spreads {:.1}% over 23 cm of spawn distance ({min}..{max} HP): a die",
            spread * 100.0
        );
    }
}

/// ...and it is billed ONCE per collision: a charger that keeps pushing after the impact pays
/// for the impact, not for every tick it leans on the hull it hit.
#[test]
fn a_collision_is_billed_once_however_long_the_push_lasts() {
    let mut state = SimulationState::new();
    let charger = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 30.0));
    state.tank_mut(target).expect("target").yaw_rad = FRAC_PI_2;
    // The event buffer is the tick's: count as we go.
    let mut rams = 0;
    for _ in 0..1_200 {
        state.apply_commands(&[(charger, TankCommand::drive(1.0, 0.0))], step());
        rams += state
            .damage_events()
            .iter()
            .filter(|event| event.cause == DamageCause::Ram && event.target == target)
            .count();
    }
    assert_eq!(rams, 1, "one collision, one bill on the target — got {rams}");
}
