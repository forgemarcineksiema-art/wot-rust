use game_core::math::{GRAVITY_MPS2, horizontal_forward, wrap_angle};
use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::contact::TerrainContact;
use crate::controller_settings::TankControllerSettings;
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
    let g = GRAVITY_MPS2;
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

    // --- 2. Decompose the surviving world velocity into the rotated hull frame. ---
    let forward = horizontal_forward(state.yaw_rad);
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    let mut v_f = state.velocity.dot(forward);
    let mut v_r = state.velocity.dot(right);

    // --- 3. Gravity along the terrain plane (single source of slope behaviour). ---
    // grade = |gradient|; `inv` is cos(theta). Projecting gravity onto the plane gives uphill
    // resistance, downhill acceleration, and sideways pull from one term — the old five stacked
    // slope penalties collapse into this.
    let grade = (contact.forward_slope * contact.forward_slope
        + contact.side_slope * contact.side_slope)
        .sqrt();
    let inv = 1.0 / (1.0 + grade * grade).sqrt();
    let slope_f = -g * contact.forward_slope * inv; // +forward_slope = uphill ahead -> resists
    let slope_r = -g * contact.side_slope * inv; // +side_slope = right is higher -> pulls left

    // --- 4. Longitudinal: engine force (P/v), resistances, gravity, then holds. ---
    let max_speed = if throttle >= 0.0 {
        settings.max_forward_speed_mps
    } else {
        settings.max_reverse_speed_mps
    };
    // Tracks can only lay down so much thrust: mu * g * traction * cos(theta). A face steeper than
    // `mu` (= max_climb_grade) therefore cannot out-pull gravity, so the climb stalls on its own.
    let grip_long = settings.longitudinal_grip_mu * g * contact.traction * inv;
    if brake > 0.0 {
        v_f = move_towards(v_f, 0.0, settings.brake_deceleration_mps2 * brake * dt);
    } else if throttle.abs() > 0.01 {
        // Engine thrust follows P/v: enormous at a crawl (where the track grip cap takes over),
        // thin near top speed — so vmax is where thrust meets the resistances, not a clamp.
        let dir = throttle.signum();
        let a_engine = settings.drive_power_mps3 * throttle.abs()
            / v_f.abs().max(settings.min_force_speed_mps);
        let commanded = dir * max_speed * throttle.abs();
        if (commanded - v_f) * dir > 0.0 {
            v_f += dir * a_engine.min(grip_long) * dt;
        } else {
            // Above the commanded speed (throttle eased, or a downhill run): engine braking.
            v_f = move_towards(v_f, commanded, settings.idle_drag_mps2 * dt);
        }
    } else {
        // Coasting: engine braking + rolling resistance — a long roll-out, not a sudden anchor.
        v_f = move_towards(v_f, 0.0, settings.idle_drag_mps2 * dt);
    }
    // Rolling + aerodynamic-ish quadratic resistance apply in every state; together with P/v they
    // put the top-speed equilibrium exactly at the spec vmax.
    let resistance =
        settings.rolling_resist_mps2 * contact.traction.max(0.5) + settings.drag_quadratic * v_f * v_f;
    v_f = move_towards(v_f, 0.0, resistance * dt);
    // Skid-steer scrub: turning bleeds forward speed into the ground.
    let scrub = settings.turn_scrub * state.yaw_rate_rad_s.abs() * v_f.abs();
    v_f = move_towards(v_f, 0.0, scrub * dt);
    v_f += slope_f * dt;
    // Track brakes hold a throttled hull on a slope it is climbing instead of letting it creep
    // backwards; this is what turns an unclimbable face into a clean stall. Fires when gravity
    // tries to push the hull opposite the way it is being driven.
    let driven_backwards = (throttle > 0.01 && v_f < 0.0) || (throttle < -0.01 && v_f > 0.0);
    if driven_backwards {
        v_f = 0.0;
    }
    // Governor: bleed any overspeed (e.g. a long downhill) back toward the track limit.
    let speed_cap = max_speed.max(0.1);
    if v_f.abs() > speed_cap {
        v_f = move_towards(v_f, v_f.signum() * speed_cap, settings.idle_drag_mps2 * dt);
    }
    // Designed barrier: a face steeper than the hull's gradeability cannot be mounted at all -- the
    // tracks lose drive and the nose digs in -- so a steep embankment stays a wall instead of
    // something a fast hull can bump over on momentum. This only fires *into* an unclimbable uphill
    // face; climbable slopes keep the smooth gravity model above.
    let uphill_grade = (contact.forward_slope * v_f.signum()).max(0.0);
    if uphill_grade >= settings.max_climb_grade && v_f.abs() > 0.0 {
        v_f = 0.0;
    }

    // --- 5. Lateral: gravity pulls sideways, kinetic friction removes it up to the grip cap. ---
    // The friction impulse only ever cancels lateral velocity (never reverses it), so it is stable
    // at 60 Hz. When the demand (a hard turn at speed, or a steep/low-traction face) exceeds the
    // cap, the residual is the slide.
    v_r += slope_r * dt;
    let lat_cap = settings.lateral_grip_mu * g * contact.traction;
    v_r -= v_r.signum() * v_r.abs().min(lat_cap * dt);

    // --- 6. Reassemble the world velocity and integrate position. The height is NOT touched
    // here: the world step resolves it against the terrain (`vertical::resolve_vertical`), which
    // is what lets a hull leave the ground instead of teleporting down every face.
    state.velocity = forward * v_f + right * v_r;
    state.position += state.velocity * dt;
}

fn move_towards(current: f32, target: f32, max_delta: f32) -> f32 {
    current + (target - current).clamp(-max_delta, max_delta)
}
