//! Authoritative hull attitude: pitch/roll advanced kinematically toward the support-plane
//! target. Deliberately spring-free — a rate-limited approach carries no oscillation state, so
//! it is deterministic, replay-stable, and identical on the server and the client predictor.
//! Weight-transfer theatrics (brake dive, turn lean) stay a client-side presentation layer on
//! top (see `docs/vehicle-movement-policy.md`, "Hull Attitude and the Support Envelope").

use crate::movement::TankKinematicState;

/// Hardest tilt the attitude may reach (~34°): covers the steady gradeability climb and most of the
/// momentum-climb band, so a hull on a steep face reads as steep instead of saturating early. A
/// clamp still keeps a degenerate terrain sample from flipping the hull.
pub const MAX_HULL_TILT_RAD: f32 = 0.6;

/// How fast the hull rotates onto a new support plane (rad/s). Fast enough to settle onto a
/// slope within ~0.4 s of full tilt, slow enough that a heightmap step reads as the bow coming
/// down, not a snap.
pub const HULL_ATTITUDE_RATE_RAD_S: f32 = 1.4;

/// Advance the attitude one grounded tick toward the support-plane targets. Airborne hulls skip
/// this — flight freezes the attitude the hull left the ground with.
pub fn advance_hull_attitude(
    state: &mut TankKinematicState,
    target_pitch_rad: f32,
    target_roll_rad: f32,
    dt: f32,
) {
    let step = HULL_ATTITUDE_RATE_RAD_S * dt;
    let pitch_target = target_pitch_rad.clamp(-MAX_HULL_TILT_RAD, MAX_HULL_TILT_RAD);
    let roll_target = target_roll_rad.clamp(-MAX_HULL_TILT_RAD, MAX_HULL_TILT_RAD);
    state.pitch_rad += (pitch_target - state.pitch_rad).clamp(-step, step);
    state.roll_rad += (roll_target - state.roll_rad).clamp(-step, step);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attitude_walks_to_the_target_at_the_rate_limit_and_clamps_extremes() {
        let mut state = TankKinematicState::default();
        let dt = 1.0 / 60.0;
        advance_hull_attitude(&mut state, 1.0, -1.0, dt);
        let step = HULL_ATTITUDE_RATE_RAD_S * dt;
        assert!((state.pitch_rad - step).abs() < 1.0e-6, "one rate-limited step");
        assert!((state.roll_rad + step).abs() < 1.0e-6);
        for _ in 0..120 {
            advance_hull_attitude(&mut state, 1.0, -1.0, dt);
        }
        assert!((state.pitch_rad - MAX_HULL_TILT_RAD).abs() < 1.0e-5, "clamped at max tilt");
        assert!((state.roll_rad + MAX_HULL_TILT_RAD).abs() < 1.0e-5);
    }
}
