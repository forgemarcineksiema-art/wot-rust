//! HULLS THAT MEET, TOUCH — P1.2 of `docs/contact-and-tracks-program.md`.
//!
//! The opening audit measured two T-54s under full throttle coming to rest with **0.1217 m** of air
//! between their collision boxes — `CONTACT_SKIN_M` to the millimetre — which with the fleet's
//! phantom hitbox margins put 0.40 m of daylight between two tanks that were supposed to be leaning
//! on each other. The detection range had become a parking distance.
//!
//! A speculative contact says how fast a pair may close rather than whether they may close at all,
//! so the gap they are allowed to shut is exactly the gap that is there. These are the numbers that
//! has to keep producing: hulls meet, hulls stay met, and nothing gets through anything.

use game_core::{TankId, TankSpec, VehicleKind};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

fn step() -> FixedTimestep {
    FixedTimestep::from_hz(60)
}

/// What the old skin held two hulls apart by. Nothing may quietly grow back toward it.
const OLD_STANDOFF_M: f32 = 0.1217;

/// Hulls that drive into each other must end up in contact, not parked a margin short of it.
#[test]
fn two_hulls_driving_together_come_to_rest_touching() {
    let spec = TankSpec::t54_1951();
    let half_len = spec.hitbox.half_length_m;
    let mut state = SimulationState::new();
    let a = state.spawn_tank_with_yaw(game_core::TeamId(1), spec.clone(), Vec3::ZERO, 0.0);
    let b = state.spawn_tank_with_yaw(
        game_core::TeamId(2),
        spec,
        Vec3::new(0.0, 0.0, 4.0 * half_len + 6.0),
        std::f32::consts::PI,
    );
    let go = [(a, TankCommand::drive(1.0, 0.0)), (b, TankCommand::drive(1.0, 0.0))];
    for _ in 0..1_200 {
        state.apply_commands(&go, step());
    }

    let gap = gap_between(&state, a, b) - 2.0 * half_len;
    println!("head-on resting gap: {gap:.4} m (was {OLD_STANDOFF_M} m)");
    assert!(
        gap.abs() <= 0.03,
        "two hulls pressing together must finish in contact, not {gap:.4} m apart"
    );
}

/// ...and stay there. A contact that has to be re-won every tick reads as a shake, whatever the
/// average position says.
#[test]
fn a_pressed_contact_holds_still() {
    let spec = TankSpec::t54_1951();
    let half_len = spec.hitbox.half_length_m;
    let mut state = SimulationState::new();
    let a = state.spawn_tank_with_yaw(game_core::TeamId(1), spec.clone(), Vec3::ZERO, 0.0);
    let b = state.spawn_tank_with_yaw(
        game_core::TeamId(2),
        spec,
        Vec3::new(0.0, 0.0, 4.0 * half_len + 6.0),
        std::f32::consts::PI,
    );
    let go = [(a, TankCommand::drive(1.0, 0.0)), (b, TankCommand::drive(1.0, 0.0))];
    for _ in 0..1_200 {
        state.apply_commands(&go, step());
    }

    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    let mut worst_step = 0.0_f32;
    let mut previous = position(&state, a);
    for _ in 0..180 {
        state.apply_commands(&go, step());
        let gap = gap_between(&state, a, b) - 2.0 * half_len;
        lo = lo.min(gap);
        hi = hi.max(gap);
        let now = position(&state, a);
        worst_step = worst_step.max((now - previous).length());
        previous = now;
    }
    println!("pressed gap over 3 s: {lo:.4}..{hi:.4} m, worst step {worst_step:.5} m/tick");
    assert!(hi - lo <= 0.01, "the contact breathed by {:.4} m while pressed", hi - lo);
    assert!(worst_step <= 0.005, "a pressed hull crept {worst_step:.5} m in a tick");
}

/// A charge must be stopped by the hull in front of it, not by luck. The speculative margin carries
/// the ground a pair can cover in one tick precisely so a fast approach is caught while there is
/// still room to catch it.
#[test]
fn a_charge_at_full_speed_does_not_get_through() {
    for kind in [VehicleKind::T54_1951, VehicleKind::T34_85] {
        let spec = kind.spec();
        let half_len = spec.hitbox.half_length_m;
        let mut state = SimulationState::new();
        let charger =
            state.spawn_tank_with_yaw(game_core::TeamId(1), spec.clone(), Vec3::ZERO, 0.0);
        let parked =
            state.spawn_tank_with_yaw(game_core::TeamId(2), spec, Vec3::new(0.0, 0.0, 90.0), 0.0);
        let go = [(charger, TankCommand::drive(1.0, 0.0)), (parked, TankCommand::drive(0.0, 0.0))];
        let mut deepest = f32::INFINITY;
        for _ in 0..900 {
            state.apply_commands(&go, step());
            deepest = deepest.min(gap_between(&state, charger, parked) - 2.0 * half_len);
            assert!(
                position(&state, charger).z < position(&state, parked).z,
                "{kind:?} drove through the hull in front of it"
            );
        }
        println!("{kind:?} charge: deepest interpenetration {:.4} m", -deepest.min(0.0));
        assert!(deepest > -0.10, "{kind:?} buried itself {:.3} m into the hull it hit", -deepest);
    }
}

fn position(state: &SimulationState, id: TankId) -> Vec3 {
    state.tank(id).expect("tank").position
}

fn gap_between(state: &SimulationState, a: TankId, b: TankId) -> f32 {
    (position(state, b).z - position(state, a).z).abs()
}
