use game_core::math::{horizontal_forward, wrap_angle};
use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::contact::TerrainContact;
use crate::controller_settings::TankControllerSettings;
use crate::forces::move_towards;
use crate::vertical::is_grounded;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TankControlInput {
    pub throttle: f32,
    pub steer: f32,
    pub brake: f32,
}

/// Planar rigid-body state of a hull. `velocity` is the world-frame velocity (its y is zero
/// while the ground carries the hull and becomes the fall speed when it leaves the ground — see
/// `vertical::resolve_vertical`) and `yaw_rate_rad_s` is the hull's angular velocity. Keeping a
/// real velocity *vector* (not a scalar forward speed) is what lets the hull carry momentum
/// through a turn, slide on a steep face, and be stopped along one axis by a wall while still
/// moving along the other.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TankKinematicState {
    pub position: Vec3,
    pub velocity: Vec3,
    pub yaw_rad: f32,
    pub yaw_rate_rad_s: f32,
    /// Authoritative hull pitch (+nose up), derived kinematically from the support plane and
    /// rate-limited (see `hull_attitude`). Frozen while airborne. `serde(default)` keeps older
    /// fixtures loading level.
    #[serde(default)]
    pub pitch_rad: f32,
    /// Authoritative hull roll (+right side up); same lifecycle as `pitch_rad`.
    #[serde(default)]
    pub roll_rad: f32,
}

impl Default for TankKinematicState {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            yaw_rad: 0.0,
            yaw_rate_rad_s: 0.0,
            pitch_rad: 0.0,
            roll_rad: 0.0,
        }
    }
}

impl TankKinematicState {
    /// Signed speed along the hull's facing (+ forward, - reverse). The old scalar model stored
    /// this directly; callers that still think in "forward speed" (HUD, bloom sign) read it here.
    pub fn forward_speed(&self) -> f32 {
        self.velocity.dot(horizontal_forward(self.yaw_rad))
    }

    /// World speed magnitude.
    pub fn speed(&self) -> f32 {
        self.velocity.length()
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

/// Advance the hull one tick as a planar rigid body. The model is deterministic and runs
/// identically on the server and the client predictor.
///
/// Order matters: rotate the hull first (angular inertia), then resolve forces in the *new* hull
/// frame so the world-frame velocity that survives a heading change reappears as lateral velocity
/// — that is the mechanism behind momentum-through-turns and drift.
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
    let dt = dt_seconds;

    // In flight nothing the driver does reaches the ground: no thrust, no brakes, no steering
    // authority, no ground friction. The hull keeps its linear momentum and whatever yaw rotation
    // it left the ground with; the world step resolves the vertical against the terrain.
    if !is_grounded(state.position.y, contact.height_m) {
        state.yaw_rad = wrap_angle(state.yaw_rad + state.yaw_rate_rad_s * dt);
        state.position.x += state.velocity.x * dt;
        state.position.z += state.velocity.z * dt;
        return;
    }

    // --- 1. Angular: ramp the yaw rate toward the commanded rate, then integrate the heading. ---
    // The finite `yaw_accel_rad_s2` ramp is the rotational inertia: the hull no longer snaps to a
    // new yaw the instant the key is tapped, and releasing steer lets the rotation coast down.
    let forward_speed = state.forward_speed();
    let steering_direction = if forward_speed.abs() > 0.01 {
        forward_speed.signum()
    } else if throttle.abs() > 0.01 {
        throttle.signum()
    } else {
        // Stationary with no throttle: a steer input still pivots the hull in place (neutral
        // steer / counter-rotating tracks), decoupled from the throttle.
        1.0
    };
    let turn_grip = (contact.traction - contact.roughness * 0.2).clamp(0.25, 1.0);
    let target_yaw_rate = steer * steering_direction * settings.turn_rate_rad_s * turn_grip;
    state.yaw_rate_rad_s =
        move_towards(state.yaw_rate_rad_s, target_yaw_rate, settings.yaw_accel_rad_s2 * dt);
    state.yaw_rad = wrap_angle(state.yaw_rad + state.yaw_rate_rad_s * dt);

    // --- 2. Decompose the surviving world velocity into the rotated hull frame, then resolve the
    // ground forces (slope gravity, static hold, drive/climb, lateral friction) into a new world
    // velocity — see `forces::resolve_ground_velocity`. Rotating first is what makes the velocity
    // that survives a heading change reappear as lateral velocity: momentum-through-turns and drift.
    let forward = horizontal_forward(state.yaw_rad);
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    let v_f = state.velocity.dot(forward);
    let v_r = state.velocity.dot(right);
    state.velocity = crate::forces::resolve_ground_velocity(
        v_f,
        v_r,
        state.yaw_rate_rad_s,
        throttle,
        brake,
        settings,
        &contact,
        forward,
        right,
        dt,
    );
    // The height is NOT touched here: the world step resolves it against the terrain
    // (`vertical::resolve_vertical`), which lets a hull leave the ground instead of teleporting.
    state.position += state.velocity * dt;
}
