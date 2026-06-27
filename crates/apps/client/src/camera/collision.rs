//! Boom collision and terrain clearance for the third-person camera.
//!
//! Split out of the controller so the camera math (pose construction) and the obstacle math
//! (slab tests, terrain occlusion) stay separately reviewable. Everything here is a pure function
//! of the boom geometry plus the environment — no controller state beyond two clearance scalars.

use glam::Vec3;

use super::BattleCameraEnvironment;

/// Number of terrain samples taken along the boom. Cover uses an exact slab test; terrain is
/// still a height-field march, but only a clear->blocked transition cuts the boom.
const TERRAIN_STEPS: u32 = 32;

/// Pull the eye in to the first thing the boom hits between `target` and `desired_eye`: static
/// cover (exact ray-AABB slab) or a terrain ridge. Returns `desired_eye` on a clear boom.
pub(super) fn resolve_boom_collision(
    target: Vec3,
    desired_eye: Vec3,
    environment: &BattleCameraEnvironment<'_>,
    obstacle_clearance_m: f32,
    terrain_clearance_m: f32,
) -> Vec3 {
    let segment = desired_eye - target;
    let length = segment.length();
    if length <= f32::EPSILON {
        return desired_eye;
    }

    let mut cut_t = 1.0_f32;
    for obstacle in environment.obstacles.iter() {
        if let Some(entry) = obstacle.segment_entry(target, desired_eye) {
            cut_t = cut_t.min(entry);
        }
    }
    cut_t = cut_t.min(terrain_occlusion_t(target, segment, environment, terrain_clearance_m));
    if cut_t >= 1.0 {
        return desired_eye;
    }
    let pulled = (cut_t * length - obstacle_clearance_m).max(0.0);
    target + (segment / length) * pulled
}

/// First boom fraction where the segment dips under the terrain (plus clearance). Terrain
/// occludes the camera exactly like static cover: the boom shortens in front of a ridge crest
/// instead of looking through it (raising only the eye sample left the mid-boom inside the hill).
/// Only a clear->blocked transition cuts, so a target that itself starts under the heightmap
/// (interpolation dips, test poses) keeps the full boom and falls back to plain eye clearance
/// instead of collapsing the camera onto the hull.
fn terrain_occlusion_t(
    target: Vec3,
    segment: Vec3,
    environment: &BattleCameraEnvironment<'_>,
    terrain_clearance_m: f32,
) -> f32 {
    let Some(terrain) = environment.terrain else {
        return 1.0;
    };
    let blocked = |point: Vec3| {
        terrain
            .sample_height(point.x, point.z)
            .is_some_and(|ground| point.y < ground + terrain_clearance_m)
    };
    let mut was_clear = !blocked(target);
    for step in 1..=TERRAIN_STEPS {
        let point = target + segment * (step as f32 / TERRAIN_STEPS as f32);
        if blocked(point) {
            if was_clear {
                return (step - 1) as f32 / TERRAIN_STEPS as f32;
            }
        } else {
            was_clear = true;
        }
    }
    1.0
}

/// Lift the eye so it never sits below the heightmap (plus clearance).
pub(super) fn apply_terrain_clearance(
    mut eye: Vec3,
    environment: &BattleCameraEnvironment<'_>,
    terrain_clearance_m: f32,
) -> Vec3 {
    if let Some(terrain) = environment.terrain
        && let Some(height) = terrain.sample_height(eye.x, eye.z)
    {
        eye.y = eye.y.max(height + terrain_clearance_m);
    }
    eye
}
