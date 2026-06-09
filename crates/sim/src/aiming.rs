use game_core::TankSpec;

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
        aiming.turret_yaw_rad += aiming.turret_yaw_velocity_rad_s * dt_seconds;
    }
    aiming.turret_yaw_rad = spec.effective_turret_yaw_rad(aiming.turret_yaw_rad);
    aiming.gun_pitch_rad = (aiming.gun_pitch_rad
        + command.gun_pitch_delta * GUN_ELEVATION_RATE_RAD_S * dt_seconds)
        .clamp(MIN_GUN_PITCH_RAD, MAX_GUN_PITCH_RAD);
}
