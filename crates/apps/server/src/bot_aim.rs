//! Bot gunnery: moving-target intercept, ballistic lay and target-sized trigger gates.
//!
//! Pitch and flight time both come from [`game_core::math::integrate_shell_step`], the exact
//! authoritative shell step. There is no private simplified bot trajectory.

use game_core::math::{gun_direction, integrate_shell_step, wrap_angle};
use glam::Vec3;
use sim::{MAX_GUN_PITCH_RAD, MIN_GUN_PITCH_RAD, SHELL_MAX_AGE_SECONDS, TankState};

/// Match the authoritative server's 60 Hz shell integration.
const SOLVE_DT_S: f32 = 1.0 / 60.0;
/// Fixed-point refinements for drop compensation and linear target intercept.
const SOLVE_ROUNDS: usize = 4;
const INTERCEPT_ROUNDS: usize = 4;
/// A corrupt velocity must not send a cached lay across the map.
const MAX_LEAD_DISPLACEMENT_M: f32 = 120.0;
/// Aim at the middle 80% of the target silhouette, leaving dispersion margin at the rim.
const GATE_MARGIN: f32 = 0.8;

#[derive(Debug, Clone, Copy)]
pub(crate) struct BotAimErrors {
    pub turret_error: f32,
    pub pitch_error: f32,
    pub yaw_gate: f32,
    pub pitch_gate: f32,
}

impl BotAimErrors {
    pub(crate) fn on_target(&self) -> bool {
        self.turret_error.abs() < self.yaw_gate && self.pitch_error.abs() < self.pitch_gate
    }
}

/// Absolute hull-frame lay cached between solves. Target switches invalidate the cache.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BotFiringSolution {
    pub target: game_core::TankId,
    desired_turret_yaw_rad: f32,
    desired_gun_pitch_rad: f32,
    /// Future hull centre shared by the gun lay and the trigger-safety line.
    aim_point_world: Vec3,
    yaw_gate: f32,
    pitch_gate: f32,
}

impl BotFiringSolution {
    pub(crate) fn errors(&self, tank: &TankState) -> BotAimErrors {
        BotAimErrors {
            turret_error: wrap_angle(self.desired_turret_yaw_rad - tank.turret_yaw_rad),
            pitch_error: self.desired_gun_pitch_rad - tank.gun_pitch_rad,
            yaw_gate: self.yaw_gate,
            pitch_gate: self.pitch_gate,
        }
    }

    pub(crate) fn aim_point_world(&self) -> Vec3 {
        self.aim_point_world
    }
}

/// Solve a constant-velocity intercept, then pull the world lay through the hull attitude.
pub(crate) fn solve_firing_solution(tank: &TankState, target: &TankState) -> BotFiringSolution {
    let muzzle = tank.muzzle_world_position();
    let target_center = target.position + Vec3::Y * target.spec.hitbox.center_y_m;
    let shell = tank.selected_shell();
    let (aim_point, world_pitch) = intercept_lay(
        muzzle,
        target_center,
        target.velocity_mps,
        shell.muzzle_velocity_mps,
        shell.drag_per_s(),
    );
    let delta = aim_point - muzzle;
    let world_yaw = delta.x.atan2(delta.z);
    let local = tank.hull_pose().basis().transpose() * gun_direction(world_yaw, world_pitch);
    let desired_turret = local.x.atan2(local.z);
    let desired_pitch = local.y.clamp(-1.0, 1.0).asin().clamp(MIN_GUN_PITCH_RAD, MAX_GUN_PITCH_RAD);
    let distance = delta.length().max(1.0);
    BotFiringSolution {
        target: target.id,
        desired_turret_yaw_rad: desired_turret,
        desired_gun_pitch_rad: desired_pitch,
        aim_point_world: aim_point,
        yaw_gate: (target.spec.hitbox.half_width_m * GATE_MARGIN / distance).atan(),
        pitch_gate: (target.spec.hitbox.half_height_m * GATE_MARGIN / distance).atan(),
    }
}

