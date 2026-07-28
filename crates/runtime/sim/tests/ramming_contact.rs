//! Negative contact tests for ramming, per the engineering rule that every contact-shape
//! approximation needs a locked scenario that must NOT happen. The pre-fix predicate compared
//! center distance against summed hull half-lengths, so tanks passing cleanly side by side (or
//! approaching a broadside with 1.6 m of air left) took ramming damage and thrown tracks.

use std::f32::consts::{FRAC_PI_2, PI};

use game_core::{DamageCause, ModuleSlot, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

#[test]
fn full_speed_side_pass_deals_no_ram_damage() {
    // Two enemy T-55As drive antiparallel with a 4.5 m lateral gap between centers: the hull
    // sides pass 1.0 m apart (half_width 1.75 each) and never touch.
    let mut state = SimulationState::new();
    let a = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 0.0));
    let b = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(4.5, 0.0, 90.0));
    state.tank_mut(b).expect("tank b").yaw_rad = PI;
    let step = FixedTimestep::from_hz(60);

    for _ in 0..600 {
        state.apply_commands(
            &[(a, TankCommand::drive(1.0, 0.0)), (b, TankCommand::drive(1.0, 0.0))],
            step,
        );
        assert!(
            state.damage_events().iter().all(|event| event.cause != DamageCause::Ram),
            "a clean side-by-side pass must never register ramming"
        );
    }

    for id in [a, b] {
        let tank = state.tank(id).expect("tank");
        assert_eq!(tank.hit_points, TankSpec::t54_1951().hit_points, "no phantom hull damage");
        assert!(
            tank.modules.is_functional(ModuleSlot::Suspension),
            "no phantom thrown tracks on a near miss"
        );
    }
    // Sanity: both tanks actually passed each other instead of stalling mid-field.
    assert!(state.tank(a).expect("tank a").position.z > 60.0, "tank a should drive through");
    assert!(state.tank(b).expect("tank b").position.z < 30.0, "tank b should drive through");
}

#[test]
fn tbone_ram_registers_only_when_hulls_actually_touch() {
    // Rammer drives +z into the broadside of a stationary enemy. True side contact sits at
    // half_length + half_width = 3.20 + 1.75 = 4.95 m between centers. The old length-circle
    // predicate fired at a 6.5 m center gap — 1.6 m of air before the hulls met.
    let mut state = SimulationState::new();
    let rammer = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 30.0));
    state.tank_mut(target).expect("target").yaw_rad = FRAC_PI_2;
    let step = FixedTimestep::from_hz(60);

    let mut contact_gap = None;
    for _ in 0..600 {
        state.apply_commands(&[(rammer, TankCommand::drive(1.0, 0.0))], step);
        let gap = state.tank(target).expect("target").position.z
            - state.tank(rammer).expect("rammer").position.z;
        if state.damage_events().iter().any(|event| event.cause == DamageCause::Ram) {
            contact_gap = Some(gap);
            break;
        }
        assert!(gap > 4.9, "no ram may fire while the hulls are still apart (center gap {gap})");
    }

    // Contact + base slop + at most one tick of closing travel (15.28 m/s / 60 Hz ~= 0.26 m).
    let gap = contact_gap.expect("a genuine t-bone must still register ramming damage");
    assert!(gap <= 5.45, "ram must fire at real hull contact, got center gap {gap}");
}

/// Drive one hull bow-first into the broadside of a stationary enemy and report what the ram
/// cost each of them, in hit points. `charger_first` only changes the order the two hulls are
/// spawned in — i.e. their index in the roster — and must change nothing else.
fn tbone_losses(charger_first: bool) -> (u32, u32) {
    let mut state = SimulationState::new();
    let charger_at = Vec3::ZERO;
    let target_at = Vec3::new(0.0, 0.0, 30.0);
    let (charger, target) = if charger_first {
        let charger = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), charger_at);
        (charger, state.spawn_tank(TeamId(2), TankSpec::t54_1951(), target_at))
    } else {
        let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), target_at);
        (state.spawn_tank(TeamId(1), TankSpec::t54_1951(), charger_at), target)
    };
    state.tank_mut(target).expect("target").yaw_rad = FRAC_PI_2;
    let step = FixedTimestep::from_hz(60);

    for _ in 0..600 {
        state.apply_commands(&[(charger, TankCommand::drive(1.0, 0.0))], step);
        if state.damage_events().iter().any(|event| event.cause == DamageCause::Ram) {
            break;
        }
    }
    let full = TankSpec::t54_1951().hit_points;
    (
        full - state.tank(charger).expect("charger").hit_points,
        full - state.tank(target).expect("target").hit_points,
    )
}

/// A ram's outcome is GEOMETRY, not roster order.
///
/// The split used to be positional — `tanks[right]` paid the full bill and `tanks[left]` half —
/// so the very same t-bone charged the stationary defender HALF if it happened to be spawned
/// first, and full if it was spawned second. In a game with no ±25% dice, an outcome that turns
/// on an array index is the least honest thing in the collision.
#[test]
fn a_ram_charges_the_same_bill_whichever_hull_spawned_first() {
    assert_eq!(
        tbone_losses(true),
        tbone_losses(false),
        "the same collision must cost the same, whatever order the roster holds the hulls in"
    );
}

/// ...and the asymmetry that DOES survive is the honest one: the impulse is shared (Newton's
/// third law), but the hull that meets it bow-on braces the whole vehicle behind its thickest,
/// most sloped plate, while the broadsided hull takes the same energy through a thin flank and
/// its running gear.
#[test]
fn a_broadsided_hull_pays_more_than_the_bow_that_charged_it() {
    let (charger_loss, target_loss) = tbone_losses(true);
    assert!(charger_loss > 0, "the charger feels its own ram: {charger_loss} HP");
    assert!(
        target_loss > charger_loss,
        "the broadside must cost more than the bow: target {target_loss} HP vs charger \
         {charger_loss} HP"
    );
}
