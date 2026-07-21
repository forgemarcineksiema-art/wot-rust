//! The visibility overlay (M7): from a chosen world point at TURRET eye height, which
//! cells can a gun WORK — i.e. where would a hull-height target be seen? The same
//! stepped-segment rule the sim's spotting raycast and the map tests use (2 m steps,
//! 0.3 m clearance). This is the instrument hull-downs and crossfires are designed with;
//! the height brush only digs the ground it asks for.

use glam::Vec3;
use renderer_api::SceneVertex;
use terrain::HeightMap;

/// The observer's eye over its ground: a turret working over a crest.
pub const TURRET_EYE_M: f32 = 2.4;
/// The target's presented height: the centre of a hull.
pub const HULL_TARGET_M: f32 = 1.0;

/// The DEAD-GROUND tint: dark slate where a hull-height target is NOT seen — the shadow
/// a designer actually reads (hull-downs live in it, flanks sneak through it). Seen ground
/// keeps its natural look.
const DEAD_SLATE: [f32; 3] = [0.14, 0.15, 0.19];

/// The stepped-segment rule shared with the map contract tests.
fn sight_clear(heightmap: &HeightMap, from: Vec3, to: Vec3) -> bool {
    let flat = ((to.x - from.x).powi(2) + (to.z - from.z).powi(2)).sqrt();
    let steps = (flat / 2.0).ceil().max(1.0) as u32;
    for step in 1..steps {
        let t = step as f32 / steps as f32;
        let x = from.x + (to.x - from.x) * t;
        let z = from.z + (to.z - from.z) * t;
        let line = from.y + (to.y - from.y) * t;
        let Some(ground) = heightmap.sample_height(x, z) else { return false };
        if ground > line - 0.3 {
            return false;
        }
    }
    true
}

/// The viewshed mesh: every SECOND heightmap cell within `range_m` of the observer gets a
/// dark pad when a hull there is HIDDEN from the turret eye at `from` — dead ground made
/// visible. One-shot compute, cached by the app until the world or the anchor changes.
pub fn viewshed_mesh(
    heightmap: &HeightMap,
    from_xz: [f32; 2],
    range_m: f32,
) -> (Vec<SceneVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let cell = heightmap.cell_size_m();
    let Some(eye_ground) = heightmap.sample_height(from_xz[0], from_xz[1]) else {
        return (vertices, indices);
    };
    let eye = Vec3::new(from_xz[0], eye_ground + TURRET_EYE_M, from_xz[1]);
    let stride = 2_usize;
    for zi in (0..heightmap.height()).step_by(stride) {
        for xi in (0..heightmap.width()).step_by(stride) {
            let (x, z) = (xi as f32 * cell, zi as f32 * cell);
            let dx = x - from_xz[0];
            let dz = z - from_xz[1];
            if dx * dx + dz * dz > range_m * range_m {
                continue;
            }
            let ground = heightmap.sample_at_index(xi, zi);
            let target = Vec3::new(x, ground + HULL_TARGET_M, z);
            if sight_clear(heightmap, eye, target) {
                continue;
            }
            let half = cell * 0.5;
            let lift = ground + 0.1;
            let base = vertices.len() as u32;
            for (px, pz) in [
                (x - half, z - half),
                (x + half, z - half),
                (x + half, z + half),
                (x - half, z + half),
            ] {
                let seat = heightmap.sample_height(px, pz).unwrap_or(lift) + 0.1;
                vertices.push(SceneVertex::new([px, seat, pz], [0.0, 1.0, 0.0], DEAD_SLATE));
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ridge_shadows_the_far_side_and_open_ground_is_seen() {
        // A single ridge wall across z = 150 on a flat plain.
        let heightmap = terrain::heightmap_from_fn(61, 5.0, |_, z| {
            5.0 + 12.0 * (-0.5 * ((z - 150.0) / 6.0).powi(2)).exp()
        });
        let (vertices, _) = viewshed_mesh(&heightmap, [150.0, 60.0], 400.0);
        assert!(!vertices.is_empty());
        let dead = |z: f32| {
            vertices.iter().any(|vertex| {
                (vertex.position[2] - z).abs() < 6.0 && (vertex.position[0] - 150.0).abs() < 12.0
            })
        };
        assert!(!dead(100.0), "open ground on the observer's side is seen (no pad)");
        assert!(dead(180.0), "the ridge's far side is dead ground (hull-down works)");
    }
}
