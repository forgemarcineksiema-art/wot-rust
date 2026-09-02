//! Authoritative hull attitude: the sprung hull (Inny Poziom G7). Pitch and roll are a
//! spring-damper riding the support plane the running gear stands on — the semi-implicit
//! arithmetic the presentation spring ran for a year, now inside the tick, so the gun, the
//! armour and every client see the same tonnes settling. Integrated in f32 at the fixed tick,
//! which keeps determinism and replay-exactness; the rest is an exact fixed point (a hull within
//! a hair of its target is snapped to it, as friction does), so a terrain-free replay stays
//! bit-exact level. Weight transfer falls out of the spring's own inputs — the drive's
//! longitudinal acceleration dives the nose under braking through the centre of mass and the
//! wheelbase, the turn leans the hull through its gauge — no theatre layer and no knob: every
//! number comes from the vehicle (`HullSpring::for_spec`).

use crate::movement::TankKinematicState;
use serde::{Deserialize, Serialize};

/// Hardest tilt the attitude may reach (~34°): covers the steady gradeability climb and most of the
/// momentum-climb band, so a hull on a steep face reads as steep instead of saturating early. A
/// clamp still keeps a degenerate terrain sample from flipping the hull.
pub const MAX_HULL_TILT_RAD: f32 = 0.6;

/// Under this — radians off the target and radians per second, together — the hull is at rest
/// on its target: the spring is snapped onto it exactly, so the rest is a fixed point of the
/// integrator (a hull that was level stays bit-exact level, a disturbed one returns to it) and
/// a settled hull never carries sub-ulp ringing into the replay hash.
pub const ATTITUDE_REST_EPSILON: f32 = 1.0e-5;

/// The most weight transfer may add to the support plane's target, per axis (~3.4°): a ram at
/// ten metres per second squared is a jolt in the body, never a flip.
pub const MAX_WEIGHT_TRANSFER_RAD: f32 = 0.06;

/// The hull's springs — derived from the vehicle, never authored as a knob: the natural
/// frequency from the static deflection of the suspension (`ω = √(g / sag)`, the one law every
/// sprung mass obeys — mass cancels, as it does in steel sized for its load), the damping from
/// the suspension family, and the two weight-transfer gains from the centre of mass, the
/// wheelbase and the gauge.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HullSpring {
    pub omega_rad_s: f32,
    pub zeta: f32,
    /// Radians of pitch per m/s² of longitudinal acceleration (nose up with acceleration, down
    /// under braking).
    pub dive_rad_per_mps2: f32,
    /// Radians of roll per m/s² of lateral (centripetal) acceleration (right side up in a right
    /// turn — the body leans out of the turn).
    pub lean_rad_per_mps2: f32,
}

impl HullSpring {
    /// A stiff, well-damped stand-in for callers without a vehicle (harness fixtures): settles
    /// inside a few ticks and transfers no weight.
    pub const STIFF: Self =
        Self { omega_rad_s: 12.0, zeta: 0.7, dive_rad_per_mps2: 0.0, lean_rad_per_mps2: 0.0 };
}

impl Default for HullSpring {
    fn default() -> Self {
        Self::STIFF
    }
}

/// Advance the attitude one grounded tick on its springs toward the support-plane targets, with
/// the tick's accelerations (hull frame: `accel_long_mps2` forward positive, `accel_lat_mps2`
/// right positive) transferring weight into the targets. Airborne hulls skip this — flight
/// freezes the attitude the hull left the ground with.
pub fn advance_hull_attitude(
    state: &mut TankKinematicState,
    target_pitch_rad: f32,
    target_roll_rad: f32,
    accel_long_mps2: f32,
    accel_lat_mps2: f32,
    spring: &HullSpring,
    dt: f32,
) {
    let dive = (spring.dive_rad_per_mps2 * accel_long_mps2)
        .clamp(-MAX_WEIGHT_TRANSFER_RAD, MAX_WEIGHT_TRANSFER_RAD);
    let lean = (spring.lean_rad_per_mps2 * accel_lat_mps2)
        .clamp(-MAX_WEIGHT_TRANSFER_RAD, MAX_WEIGHT_TRANSFER_RAD);
    let pitch_target = (target_pitch_rad + dive).clamp(-MAX_HULL_TILT_RAD, MAX_HULL_TILT_RAD);
    let roll_target = (target_roll_rad + lean).clamp(-MAX_HULL_TILT_RAD, MAX_HULL_TILT_RAD);
    spring_axis(&mut state.pitch_rad, &mut state.pitch_vel_rad_s, pitch_target, spring, dt);
    spring_axis(&mut state.roll_rad, &mut state.roll_vel_rad_s, roll_target, spring, dt);
}