/// Refine the future point from the real flight time. Unreachable or corrupt input falls back
/// to the stationary lay so the bot keeps closing instead of aiming at an unbounded point.
fn intercept_lay(
    muzzle: Vec3,
    target_center: Vec3,
    target_velocity: Vec3,
    muzzle_velocity_mps: f32,
    drag_per_s: f32,
) -> (Vec3, f32) {
    let stationary = lay_to_point(muzzle, target_center, muzzle_velocity_mps, drag_per_s);
    if !target_velocity.is_finite() || target_velocity.length_squared() <= f32::EPSILON {
        return (target_center, stationary.pitch);
    }
    let mut point = target_center;
    for _ in 0..INTERCEPT_ROUNDS {
        let Some(lay) = ballistic_lay_to_point(muzzle, point, muzzle_velocity_mps, drag_per_s)
        else {
            return (target_center, stationary.pitch);
        };
        let displacement = target_velocity * lay.flight_time_s;
        if !displacement.is_finite() || displacement.length() > MAX_LEAD_DISPLACEMENT_M {
            return (target_center, stationary.pitch);
        }
        point = target_center + displacement;
    }
    ballistic_lay_to_point(muzzle, point, muzzle_velocity_mps, drag_per_s)
        .map_or((target_center, stationary.pitch), |lay| (point, lay.pitch))
}

#[derive(Debug, Clone, Copy)]
struct BallisticLay {
    pitch: f32,
    flight_time_s: f32,
}

fn lay_to_point(
    muzzle: Vec3,
    point: Vec3,
    muzzle_velocity_mps: f32,
    drag_per_s: f32,
) -> BallisticLay {
    let delta = point - muzzle;
    let flat = Vec3::new(delta.x, 0.0, delta.z).length().max(1.0);
    ballistic_lay(muzzle_velocity_mps, drag_per_s, flat, delta.y)
        .unwrap_or(BallisticLay { pitch: (delta.y / flat).atan(), flight_time_s: 0.0 })
}

fn ballistic_lay_to_point(
    muzzle: Vec3,
    point: Vec3,
    muzzle_velocity_mps: f32,
    drag_per_s: f32,
) -> Option<BallisticLay> {
    let delta = point - muzzle;
    let flat = Vec3::new(delta.x, 0.0, delta.z).length().max(1.0);
    ballistic_lay(muzzle_velocity_mps, drag_per_s, flat, delta.y)
}

/// Refine pitch from measured vertical miss and return measured time at the final crossing.
fn ballistic_lay(
    muzzle_velocity_mps: f32,
    drag_per_s: f32,
    flat_m: f32,
    rise_m: f32,
) -> Option<BallisticLay> {
    let mut pitch = (rise_m / flat_m).atan();
    for _ in 0..SOLVE_ROUNDS {
        let sample = arc_at_range(muzzle_velocity_mps, drag_per_s, pitch, flat_m)?;
        pitch += ((rise_m - sample.height_m) / flat_m).atan();
    }
    let sample = arc_at_range(muzzle_velocity_mps, drag_per_s, pitch, flat_m)?;
    Some(BallisticLay { pitch, flight_time_s: sample.flight_time_s })
}

fn arc_at_range(
    muzzle_velocity_mps: f32,
    drag_per_s: f32,
    pitch: f32,
    flat_m: f32,
) -> Option<ArcSample> {
    let mut position = Vec3::ZERO;
    let mut velocity = Vec3::new(pitch.cos(), pitch.sin(), 0.0) * muzzle_velocity_mps;
    let mut age = 0.0;
    while age < SHELL_MAX_AGE_SECONDS {
        let previous = position;
        integrate_shell_step(&mut velocity, drag_per_s, SOLVE_DT_S);
        position += velocity * SOLVE_DT_S;
        age += SOLVE_DT_S;
        if position.x >= flat_m {
            let t = (flat_m - previous.x) / (position.x - previous.x).max(1.0e-6);
            return Some(ArcSample {
                height_m: previous.y + (position.y - previous.y) * t,
                flight_time_s: age - SOLVE_DT_S + SOLVE_DT_S * t,
            });
        }
    }
    None
}

struct ArcSample {
    height_m: f32,
    flight_time_s: f32,
}

#[cfg(test)]
#[path = "bot_aim_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "bot_aim_intercept_tests.rs"]
mod intercept_tests;
