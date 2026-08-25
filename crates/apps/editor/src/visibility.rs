//! The visibility overlay (M7): from a chosen world point at the commander's eye height,
//! where is a hull's CENTRE masked — the dead-ground shadow hull-downs and crossfires are
//! designed with.
//!
//! The sight rule is not copied here, it is CALLED: every ray goes through
//! `sim::line_of_sight` — the exact stepped-terrain + cover-slab rule the spotting
//! recompute resolves with — and the eye/target heights come off the benchmark T-54's
//! hitbox (`observer_eye` = hitbox top, target = hull centre), so what this instrument
//! calls dead ground is dead to the live game too. The previous local copy carried the
//! opposite slack sign and ignored cover boxes entirely: it certified crests the sim saw
//! straight over, and buildings cast no shadow in it.
//!
//! Reading the pads: dark = the hull CENTRE there is masked from the eye. The turret top
//! may still be spotted over a crest — that band is precisely where hull-down trades live.

use glam::Vec3;
use renderer_api::SceneVertex;
use terrain::{HeightMap, StaticCoverObject};

/// The DEAD-GROUND tint: dark slate where a hull-centre target is NOT seen — the shadow
/// a designer actually reads (hull-downs live in it, flanks sneak through it). Seen ground
/// keeps its natural look.
const DEAD_SLATE: [f32; 3] = [0.14, 0.15, 0.19];

/// The benchmark eye and target heights, off the same spec the sim spots with.
fn benchmark_heights() -> (f32, f32) {
    let hitbox = game_core::TankSpec::t54_1951().hitbox;
    (hitbox.center_y_m + hitbox.half_height_m, hitbox.center_y_m)
}

/// The viewshed mesh: every SECOND heightmap cell within `range_m` of the observer gets a
/// dark pad when a hull centre there is HIDDEN from the commander's eye at `from` — dead
/// ground made visible, with the map's cover boxes casting their honest shadows. One-shot
/// compute, cached by the app until the world or the anchor changes.
pub fn viewshed_mesh(
    heightmap: &HeightMap,
    cover: &[StaticCoverObject],
    from_xz: [f32; 2],
    range_m: f32,
) -> (Vec<SceneVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let cell = heightmap.cell_size_m();
    let Some(eye_ground) = heightmap.sample_height(from_xz[0], from_xz[1]) else {
        return (vertices, indices);
    };
    let (eye_m, hull_m) = benchmark_heights();
    let eye = Vec3::new(from_xz[0], eye_ground + eye_m, from_xz[1]);
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
            let target = Vec3::new(x, ground + hull_m, z);
            if sim::line_of_sight(Some(heightmap), cover, eye, target) {
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
    use terrain::StaticCoverKind;

    #[test]
    fn a_ridge_shadows_the_far_side_and_open_ground_is_seen() {
        // A single ridge wall across z = 150 on a flat plain.
        let heightmap = terrain::heightmap_from_fn(61, 5.0, |_, z| {
            5.0 + 12.0 * (-0.5 * ((z - 150.0) / 6.0).powi(2)).exp()
        });
        let (vertices, _) = viewshed_mesh(&heightmap, &[], [150.0, 60.0], 400.0);
        assert!(!vertices.is_empty());
        let dead = |z: f32| {
            vertices.iter().any(|vertex| {
                (vertex.position[2] - z).abs() < 6.0 && (vertex.position[0] - 150.0).abs() < 12.0
            })
        };
        assert!(!dead(100.0), "open ground on the observer's side is seen (no pad)");
        assert!(dead(180.0), "the ridge's far side is dead ground (hull-down works)");
    }

    /// The shadow the old instrument never cast: a building between the eye and the plain
    /// is a wall to the sim, so the viewshed must go dark behind it — and only behind it.
    #[test]
    fn a_cover_box_casts_dead_ground_behind_it() {
        let heightmap = terrain::heightmap_from_fn(61, 5.0, |_, _| 5.0);
        let barn = StaticCoverObject {
            id: "barn".into(),
            name: "barn".into(),
            kind: StaticCoverKind::FarmBuilding,
            center: [150.0, 5.0 + 4.0, 150.0],
            half_extents_m: [12.0, 4.0, 4.0],
        };
        let (vertices, _) =
            viewshed_mesh(&heightmap, std::slice::from_ref(&barn), [150.0, 60.0], 400.0);
        let dead = |z: f32| {
            vertices.iter().any(|vertex| {
                (vertex.position[2] - z).abs() < 6.0 && (vertex.position[0] - 150.0).abs() < 6.0
            })
        };
        assert!(dead(200.0), "the ground behind the barn is dead");
        assert!(!dead(100.0), "the ground in front of the barn is seen");
        let flank_seen = vertices.iter().all(|vertex| {
            !((vertex.position[2] - 200.0).abs() < 6.0 && (vertex.position[0] - 300.0).abs() < 8.0)
        });
        assert!(flank_seen, "ground far to the barn's flank stays seen");
    }
}
