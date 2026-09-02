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
    /// Authoritative hull pitch (+nose up), the sprung hull on its support plane (Inny Poziom G7),
    /// see `hull_attitude`. Frozen while airborne. `serde(default)` keeps older
    /// fixtures loading level.
    #[serde(default)]
    pub pitch_rad: f32,
    /// Authoritative hull roll (+right side up); same lifecycle as `pitch_rad`.
    #[serde(default)]
    pub roll_rad: f32,
    /// The pitch spring's velocity (rad/s) — the sprung hull's state (Inny Poziom G7), carried
    /// through the tick, the snapshot and the predictor so all three settle the same hull.
    #[serde(default)]
    pub pitch_vel_rad_s: f32,
    /// The roll spring's velocity (rad/s); same lifecycle as `pitch_vel_rad_s`.
    #[serde(default)]
    pub roll_vel_rad_s: f32,
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
            pitch_vel_rad_s: 0.0,
            roll_vel_rad_s: 0.0,
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

/// Decide the hull's velocity and heading for one tick, WITHOUT moving it.
///
/// The model is deterministic and runs identically on the server and the client predictor.
/// Order matters: rotate the hull first (angular inertia), then resolve forces in the *new* hull
/// frame so the world-frame velocity that survives a heading change reappears as lateral velocity
/// — that is the mechanism behind momentum-through-turns and drift.
///
/// Pair it with [`integrate_hull_position`]; [`step_custom_tank_controller`] is the two together.
pub fn advance_hull_drive(
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
        return;
    }

    // --- 1. Angular: ramp the yaw rate toward the commanded rate, then integrate the heading. ---
    // The finite `yaw_accel_rad_s2` ramp is the rotational inertia: the hull no longer snaps to a
    // new yaw the instant the key is tapped, and releasing steer lets the rotation coast down.
    let forward_speed = state.forward_speed();
    // The steer sense follows the COMMANDED drive first, and the hull's travel only when
    // coasting. A tracked hull's yaw is its belt difference, and the belts do what the driver
    // commands — with W+D held the nose swings right the instant the command lands, even while
    // the hull is still sliding backward out of a reverse. The old priority (travel direction
    // first) made every reverse->forward turn kick the WRONG way for the whole braking phase
    // and then unwind through the yaw-rate ramp (player verdict 2026-08-22: unintuitive, eats
    // reaction time). Steering while REVERSING still mirrors, exactly as before: there the
    // commanded drive itself is backward (`reverse_steering_mirrors_forward_steering`).
    let steering_direction = if throttle.abs() > 0.01 {
        throttle.signum()
    } else if forward_speed.abs() > 0.01 {
        // No throttle: the belts are dragged by the hull's own motion, so the steer sense
        // follows the travel (a coasting hull steers like it drives).
        forward_speed.signum()
    } else {
        // Stationary with no throttle: a steer input still pivots the hull in place (neutral
        // steer / counter-rotating tracks), decoupled from the throttle.
        1.0
    };
    // Turn grip reads the material too, or a hull would slide on cobble but still pivot on it
    // like grass. Same scale the force model uses; grass is exactly 1.0.
    let turn_grip =
        (contact.traction * contact.ground.grip - contact.roughness * 0.2).clamp(0.25, 1.0);
    // A hull that cannot drive one track BACKWARDS does not spin about its own centre. It slows
    // or stops the inner belt and swings about THAT, which costs it half the rate for the same
    // track speed — half as much track is doing the work. See `game_core::SteeringKind`: the
    // Tiger I and the Centurion counter-rotate, and nothing else in the roster does.
    // A pivot is a COMMAND — no throttle, steer applied — and it is recognised as one while the
    // hull is going no faster than a braked-belt pivot could itself carry it. Gating on the hull
    // being stationary instead would be circular: the walk below is what a braked-belt pivot
    // produces, so using it to cancel the pivot cancels the cause with its own effect (measured:
    // it flip-flopped tick to tick and averaged back to the full rate).
    let pivot_walk = settings.turn_rate_rad_s * settings.pivot_arm_m;
    let pivoting =
        throttle.abs() <= 0.01 && steer.abs() > 0.01 && forward_speed.abs() <= pivot_walk * 1.5;
    let mechanism = if pivoting && !settings.steering.counter_rotates() { 0.5 } else { 1.0 };
    let mut target_yaw_rate =
        steer * steering_direction * settings.turn_rate_rad_s * turn_grip * mechanism;

    // A tracked hull does not HAVE a yaw command. It has two belts, and the yaw is whatever their
    // speed difference makes it: `omega = (v_left - v_right) / gauge`. So the commanded rate above
    // is really a REQUEST FOR A DIFFERENTIAL, and a belt that cannot drive cannot supply its half.
    //
    // With one belt thrown the difference is not optional and cannot be steered out: straightening
    // means matching the belts, and the only way to match a dead belt is to stop. The clamp is
    // one-sided for exactly that reason — you may always turn HARDER toward the dead side (brake
    // the good belt), never away from it. Equal belts force nothing, so a healthy hull keeps every
    // number it had.
    // ...but only while the hull is DRIVING. During a commanded pivot the driver is setting the
    // belt difference deliberately, and a braked-belt hull's forward "speed" is then just the walk
    // around the stopped belt (below) — feeding that back in as a reason to turn more would be the
    // same circle P4.5 already fell into once, a cause cancelled by its own effect. A pivot's belt
    // health is already priced into `turn_rate_rad_s` by the caller.
    let forced = if pivoting {
        0.0
    } else {
        settings.belts.forced_yaw_rate(forward_speed, settings.pivot_arm_m)
    };
    if forced > 0.0 {
        target_yaw_rate = target_yaw_rate.max(forced);
    } else if forced < 0.0 {
        target_yaw_rate = target_yaw_rate.min(forced);
    }
    state.yaw_rate_rad_s =
        move_towards(state.yaw_rate_rad_s, target_yaw_rate, settings.yaw_accel_rad_s2 * dt);
    state.yaw_rad = wrap_angle(state.yaw_rad + state.yaw_rate_rad_s * dt);

    // --- 2. Decompose the surviving world velocity into the rotated hull frame, then resolve the
    // ground forces (slope gravity, static hold, drive/climb, lateral friction) into a new world
    // velocity — see `forces::resolve_ground_velocity`. Rotating first is what makes the velocity
    // that survives a heading change reappear as lateral velocity: momentum-through-turns and drift.
    let forward = horizontal_forward(state.yaw_rad);
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    let mut v_f = state.velocity.dot(forward);
    let v_r = state.velocity.dot(right);

    // ...and swinging about a belt is a rotation AND a forward motion, inseparably: the hull's
    // centre travels the arc around the stopped track, `omega * half_gauge` along the heading. A
    // tank steered this way does not turn on a coin — it walks itself round, which is the whole
    // difference in a street the width of a tank.
    if pivoting && !settings.steering.counter_rotates() {
        v_f = state.yaw_rate_rad_s.abs() * settings.pivot_arm_m;
    }
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
}

