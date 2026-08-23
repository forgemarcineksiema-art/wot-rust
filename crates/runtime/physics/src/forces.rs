//! Ground-frame force resolution for the planar rigid-body hull: slope gravity, the static
//! track-lock (parking), longitudinal drive/resistance/climb, and lateral friction. Split from
//! `movement.rs` for the reviewability budget; the physics is unchanged and deterministic.

use game_core::math::GRAVITY_MPS2;
use glam::Vec3;

use crate::contact::TerrainContact;
use crate::controller_settings::TankControllerSettings;

/// Below this speed an opposing drift is start-up creep, not established momentum. The drive may
/// lock that creep against a slope; a hull already moving faster must decelerate through zero.
const DRIVE_DIRECTION_EPS_MPS: f32 = 0.05;

/// Resolve one grounded tick's forces into a new world-frame velocity. `v_f`/`v_r` are the hull's
/// forward/right speeds (world velocity decomposed into the current heading), `yaw_rate` drives the
/// skid-steer scrub. Returns `Vec3::ZERO` when the static hold locks the hull.
#[expect(clippy::too_many_arguments)]
pub(crate) fn resolve_ground_velocity(
    mut v_f: f32,
    mut v_r: f32,
    yaw_rate: f32,
    throttle: f32,
    brake: f32,
    settings: &TankControllerSettings,
    contact: &TerrainContact,
    forward: Vec3,
    right: Vec3,
    dt: f32,
) -> Vec3 {
    let g = GRAVITY_MPS2;
    // What the ground IS, folded into what the ground DOES. `traction` has always carried the
    // shape of the ground (slope, side slope, roughness); `ground.grip` carries its MATERIAL, from
    // the same rule the picture's splat is baked from — so cobble bites, a ploughed field slips,
    // and both do it because of what you can see under the track. Grass is exactly 1.0, which is
    // what keeps a grass map bit-identical to the model before material existed.
    let traction = (contact.traction * contact.ground.grip).clamp(0.05, 1.5);

    // Gravity along the terrain plane (single source of slope behaviour). grade = |gradient|;
    // `inv` is cos(theta). One projection gives uphill resistance, downhill accel, and side pull.
    let grade = (contact.forward_slope.powi(2) + contact.side_slope.powi(2)).sqrt();
    let inv = 1.0 / (1.0 + grade * grade).sqrt();
    let slope_f = -g * contact.forward_slope * inv; // +forward_slope = uphill ahead -> resists
    let slope_r = -g * contact.side_slope * inv; // +side_slope = right is higher -> pulls left

    // Remember whether the driver is deliberately reversing established momentum. The start-up
    // anti-rollback below may lock slope creep, but it must never turn an 8 m/s direction change
    // into a one-tick stop.
    let carrying_opposing_momentum = v_f.abs() > DRIVE_DIRECTION_EPS_MPS && v_f * throttle < 0.0;

    // Static hold: a stopped, undriven hull locks its tracks (the park brake). The slope demand
    // g*grade*inv is met by static friction mu_s*g*traction*inv — the cos(theta) cancels, so the
    // hold is simply `grade <= mu_s * traction`. Within it the hull neither creeps nor side-slides;
    // steeper (or too slick) it never grabs and the kinetic model below lets it slide. Only linear
    // motion is locked — the caller's neutral-steer pivot (yaw) still turns it in place.
    let planar_speed = (v_f * v_f + v_r * v_r).sqrt();
    if throttle.abs() < 0.01
        && planar_speed < settings.static_hold_speed_mps
        && grade <= settings.static_grip_mu * traction
        // ...and only over momentum the lock could actually arrest. A park brake is friction, not
        // an anchor: it can remove at most `mu_s * g * traction * cos(theta)` of speed in a tick.
        // Returning ZERO regardless used to erase anything below the grab threshold outright,
        // which was invisible while a hull's own drive was the only thing that could put velocity
        // there — and became a real hole the moment CONTACT could. A shoved hull was re-zeroed by
        // its own handbrake on the very next tick, so a push could never accumulate into a shove.
        && planar_speed <= settings.static_grip_mu * g * traction * inv * dt
    {
        return Vec3::ZERO;
    }

    // Tracks slip progressively on faces past the steady gradeability: steady climbing stalls at
    // `max_climb_grade`, but the slip only *weakens* drive (it does not zero it), so momentum can
    // carry the hull a bounded way up a steep hump before it bleeds off.
    let drive_dir = if v_f.abs() > DRIVE_DIRECTION_EPS_MPS {
        v_f.signum()
    } else if throttle.abs() > 0.01 {
        throttle.signum()
    } else {
        0.0
    };
    let uphill_ahead = (contact.forward_slope * drive_dir).max(0.0);
    let climb_slip = if uphill_ahead > settings.max_climb_grade {
        (settings.max_climb_grade / uphill_ahead).powi(2)
    } else {
        1.0
    };
    let max_speed = if throttle >= 0.0 {
        settings.max_forward_speed_mps
    } else {
        settings.max_reverse_speed_mps
    };
    // Track thrust cap: mu * g * traction * cos(theta), weakened by the climb slip on steep faces.
    let grip_long = settings.longitudinal_grip_mu * g * traction * inv * climb_slip;
    if brake > 0.0 {
        v_f = move_towards(v_f, 0.0, settings.brake_deceleration_mps2 * brake * dt);
    } else if throttle.abs() > 0.01 {
        // Engine thrust follows P/v: huge at a crawl (grip-capped), thin near top speed.
        let dir = throttle.signum();
        let a_engine = settings.drive_power_mps3 * throttle.abs()
            / v_f.abs().max(settings.min_force_speed_mps);
        let commanded = dir * max_speed * throttle.abs();
        if (commanded - v_f) * dir > 0.0 {
            v_f += dir * a_engine.min(grip_long) * dt;
        } else {
            v_f = move_towards(v_f, commanded, settings.idle_drag_mps2 * dt);
        }
    } else {
        v_f = move_towards(v_f, 0.0, settings.idle_drag_mps2 * dt);
    }
    // Rolling + quadratic resistance (every state) put the top-speed equilibrium at the spec vmax.
    let resistance =
        settings.rolling_resist_mps2 * traction.max(0.5) * contact.ground.rolling_resist
            + settings.drag_quadratic * v_f * v_f;
    v_f = move_towards(v_f, 0.0, resistance * dt);
    // Wading: past the splash depth the hull pushes a bow wave and the bed sucks at the tracks
    // (see `water`). Exactly zero on dry ground, so waterless maps stay bit-identical.
    let wading = crate::water::wading_resistance_mps2(contact.water_depth_m, v_f);
    if wading > 0.0 {
        v_f = move_towards(v_f, 0.0, wading * dt);
        v_r = move_towards(v_r, 0.0, wading * dt);
    }
    let scrub = settings.turn_scrub * yaw_rate.abs() * v_f.abs(); // skid-steer bleed
    v_f = move_towards(v_f, 0.0, scrub * dt);
    v_f += slope_f * dt;
    // Track brakes hold a starting/stalled hull instead of letting gravity creep it against the
    // commanded direction. Established opposing momentum is different: changing W <-> S must
    // bleed it through the force model before the hull can reverse.
    if !carrying_opposing_momentum
        && ((throttle > 0.01 && v_f < 0.0) || (throttle < -0.01 && v_f > 0.0))
    {
        v_f = 0.0;
    }
    // Governor: bleed any overspeed (a long downhill) back toward the track limit.
    let speed_cap = max_speed.max(0.1);
    if v_f.abs() > speed_cap {
        v_f = move_towards(v_f, v_f.signum() * speed_cap, settings.idle_drag_mps2 * dt);
    }
    // Designed wall: a face past the climb *ceiling* (a cliff or the railway embankment) finds no
    // drive and digs the nose in — a barrier, not a momentum bump-over. The 0.6–ceiling band above
    // keeps the smooth momentum-climb model.
    if uphill_ahead >= settings.momentum_climb_ceiling && v_f.abs() > 0.0 {
        v_f = 0.0;
    }

    // Lateral: gravity pulls sideways, kinetic friction cancels it up to the grip cap; the residual
    // above the cap (a hard turn at speed, or a steep/low-traction face) is the slide.
    v_r += slope_r * dt;
    let lat_cap = settings.lateral_grip_mu * g * traction;
    v_r -= v_r.signum() * v_r.abs().min(lat_cap * dt);

    forward * v_f + right * v_r
}

pub(crate) fn move_towards(current: f32, target: f32, max_delta: f32) -> f32 {
    current + (target - current).clamp(-max_delta, max_delta)
}
