//! Bot gunnery: moving-target intercept, ballistic lay and target-sized trigger gates.
//!
//! Pitch and flight time both come from [`game_core::math::integrate_shell_step`], the exact
//! authoritative shell step. There is no private simplified bot trajectory.

use game_core::math::{gun_direction, integrate_shell_step, wrap_angle};
use game_core::{
    ArmorZone, ShellSpec, WeakspotFrame, resolve_penetration_at_distance_on_zone_scaled,
    vehicle_armor_volumes,
};
use glam::{Mat3, Vec3};
use sim::{
    SHELL_MAX_AGE_SECONDS, SegmentImpact, ShellTraceWorld, TankState, TraceTank, segment_impact,
};

/// Match the authoritative server's 60 Hz shell integration.
const SOLVE_DT_S: f32 = 1.0 / 60.0;
/// Fixed-point refinements for drop compensation and linear target intercept.
const SOLVE_ROUNDS: usize = 4;
const INTERCEPT_ROUNDS: usize = 4;
/// A corrupt velocity must not send a cached lay across the map.
const MAX_LEAD_DISPLACEMENT_M: f32 = 120.0;
/// Aim at the middle 80% of the target silhouette, leaving dispersion margin at the rim.
const GATE_MARGIN: f32 = 0.8;
/// How far past a weakspot's surface the verification lay probes, so the segment actually
/// enters the armor volume instead of stopping ON its boundary plane.
const LAY_PROBE_OVERSHOOT_M: f32 = 2.0;
/// A weakspot is worth holding on only while its disc subtends at least this fraction of the
/// gun's aimed dispersion radius — roughly a 2% straight-in hit chance. Below that the hold is
/// theater: the round lands by luck alone, on the very plate the gunner switched away from.
/// With the fleet's 2–3 mrad guns this retires a 0.11 m bow port past ~300 m; what happens
/// past the line belongs to the futility clock, which already makes a bouncing bot relocate —
/// closing in IS the counterplay the weakspot economy asks for.
const PATCH_FEASIBLE_DISPERSION_FACTOR: f32 = 0.15;

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
    let choice = choose_aim_point(tank, target);
    let shell = tank.selected_shell();
    let (aim_point, world_pitch) = intercept_lay(
        muzzle,
        choice.point,
        target.velocity_mps,
        shell.muzzle_velocity_mps,
        shell.drag_per_s(),
    );
    let delta = aim_point - muzzle;
    let world_yaw = delta.x.atan2(delta.z);
    let local = tank.hull_pose().basis().transpose() * gun_direction(world_yaw, world_pitch);
    let desired_turret = local.x.atan2(local.z);
    // The bot obeys ITS OWN gun's arc: a T-54 bot must not promise itself the -8 degrees the
    // fleet constant used to hand everyone.
    let (min_pitch, max_pitch) = tank.spec.gun_pitch_limits_rad();
    let desired_pitch = local.y.clamp(-1.0, 1.0).asin().clamp(min_pitch, max_pitch);
    let distance = delta.length().max(1.0);
    BotFiringSolution {
        target: target.id,
        desired_turret_yaw_rad: desired_turret,
        desired_gun_pitch_rad: desired_pitch,
        aim_point_world: aim_point,
        yaw_gate: (choice.half_width_m * GATE_MARGIN / distance).atan(),
        pitch_gate: (choice.half_height_m * GATE_MARGIN / distance).atan(),
    }
}

/// The point on the target worth shooting, with the half-extents the trigger gates read.
struct AimChoice {
    point: Vec3,
    half_width_m: f32,
    half_height_m: f32,
}