/// One tick of the drive with the movement included: [`advance_hull_drive`] then
/// [`integrate_hull_position`]. This is the shape every caller that does NOT interleave a
/// hull-to-hull contact solve wants — the client predictor included, since it simulates one hull
/// against neighbours it treats as static.
pub fn step_custom_tank_controller_on_contact(
    state: &mut TankKinematicState,
    input: TankControlInput,
    settings: &TankControllerSettings,
    contact: TerrainContact,
    dt_seconds: f32,
) {
    advance_hull_drive(state, input, settings, contact, dt_seconds);
    integrate_hull_position(state, dt_seconds);
}

/// Move the hull by whatever velocity it ends the tick with.
///
/// Split out of the drive step so the roster can solve hull-to-hull contacts BETWEEN deciding a
/// velocity and spending it. That ordering is the whole ballgame: a contact resolved after the
/// move can only correct the velocity for next tick, so a crowd pressing together creeps forward
/// a little every tick — which at a river ford means creeping into the water. Resolved before the
/// move, an approach that would overlap is simply never taken.
///
/// Only the horizontal axes are integrated. The height belongs to `vertical::resolve_vertical`,
/// which is what lets a hull leave the ground instead of teleporting down a cliff.
pub fn integrate_hull_position(state: &mut TankKinematicState, dt_seconds: f32) {
    state.position.x += state.velocity.x * dt_seconds;
    state.position.z += state.velocity.z * dt_seconds;
}
