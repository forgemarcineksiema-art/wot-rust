use game_core::math::horizontal_forward;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use terrain::HeightMap;

use crate::controller_settings::TankControllerSettings;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TankControlInput {
    pub throttle: f32,
    pub steer: f32,
    pub brake: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TankKinematicState {
    pub position: Vec3,
    pub yaw_rad: f32,
    pub forward_speed_mps: f32,
}

impl Default for TankKinematicState {
    fn default() -> Self {
        Self { position: Vec3::ZERO, yaw_rad: 0.0, forward_speed_mps: 0.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TerrainContact {
    pub height_m: f32,
    pub forward_slope: f32,
    pub side_slope: f32,
    pub roughness: f32,
    pub traction: f32,
}

impl TerrainContact {
    pub fn flat(height_m: f32) -> Self {
        Self { height_m, forward_slope: 0.0, side_slope: 0.0, roughness: 0.0, traction: 1.0 }
    }
}

pub fn step_custom_tank_controller(
    state: &mut TankKinematicState,
    input: TankControlInput,
    settings: &TankControllerSettings,
    dt_seconds: f32,
) {
    step_custom_tank_controller_on_contact(
        state,
        input,
        settings,
        TerrainContact::flat(state.position.y),
        dt_seconds,
    );
}

pub fn step_custom_tank_controller_on_contact(
    state: &mut TankKinematicState,
    input: TankControlInput,
    settings: &TankControllerSettings,
    contact: TerrainContact,
    dt_seconds: f32,
) {
    let throttle = input.throttle.clamp(-1.0, 1.0);
    let steer = input.steer.clamp(-1.0, 1.0);
    let brake = input.brake.clamp(0.0, 1.0);
    let max_speed = if throttle >= 0.0 {
        settings.max_forward_speed_mps
    } else {
        settings.max_reverse_speed_mps
    };
    let terrain_speed_scale =
        (1.0 - contact.forward_slope.abs() * 0.55 - contact.roughness * 0.35).clamp(0.35, 1.0);
    let motion_sign =
        if throttle.abs() > 0.01 { throttle.signum() } else { state.forward_speed_mps.signum() };
    let uphill_grade = (contact.forward_slope * motion_sign).max(0.0);
    // A slope past the tank's gradeability collapses climb speed to zero, so steep faces
    // (such as the railway embankment) become real barriers rather than slow climbs.
    let grade_limit = settings.max_climb_grade.max(0.05);
    let grade_headroom = ((grade_limit - uphill_grade) / grade_limit).clamp(0.0, 1.0);
    if uphill_grade >= grade_limit && state.forward_speed_mps * motion_sign > 0.0 {
        state.forward_speed_mps = 0.0;
    }
    let uphill_scale = (1.0 - uphill_grade * 1.5).clamp(0.25, 1.0) * grade_headroom;
    let target_speed =
        if brake > 0.0 { 0.0 } else { throttle * max_speed * terrain_speed_scale * uphill_scale };
    let accel = acceleration_for_input(settings, contact, brake, throttle, uphill_grade);

    state.forward_speed_mps =
        move_towards(state.forward_speed_mps, target_speed, accel * dt_seconds);

    let speed_factor =
        (state.forward_speed_mps.abs() / settings.max_forward_speed_mps.max(0.01)).clamp(0.18, 1.0);
    let turn_grip = (contact.traction - contact.roughness * 0.2).clamp(0.25, 1.0);
    let steering_direction = if state.forward_speed_mps.abs() > 0.01 {
        state.forward_speed_mps.signum()
    } else if throttle.abs() > 0.01 {
        throttle.signum()
    } else {
        1.0
    };
    state.yaw_rad += steer
        * steering_direction
        * settings.turn_rate_rad_s
        * speed_factor
        * turn_grip
        * dt_seconds;

    let forward = horizontal_forward(state.yaw_rad);
    state.position += forward * state.forward_speed_mps * dt_seconds;
    state.position.y = contact.height_m;
}

pub fn sample_tank_terrain_contact(
    heightmap: &HeightMap,
    position: Vec3,
    yaw_rad: f32,
    probe_length_m: f32,
) -> Option<TerrainContact> {
    let probe = probe_length_m.max(heightmap.cell_size_m() * 0.5).max(0.5);
    let forward = horizontal_forward(yaw_rad);
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    let center = heightmap.sample_height(position.x, position.z)?;
    let front = sample_offset(heightmap, position, forward, probe).unwrap_or(center);
    let back = sample_offset(heightmap, position, -forward, probe).unwrap_or(center);
    let right_h = sample_offset(heightmap, position, right, probe).unwrap_or(center);
    let left_h = sample_offset(heightmap, position, -right, probe).unwrap_or(center);
    let forward_slope = (front - back) / (probe * 2.0);
    let side_slope = (right_h - left_h) / (probe * 2.0);
    let roughness = [front, back, right_h, left_h]
        .into_iter()
        .map(|height| (height - center).abs() / probe)
        .fold(0.0, f32::max)
        .clamp(0.0, 1.0);
    let traction = (1.0 - roughness * 0.45 - side_slope.abs() * 0.35 - forward_slope.abs() * 0.15)
        .clamp(0.35, 1.0);

    Some(TerrainContact { height_m: center, forward_slope, side_slope, roughness, traction })
}

fn acceleration_for_input(
    settings: &TankControllerSettings,
    contact: TerrainContact,
    brake: f32,
    throttle: f32,
    uphill_grade: f32,
) -> f32 {
    if brake > 0.0 {
        return settings.brake_deceleration_mps2 * brake;
    }

    let base = if throttle.abs() > 0.01 {
        settings.acceleration_mps2 * contact.traction
    } else {
        settings.idle_drag_mps2
    };
    (base - uphill_grade * 4.5).max(settings.idle_drag_mps2 * 0.5)
}

fn sample_offset(
    heightmap: &HeightMap,
    position: Vec3,
    direction: Vec3,
    distance: f32,
) -> Option<f32> {
    let point = position + direction * distance;
    heightmap.sample_height(point.x, point.z)
}

fn move_towards(current: f32, target: f32, max_delta: f32) -> f32 {
    current + (target - current).clamp(-max_delta, max_delta)
}
