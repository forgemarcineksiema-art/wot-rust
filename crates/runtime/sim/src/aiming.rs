use game_core::TankSpec;
use game_core::math::wrap_angle;

use crate::TankCommand;

pub const GUN_ELEVATION_RATE_RAD_S: f32 = 0.5;
pub const MIN_GUN_PITCH_RAD: f32 = -0.14;
pub const MAX_GUN_PITCH_RAD: f32 = 0.35;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct AimingState {
    pub turret_yaw_rad: f32,
    pub turret_yaw_velocity_rad_s: f32,
    pub gun_pitch_rad: f32,
}

pub fn step_aiming(
    aiming: &mut AimingState,
    spec: &TankSpec,
    command: TankCommand,
    dt_seconds: f32,
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
    aiming.gun_pitch_rad = (aiming.gun_pitch_rad
        + command.gun_pitch_delta * GUN_ELEVATION_RATE_RAD_S * dt_seconds)
        .clamp(MIN_GUN_PITCH_RAD, MAX_GUN_PITCH_RAD);
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
            step_aiming(&mut aiming, &spec, command, 1.0 / 60.0);
            assert!(
                aiming.turret_yaw_rad > -PI - 1.0e-5 && aiming.turret_yaw_rad <= PI + 1.0e-5,
                "turret yaw must stay wrapped, got {}",
                aiming.turret_yaw_rad
            );
        }
    }
}
