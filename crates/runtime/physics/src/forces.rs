//! Ground-frame force resolution for the planar rigid-body hull: slope gravity, the static
//! track-lock (parking), longitudinal drive/resistance/climb, and lateral friction. Split from
//! `movement.rs` for the reviewability budget; the physics is unchanged and deterministic.

use game_core::math::GRAVITY_MPS2;
use glam::Vec3;

use crate::contact::TerrainContact;
use crate::controller_settings::TankControllerSettings;

/// Resolve one grounded tick's forces into a new world-frame velocity. `v_f`/`v_r` are the hull's
/// forward/right speeds (world velocity decomposed into the current heading), `yaw_rate` drives the
/// skid-steer scrub. Returns `Vec3::ZERO` when the static hold locks the hull.
#[allow(clippy::too_many_arguments)]
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

    // Gravity along the terrain plane (single source of slope behaviour). grade = |gradient|;
    // `inv` is cos(theta). One projection gives uphill resistance, downhill accel, and side pull.
    let grade = (contact.forward_slope.powi(2) + contact.side_slope.powi(2)).sqrt();
    let inv = 1.0 / (1.0 + grade * grade).sqrt();
    let slope_f = -g * contact.forward_slope * inv; // +forward_slope = uphill ahead -> resists
    let slope_r = -g * contact.side_slope * inv; // +side_slope = right is higher -> pulls left

    // Static hold: a stopped, undriven hull locks its tracks (the park brake). The slope demand
    // g*grade*inv is met by static friction mu_s*g*traction*inv — the cos(theta) cancels, so the
    // hold is simply `grade <= mu_s * traction`. Within it the hull neither creeps nor side-slides;
    // steeper (or too slick) it never grabs and the kinetic model below lets it slide. Only linear
    // motion is locked — the caller's neutral-steer pivot (yaw) still turns it in place.
    let planar_speed = (v_f * v_f + v_r * v_r).sqrt();
    if throttle.abs() < 0.01
        && planar_speed < settings.static_hold_speed_mps
        && grade <= settings.static_grip_mu * contact.traction
    {
        return Vec3::ZERO;
    }

    // Tracks slip progressively on faces past the steady gradeability: steady climbing stalls at
    // `max_climb_grade`, but the slip only *weakens* drive (it does not zero it), so momentum can
    // carry the hull a bounded way up a steep hump before it bleeds off.
    let drive_dir = if v_f.abs() > 0.05 {
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
    let grip_long = settings.longitudinal_grip_mu * g * contact.traction * inv * climb_slip;
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
    let resistance = settings.rolling_resist_mps2 * contact.traction.max(0.5)
        + settings.drag_quadratic * v_f * v_f;
    v_f = move_towards(v_f, 0.0, resistance * dt);
    let scrub = settings.turn_scrub * yaw_rate.abs() * v_f.abs(); // skid-steer bleed
    v_f = move_towards(v_f, 0.0, scrub * dt);
    v_f += slope_f * dt;
    // Track brakes hold a throttled hull instead of letting gravity creep it backwards.
    if (throttle > 0.01 && v_f < 0.0) || (throttle < -0.01 && v_f > 0.0) {
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
    let lat_cap = settings.lateral_grip_mu * g * contact.traction;
    v_r -= v_r.signum() * v_r.abs().min(lat_cap * dt);

    forward * v_f + right * v_r
}

pub(crate) fn move_towards(current: f32, target: f32, max_delta: f32) -> f32 {
    current + (target - current).clamp(-max_delta, max_delta)
}
