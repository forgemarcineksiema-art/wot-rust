//! Mouse input owns a desired sight ray: independent yaw/pitch that drives the camera target.
//! The gun then solves toward the world point under that sight ray. The shot path remains
//! authoritative server/sim state; this module only provides client-side aim prediction.

use game_core::math::{gun_direction, wrap_angle};
use game_core::{TankId, TeamId};
use glam::Vec3;
use net::TankSnapshot;
use sim::{ShellTraceWorld, segment_impact};
use terrain::{HeightMap, StaticCoverObject};

// Sight rays must out-range the map: Prokhorovka targets 1000 m of playable space, and a sniper
// duel across the long diagonal aims well past 600 m.
const AIM_MAX_RANGE_M: f32 = 1200.0;
const GUN_PITCH_SOLVER_MIN_RAD: f32 = -0.5;
const GUN_PITCH_SOLVER_MAX_RAD: f32 = 0.8;
const GUN_PITCH_SOLVER_STEPS: usize = 18;

/// The player's sight ray as a **world-space** direction: yaw is the world azimuth, pitch is the
/// world elevation of the line the crosshair points along (NOT a hull-relative gun angle). The gun
/// then solves — hull-aware — toward the world point under this ray, and the app clamps the pitch
/// to what the gun can actually reach on the current hull (`clamp_desired_aim_to_gun_reach`). This
/// separation is what makes the sniper view correct on slopes: the sight looks where you point in
/// the world, independent of hull tilt, while the gun carries the tilt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DesiredAim {
    yaw_rad: f32,
    pitch_rad: f32,
}

/// Safety bound on the world sight elevation (~46°). The real limit is the per-frame gun-reach
/// clamp; this only stops the accumulated pitch from running away past anything sane.
const VIEW_PITCH_LIMIT_RAD: f32 = 0.8;

impl DesiredAim {
    pub(crate) fn new(yaw_rad: f32, pitch_rad: f32) -> Self {
        Self {
            yaw_rad: wrap_angle(yaw_rad),
            pitch_rad: pitch_rad.clamp(-VIEW_PITCH_LIMIT_RAD, VIEW_PITCH_LIMIT_RAD),
        }
    }

    pub(crate) fn yaw_rad(self) -> f32 {
        self.yaw_rad
    }

    pub(crate) fn pitch_rad(self) -> f32 {
        self.pitch_rad
    }

    pub(crate) fn set_yaw(&mut self, yaw_rad: f32) {
        self.yaw_rad = wrap_angle(yaw_rad);
    }

    /// Set the world sight elevation directly (used by the gun-reach clamp), kept in the safety bound.
    pub(crate) fn set_pitch(&mut self, pitch_rad: f32) {
        self.pitch_rad = pitch_rad.clamp(-VIEW_PITCH_LIMIT_RAD, VIEW_PITCH_LIMIT_RAD);
    }

    pub(crate) fn apply_pitch_delta(&mut self, delta_rad: f32) {
        self.set_pitch(self.pitch_rad + delta_rad);
    }
}

impl Default for DesiredAim {
    fn default() -> Self {
        Self::new(0.0, 0.0)
    }
}

