use game_core::TankSpec;
use game_core::math::wrap_angle;

use crate::TankCommand;

/// The fleet's old shared arc, kept ONLY as the fallback a caller uses when it has no spec in
/// hand (an editor probe, a fixture). Every gameplay path reads
/// [`TankSpec::gun_pitch_limits_rad`] — the arc is a per-vehicle property now, and a tank whose
/// documented depression is -5 must not aim like one built for -8.
pub const MIN_GUN_PITCH_RAD: f32 = -0.14;
pub const MAX_GUN_PITCH_RAD: f32 = 0.35;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct AimingState {
    pub turret_yaw_rad: f32,
    pub turret_yaw_velocity_rad_s: f32,
    pub gun_pitch_rad: f32,
}

/// Advance turret yaw and gun pitch one tick. `hull_pitch_delta_rad` is how much the hull
/// pitched THIS tick (the drive step measures it around the attitude advance); a vehicle with a
/// vertical stabilizer cancels that share of it so the gun holds its world elevation.
///
/// Inny Poziom A12: the elevation rate is the gun's own (`GunSpec::elevation_rate_rad_s`) — the
/// fleet shared one constant, 0.5 rad/s, slower than every hull's pitch rate — and the
/// stabilizer is a vehicle property with a historical answer (the Centurion Mk 3 carried one,
/// the wartime hulls and the T-54 obr. 1951 did not). Both are pure data, so the server and
/// the client predictor step the same function and stay bit-identical without a wire change.
pub fn step_aiming(
    aiming: &mut AimingState,
    spec: &TankSpec,
    command: TankCommand,
    dt_seconds: f32,
    hull_pitch_delta_rad: f32,
) {
    let command = command.clamped();

    if spec.has_fixed_casemate() {
        aiming.turret_yaw_rad = 0.0;
        aiming.turret_yaw_velocity_rad_s = 0.0;
    } else {
        let target_velocity = command.turret_yaw_delta * spec.turret_rotation_rad_s;
        aiming.turret_yaw_velocity_rad_s = target_velocity;
        // Wrap into (-PI, PI] so a long session of one-way traverse cannot grow the angle
        // without bound and erode f32 sin/cos precision (the camera already wraps; snapshot
        // interpolation is shortest-arc, so the seam is invisible to rendering).
        aiming.turret_yaw_rad =
            wrap_angle(aiming.turret_yaw_rad + aiming.turret_yaw_velocity_rad_s * dt_seconds);
    }
    aiming.turret_yaw_rad = spec.effective_turret_yaw_rad(aiming.turret_yaw_rad);
    let (min_pitch, max_pitch) = spec.gun_pitch_limits_rad();
    // The stabilizer acts first — a gyro-driven mount, not a gunner's reaction — so the
    // gunner's own command rides on the held gun; the arc clamp bounds both, because no
    // stabilizer depresses a breech through the turret roof.
    let stabilized = spec.vertical_stabilizer.clamp(0.0, 1.0) * hull_pitch_delta_rad;
    aiming.gun_pitch_rad = (aiming.gun_pitch_rad - stabilized
        + command.gun_pitch_delta * spec.gun.elevation_rate_rad_s * dt_seconds)
        .clamp(min_pitch, max_pitch);
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use super::*;

    #[test]
    fn continuous_traverse_keeps_turret_yaw_wrapped() {
        let spec = TankSpec::t54_1951();
        let mut aiming = AimingState::default();
        let command = TankCommand { turret_yaw_delta: 1.0, ..TankCommand::idle() };

        // Ten minutes of full one-way traverse (~275 rad unwrapped at 0.46 rad/s).
        for _ in 0..36_000 {
            step_aiming(&mut aiming, &spec, command, 1.0 / 60.0, 0.0);
            assert!(
                aiming.turret_yaw_rad > -PI - 1.0e-5 && aiming.turret_yaw_rad <= PI + 1.0e-5,
                "turret yaw must stay wrapped, got {}",
                aiming.turret_yaw_rad
            );
        }
    }

    /// Inny Poziom A12: a stabilized gun holds its WORLD elevation when the hull pitches —
    /// the hull-relative pitch moves by exactly minus the hull's change — while an
    /// unstabilized gun rides the hull 1:1, and no stabilizer leaves the arc.
    #[test]
    fn a_stabilized_gun_holds_its_world_elevation_and_an_unstabilized_one_rides_the_hull() {
        let dt = 1.0 / 60.0;
        let hull_pitch_step = 0.05; // ~2.9° in one tick: a bow coming down on a furrow

        let mut stabilized = TankSpec::t54_1951();
        stabilized.vertical_stabilizer = 1.0;
        let mut held = AimingState { gun_pitch_rad: 0.10, ..AimingState::default() };
        step_aiming(&mut held, &stabilized, TankCommand::idle(), dt, hull_pitch_step);
        let world_before = 0.0 + 0.10;
        let world_after = hull_pitch_step + held.gun_pitch_rad;
        assert!(
            (world_after - world_before).abs() < 1.0e-6,
            "stabilized: world elevation moved {world_before} -> {world_after}"
        );

        let riding = TankSpec::t54_1951();
        assert_eq!(riding.vertical_stabilizer, 0.0, "the obr. 1951 carries no STP-1");
        let mut rode = AimingState { gun_pitch_rad: 0.10, ..AimingState::default() };
        step_aiming(&mut rode, &riding, TankCommand::idle(), dt, hull_pitch_step);
        assert_eq!(rode.gun_pitch_rad, 0.10, "unstabilized: the hull-relative gun did not move");
        assert!(
            ((hull_pitch_step + rode.gun_pitch_rad) - world_before - hull_pitch_step).abs()
                < 1.0e-6,
            "unstabilized: the world elevation rode the hull 1:1"
        );

        // The arc still binds: a hull nosing up hard asks the stabilizer to depress past the
        // gun's -5° and gets the limit instead.
        let (min_pitch, _) = stabilized.gun_pitch_limits_rad();
        let mut at_limit =
            AimingState { gun_pitch_rad: min_pitch + 0.01, ..AimingState::default() };
        step_aiming(&mut at_limit, &stabilized, TankCommand::idle(), dt, 0.2);
        assert_eq!(at_limit.gun_pitch_rad, min_pitch, "no stabilizer depresses through the roof");
    }

    /// Inny Poziom A12: the gun elevates at ITS OWN rate — the gunner's command on a T-54 moves
    /// the D-10 by `elevation_rate × dt`, not by the old fleet constant.
    #[test]
    fn the_gun_elevates_at_its_own_rate() {
        let spec = TankSpec::t54_1951();
        assert!(spec.gun.elevation_rate_rad_s > 0.5, "the D-10 is faster than the old constant");
        let mut aiming = AimingState::default();
        let command = TankCommand { gun_pitch_delta: 1.0, ..TankCommand::idle() };
        step_aiming(&mut aiming, &spec, command, 1.0 / 60.0, 0.0);
        assert!((aiming.gun_pitch_rad - spec.gun.elevation_rate_rad_s / 60.0).abs() < 1.0e-6);
    }
}