/// Centre of mass while the gun beats the armor there; the largest penetrable weakspot disc
/// when it does not. The estimate is the crew's honest knowledge: the same shell trace and the
/// same penetration resolution the player's reticle hint runs against a spotted target —
/// nothing here reads hidden state, only the enemy's visible geometry and the bot's own gun.
///
/// The straight lay stands in for the arc, exactly like the trigger-safety line: at the ranges
/// where drop would bend the answer the dispersion gate has already retired the weakspot.
fn choose_aim_point(tank: &TankState, target: &TankState) -> AimChoice {
    let center = target.position + Vec3::Y * target.spec.hitbox.center_y_m;
    let hitbox = AimChoice {
        point: center,
        half_width_m: target.spec.hitbox.half_width_m,
        half_height_m: target.spec.hitbox.half_height_m,
    };
    // Vehicles without baked volumes have no patch geometry to aim at (and their box bands
    // never auto-bounce the way a real front does): centre of mass, as always.
    let Some(volumes) = vehicle_armor_volumes(target.spec.kind) else {
        return hitbox;
    };
    let muzzle = tank.muzzle_world_position();
    let shell = tank.selected_shell();
    let trace_tank = [TraceTank::from_spec(
        target.id,
        target.position,
        target.hull_pose(),
        target.turret_yaw_rad,
        &target.spec,
    )];
    let world = ShellTraceWorld {
        projectile_radius_m: shell.collision_radius_m(),
        tanks: &trace_tank,
        blockers: &[],
        heightmap: None,
        cover: &[],
        water: terrain::WaterView::DRY,
    };
    // A centre the shell beats — or a degenerate pose the probe cannot read — keeps the lay
    // this solver has always produced.
    match penetration_on_lay(muzzle, center, &shell, target, &world) {
        Some((_, penetrated)) if !penetrated => {}
        _ => return hitbox,
    }
    let basis = target.hull_pose().basis();
    let pivot = Vec3::new(0.0, 0.0, volumes.turret_ring_z);
    let turret_spin = Mat3::from_rotation_y(target.turret_yaw_rad);
    let mut spots = volumes.weakspot_aim_points();
    // Largest disc first: among penetrable weakspots the biggest is the one dispersion is most
    // likely to actually put a round through.
    spots.sort_by(|a, b| b.radius_m.total_cmp(&a.radius_m));
    for spot in spots {
        let local = match spot.frame {
            WeakspotFrame::Hull => spot.center,
            WeakspotFrame::Turret => pivot + turret_spin * (spot.center - pivot),
        };
        let point =
            target.position + basis * (Vec3::Y * target.spec.hitbox.center_y_m) + basis * local;
        let distance = muzzle.distance(point).max(1.0);
        if let Some(outward) = spot.outward {
            let outward_local = match spot.frame {
                WeakspotFrame::Hull => outward,
                WeakspotFrame::Turret => turret_spin * outward,
            };
            if (basis * outward_local).dot(muzzle - point) <= 0.0 {
                continue; // The disc faces away from this gun.
            }
        }
        if spot.radius_m / distance
            < tank.spec.gun.dispersion_mrad * 1.0e-3 * PATCH_FEASIBLE_DISPERSION_FACTOR
        {
            continue; // Too far to hold on a disc this small.
        }
        // The verification asks the authoritative trace: does the straight lay to this point
        // actually land on the weakspot's own zone (not occluded by another part of the same
        // vehicle), and does the shell beat the metal there?
        if let Some((zone, penetrated)) = penetration_on_lay(muzzle, point, &shell, target, &world)
            && zone == spot.zone
            && penetrated
        {
            return AimChoice { point, half_width_m: spot.radius_m, half_height_m: spot.radius_m };
        }
    }
    hitbox
}

/// The zone a straight lay from `muzzle` through `point` strikes on the target, and whether
/// the bot's shell beats the metal there — the reticle hint's two-step, run server-side.
/// `None` when the segment fails to strike the target at all.
fn penetration_on_lay(
    muzzle: Vec3,
    point: Vec3,
    shell: &ShellSpec,
    target: &TankState,
    world: &ShellTraceWorld<'_>,
) -> Option<(ArmorZone, bool)> {
    let direction = (point - muzzle).normalize_or_zero();
    if direction == Vec3::ZERO {
        return None;
    }
    let probe_end = point + direction * LAY_PROBE_OVERSHOOT_M;
    let impact = segment_impact(muzzle, probe_end, direction * shell.muzzle_velocity_mps, world)?;
    let SegmentImpact::Tank { zone, impact_angle_degrees, hit_position, thickness_scale, .. } =
        impact
    else {
        return None;
    };
    let result = resolve_penetration_at_distance_on_zone_scaled(
        shell,
        &target.spec.hull,
        zone,
        impact_angle_degrees,
        muzzle.distance(hit_position),
        thickness_scale,
    );
    Some((zone, result.penetrated))
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
