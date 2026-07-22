//! Cursor-to-world picking: a ray from the viewport mouse position marched against the
//! compiled heightmap. Pure math (testable without a window) — and the seed of every M4
//! brush: the brush cursor IS this probe.

use glam::Vec3;
use renderer_api::Camera;
use terrain::HeightMap;

/// The world-space ray under a viewport point. `ndc` is normalized device coordinates
/// (x right, y up, both −1..1); the basis mirrors `view_projection_matrix` (RH look-at,
/// +Y up, vertical FOV).
pub fn cursor_ray(camera: &Camera, aspect: f32, ndc: [f32; 2]) -> (Vec3, Vec3) {
    let eye = Vec3::from_array(camera.eye);
    let forward = (Vec3::from_array(camera.target) - eye).normalize_or_zero();
    let right = forward.cross(Vec3::Y).normalize_or_zero();
    let up = right.cross(forward);
    let half_v = (camera.vertical_fov_degrees.to_radians() * 0.5).tan();
    let direction =
        (forward + right * (ndc[0] * half_v * aspect) + up * (ndc[1] * half_v)).normalize_or_zero();
    (eye, direction)
}

/// Slab test of a ray against an axis-aligned box: the entry distance when it hits (0 when
/// the origin already sits inside). The shared pick primitive — cover selection and the
/// grab tool's form handles use the SAME math, so what the eye clicks is what both mean.
pub fn ray_aabb(origin: Vec3, direction: Vec3, center: Vec3, half: Vec3) -> Option<f32> {
    let inv = direction.recip();
    let t1 = (center - half - origin) * inv;
    let t2 = (center + half - origin) * inv;
    let near = t1.min(t2);
    let far = t1.max(t2);
    let enter = near.x.max(near.y).max(near.z);
    let exit = far.x.min(far.y).min(far.z);
    (enter <= exit && exit >= 0.0).then_some(enter.max(0.0))
}

/// March the ray against the heightmap: coarse 2 m steps to the first ground crossing, then
/// a short bisection for a centimetre-honest hit. `None` when the ray leaves the map or
/// never comes down (sky).
pub fn ground_hit(heightmap: &HeightMap, origin: Vec3, direction: Vec3) -> Option<Vec3> {
    const STEP_M: f32 = 2.0;
    const RANGE_M: f32 = 2_000.0;
    let above = |point: Vec3| heightmap.sample_height(point.x, point.z).map(|h| point.y - h);
    let mut t = 0.0_f32;
    let mut previous: Option<(f32, f32)> = above(origin).map(|clearance| (0.0, clearance));
    while t < RANGE_M {
        t += STEP_M;
        let point = origin + direction * t;
        let Some(clearance) = above(point) else {
            previous = None;
            continue;
        };
        if clearance <= 0.0 {
            // Bisect between the last above-ground sample and this one.
            let (mut lo, mut hi) = (previous.map_or(t - STEP_M, |(t0, _)| t0), t);
            for _ in 0..16 {
                let mid = (lo + hi) * 0.5;
                let point = origin + direction * mid;
                match above(point) {
                    Some(clearance) if clearance > 0.0 => lo = mid,
                    _ => hi = mid,
                }
            }
            let hit = origin + direction * hi;
            let ground = heightmap.sample_height(hit.x, hit.z)?;
            return Some(Vec3::new(hit.x, ground, hit.z));
        }
        previous = Some((t, clearance));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_probe_hits_the_flat_scratch_ground_where_the_eye_looks() {
        let map = map_forge::battlefield(terrain::MapId::Scratch);
        let camera = Camera {
            eye: [150.0, 60.0, 40.0],
            target: [150.0, 0.0, 150.0],
            vertical_fov_degrees: 55.0,
        };
        // Straight down the view centre: the hit must sit on the eye→target line at ground
        // height (the placeholder is a constant 5 m plain).
        let (origin, direction) = cursor_ray(&camera, 16.0 / 9.0, [0.0, 0.0]);
        let hit = ground_hit(&map.heightmap, origin, direction).expect("the plain is hit");
        assert!((hit.y - 5.0).abs() < 0.05, "ground height, got {}", hit.y);
        assert!((hit.x - 150.0).abs() < 0.5, "on the centre line, got {}", hit.x);

        // A ray toward the sky never lands (a LEVEL camera tilted up by the viewport top).
        let level = Camera {
            eye: [150.0, 60.0, 40.0],
            target: [150.0, 60.0, 150.0],
            vertical_fov_degrees: 55.0,
        };
        let (origin, direction) = cursor_ray(&level, 16.0 / 9.0, [0.0, 1.0]);
        assert!(direction.y > 0.4, "the viewport top of a level camera aims upward");
        assert!(ground_hit(&map.heightmap, origin, direction).is_none());
    }
}