/// One semi-implicit spring-damper step on one axis: `x'' = ω²(target − x) − 2ζω x'`, the
/// velocity first and the position on the new velocity, which is what keeps the step stable at
/// the 20 Hz the drive replays run at (ω·dt stays well under 2). The tilt clamp kills the
/// velocity into it, so a clamped hull does not store a push it never made.
fn spring_axis(x: &mut f32, vel: &mut f32, target: f32, spring: &HullSpring, dt: f32) {
    let (omega, zeta) = (spring.omega_rad_s, spring.zeta);
    *vel += (omega * omega * (target - *x) - 2.0 * zeta * omega * *vel) * dt;
    *x += *vel * dt;
    if *x >= MAX_HULL_TILT_RAD {
        *x = MAX_HULL_TILT_RAD;
        *vel = vel.min(0.0);
    } else if *x <= -MAX_HULL_TILT_RAD {
        *x = -MAX_HULL_TILT_RAD;
        *vel = vel.max(0.0);
    }
    if (*x - target).abs() < ATTITUDE_REST_EPSILON && vel.abs() < ATTITUDE_REST_EPSILON {
        *x = target;
        *vel = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn spring() -> HullSpring {
        HullSpring {
            omega_rad_s: 9.0,
            zeta: 0.4,
            dive_rad_per_mps2: 0.004,
            lean_rad_per_mps2: 0.003,
        }
    }

    /// The spring settles onto its target with ONE visible overshoot (underdamped, as torsion
    /// bars with shock absorbers on a few stations are) and comes to an exact rest on it.
    #[test]
    fn the_hull_settles_onto_the_plane_with_one_nod_and_rests_exactly_on_it() {
        let mut state = TankKinematicState::default();
        let (mut peak, mut trough) = (0.0_f32, f32::INFINITY);
        for _ in 0..240 {
            advance_hull_attitude(&mut state, 0.2, 0.0, 0.0, 0.0, &spring(), DT);
            if state.pitch_rad > peak {
                peak = state.pitch_rad;
            } else if peak > 0.2 {
                trough = trough.min(state.pitch_rad);
            }
        }
        assert!(peak > 0.2 && peak < 0.26, "one nod past the plane, not a wobble: {peak}");
        assert!(trough > 0.18, "the swing back stays under a tenth of the nod: {trough}");
        assert_eq!(state.pitch_rad.to_bits(), 0.2_f32.to_bits(), "an exact rest on the target");
        assert_eq!(state.pitch_vel_rad_s, 0.0);
    }

    /// A level hull at rest is a fixed point to the bit, at the tick and at the replay's 20 Hz;
    /// a hull tilted by hand walks back to that exact rest.
    #[test]
    fn a_level_rest_is_bit_exact_and_a_tilt_returns_to_it() {
        for dt in [1.0 / 60.0, 1.0 / 20.0] {
            let mut state = TankKinematicState::default();
            for _ in 0..300 {
                advance_hull_attitude(&mut state, 0.0, 0.0, 0.0, 0.0, &spring(), dt);
            }
            assert_eq!(state.pitch_rad.to_bits(), 0.0_f32.to_bits());
            assert_eq!(state.roll_rad.to_bits(), 0.0_f32.to_bits());
            let mut tilted = TankKinematicState { pitch_rad: -0.2, roll_rad: 0.15, ..state };
            for _ in 0..600 {
                advance_hull_attitude(&mut tilted, 0.0, 0.0, 0.0, 0.0, &spring(), dt);
            }
            assert_eq!(tilted.pitch_rad.to_bits(), 0.0_f32.to_bits(), "back to exact level");
            assert_eq!(tilted.roll_rad.to_bits(), 0.0_f32.to_bits());
        }
    }

    /// Weight transfer: braking dives the nose, accelerating lifts it, a right turn leans the
    /// hull right side up, and none of it can exceed the transfer cap.
    #[test]
    fn braking_dives_the_nose_and_a_right_turn_leans_the_hull_out_of_the_turn() {
        let settle = |long: f32, lat: f32| {
            let mut state = TankKinematicState::default();
            for _ in 0..300 {
                advance_hull_attitude(&mut state, 0.0, 0.0, long, lat, &spring(), DT);
            }
            (state.pitch_rad, state.roll_rad)
        };
        let (braking, _) = settle(-4.8, 0.0);
        let (launching, _) = settle(3.0, 0.0);
        let (_, right_turn) = settle(0.0, 2.5);
        assert!(braking < -0.01, "braking dives the nose: {braking}");
        assert!(launching > 0.005, "launching lifts it: {launching}");
        assert!(right_turn > 0.005, "a right turn leans the hull right side up: {right_turn}");
        let (rammed, _) = settle(-40.0, 0.0);
        assert!(rammed >= -MAX_WEIGHT_TRANSFER_RAD - 1.0e-6, "a ram is a jolt, never a flip");
    }

    /// The tilt clamp holds and does not store a push: a hull driven into the clamp rests on it
    /// with no velocity, and comes off it cleanly when the target returns.
    #[test]
    fn the_tilt_clamp_holds_without_storing_a_push() {
        let mut state = TankKinematicState::default();
        for _ in 0..300 {
            advance_hull_attitude(&mut state, 1.0, -1.0, 0.0, 0.0, &spring(), DT);
        }
        assert!((state.pitch_rad - MAX_HULL_TILT_RAD).abs() < 1.0e-5);
        assert!((state.roll_rad + MAX_HULL_TILT_RAD).abs() < 1.0e-5);
        assert!(state.pitch_vel_rad_s <= 0.0 && state.roll_vel_rad_s >= 0.0);
        for _ in 0..300 {
            advance_hull_attitude(&mut state, 0.0, 0.0, 0.0, 0.0, &spring(), DT);
        }
        assert_eq!(state.pitch_rad.to_bits(), 0.0_f32.to_bits());
    }
}