/// March the screen-center ray (`eye` + t·`forward`) to the first terrain hit, returning the
/// world aim point. Off-map samples count as open sky, so a ray that clears the map aims at
/// its far end rather than snapping short.
#[cfg(test)]
pub(crate) fn aim_point(heightmap: &HeightMap, eye: Vec3, forward: Vec3) -> Vec3 {
    const STEP_M: f32 = 1.0;
    let mut previous = eye;
    let mut previous_clearance = clearance(heightmap, eye);
    let mut travelled = STEP_M;
    while travelled <= AIM_MAX_RANGE_M {
        let point = eye + forward * travelled;
        let clearance = clearance(heightmap, point);
        if previous_clearance > 0.0 && clearance <= 0.0 {
            let frac = previous_clearance / (previous_clearance - clearance);
            return previous.lerp(point, frac.clamp(0.0, 1.0));
        }
        previous = point;
        previous_clearance = clearance;
        travelled += STEP_M;
    }
    eye + forward * AIM_MAX_RANGE_M
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn aim_point_with_sweep(
    heightmap: &HeightMap,
    cover: &[StaticCoverObject],
    tanks: &[TankSnapshot],
    owner: TankId,
    owner_team: TeamId,
    eye: Vec3,
    forward: Vec3,
) -> Vec3 {
    const STEP_M: f32 = 1.0;
    let sets = crate::hud::reticle_sweep::trace_sets(tanks, owner, owner_team);
    let world = ShellTraceWorld {
        tanks: &sets.targets,
        blockers: &sets.blockers,
        heightmap: Some(heightmap),
        cover,
    };
    let mut previous = eye;
    let mut travelled = STEP_M;
    while travelled <= AIM_MAX_RANGE_M {
        let point = eye + forward * travelled;
        // Straight sight ray, so `forward` stands in for the shell velocity; the aim point only
        // needs the impact location, not the (unused) tank impact angle.
        if let Some(impact) = segment_impact(previous, point, forward, &world) {
            return impact.point();
        }
        previous = point;
        travelled += STEP_M;
    }
    eye + forward * AIM_MAX_RANGE_M
}

/// Gun elevation (radians, + = up) that puts the muzzle on `aim` using the same fixed-step
/// ballistic integration (drag included) as the authoritative shell trace.
pub(crate) fn gun_pitch_to_hit(
    muzzle: Vec3,
    aim: Vec3,
    muzzle_velocity_mps: f32,
    drag_per_s: f32,
) -> f32 {
    let delta = aim - muzzle;
    let horizontal = (delta.x * delta.x + delta.z * delta.z).sqrt();
    if horizontal < 0.5 {
        return 0.0;
    }
    if muzzle_velocity_mps <= 0.0 {
        return delta.y.atan2(horizontal);
    }
    let mut low = GUN_PITCH_SOLVER_MIN_RAD;
    let mut high = GUN_PITCH_SOLVER_MAX_RAD;
    for _ in 0..GUN_PITCH_SOLVER_STEPS {
        let mid = (low + high) * 0.5;
        if shell_height_at_horizontal(muzzle, mid, muzzle_velocity_mps, drag_per_s, horizontal)
            < aim.y
        {
            low = mid;
        } else {
            high = mid;
        }
    }
    (low + high) * 0.5
}

fn shell_height_at_horizontal(
    muzzle: Vec3,
    pitch_rad: f32,
    muzzle_velocity_mps: f32,
    drag_per_s: f32,
    horizontal: f32,
) -> f32 {
    let dt_seconds = crate::hud::reticle_sweep::tick_dt_seconds();
    let mut position = muzzle;
    let mut velocity = gun_direction(0.0, pitch_rad) * muzzle_velocity_mps;
    let mut previous_range = 0.0;
    for _ in 0..(sim::SHELL_MAX_AGE_SECONDS / dt_seconds).ceil() as usize {
        let previous = position;
        game_core::math::integrate_shell_step(&mut velocity, drag_per_s, dt_seconds);
        position += velocity * dt_seconds;
        let range = ((position.x - muzzle.x).powi(2) + (position.z - muzzle.z).powi(2)).sqrt();
        if range >= horizontal {
            let frac = (horizontal - previous_range) / (range - previous_range);
            return previous.y + (position.y - previous.y) * frac.clamp(0.0, 1.0);
        }
        previous_range = range;
    }
    position.y
}

#[cfg(test)]
fn clearance(heightmap: &HeightMap, point: Vec3) -> f32 {
    heightmap.sample_height(point.x, point.z).map_or(f32::MAX, |ground| point.y - ground)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_hits_flat_ground_ahead_of_the_camera() {
        let flat = HeightMap::flat(64, 64, 5.0, 0.0).unwrap();
        let eye = Vec3::new(100.0, 10.0, 100.0);
        let forward = Vec3::new(0.0, -1.0, 1.0).normalize();
        let hit = aim_point(&flat, eye, forward);
        // Descends 10 m over 10 m of +Z before reaching ground level.
        assert!(hit.y.abs() < 0.5, "hit near ground, y = {}", hit.y);
        assert!((hit.z - 110.0).abs() < 1.5, "hit ~10 m ahead, z = {}", hit.z);
    }

    #[test]
    fn open_sky_ray_aims_at_max_range() {
        let flat = HeightMap::flat(64, 64, 5.0, 0.0).unwrap();
        let eye = Vec3::new(100.0, 10.0, 100.0);
        let hit = aim_point(&flat, eye, Vec3::Y); // straight up never meets ground
        assert!((hit.y - (10.0 + AIM_MAX_RANGE_M)).abs() < 1.0e-3);
    }

    #[test]
    fn elevation_rises_for_higher_targets() {
        let muzzle = Vec3::new(0.0, 1.0, 0.0);
        let level = gun_pitch_to_hit(muzzle, Vec3::new(0.0, 1.0, 100.0), 895.0, 0.09);
        assert!(level.abs() < 0.02, "level shot ~ 0, got {level}");
        let raised = gun_pitch_to_hit(muzzle, Vec3::new(0.0, 21.0, 100.0), 895.0, 0.09);
        assert!(raised > 0.15, "20 m up over 100 m elevates the gun, got {raised}");
    }

    #[test]
    fn elevation_matches_authoritative_shell_step_for_slow_long_shot() {
        let muzzle = Vec3::new(0.0, 1.0, 0.0);
        let aim = Vec3::new(0.0, 1.0, 500.0);
        let pitch = gun_pitch_to_hit(muzzle, aim, 500.0, 0.09);
        let shell_y = shell_height_at_horizontal(muzzle, pitch, 500.0, 0.09, 500.0);
        assert!(
            (shell_y - aim.y).abs() < 0.02,
            "pitch {pitch} should put the simulated shell at target height {}, got {shell_y}",
            aim.y
        );
    }

    #[test]
    fn drag_demands_more_elevation_at_range() {
        // The same shot solved in still air and in drag: the draggy shell arrives slower and
        // drops longer, so the solver must hold the gun higher — one integration for the arc
        // you fly and the arc you solve.
        let muzzle = Vec3::new(0.0, 1.0, 0.0);
        let aim = Vec3::new(0.0, 1.0, 800.0);
        let vacuum = gun_pitch_to_hit(muzzle, aim, 500.0, 0.0);
        let dragged = gun_pitch_to_hit(muzzle, aim, 500.0, 0.21);
        assert!(dragged > vacuum + 1.0e-3, "drag must raise the solution: {dragged} vs {vacuum}");
    }
}
