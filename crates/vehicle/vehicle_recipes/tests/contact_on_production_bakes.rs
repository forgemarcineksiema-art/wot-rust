//! `MeshContactIndex` proven against REAL production bakes. The kernel's own unit tests cover
//! the machinery on synthetic quads; these three lived beside them until the recipes moved to
//! this crate (W4 F4b) — a dev-dependency back onto `vehicle_recipes` would compile
//! `vehicle_geometry` twice and split its types, so the tests that need a real casting live
//! with the recipes that bake one.

use glam::Vec3;
use vehicle_geometry::{MeshContactIndex, SubmeshKind};
use vehicle_recipes::bake_vehicle;

/// The BVH must agree with brute force on a real, many-triangle casting from every angle —
/// this is what proves the tree traversal never drops or reorders a hit.
#[test]
fn the_bvh_matches_brute_force_on_the_t54_hull() {
    let baked = bake_vehicle(game_core::VehicleKind::T54_1951).expect("bake");
    let hull = baked.submesh(SubmeshKind::Hull).expect("hull submesh");
    let index = MeshContactIndex::from_mesh(&hull.mesh, Vec3::ZERO);
    assert!(index.triangle_count() > 50, "a real hull has many triangles");

    // Fire inward from a shell of directions on a sphere around the hull.
    let mut checked = 0;
    for yaw_step in 0..12 {
        for pitch_step in -2..=2 {
            let yaw = yaw_step as f32 / 12.0 * std::f32::consts::TAU;
            let pitch = pitch_step as f32 * 0.3;
            let outward = Vec3::new(yaw.cos() * pitch.cos(), pitch.sin(), yaw.sin() * pitch.cos());
            let origin = Vec3::new(0.0, 1.0, 0.0) + outward * 8.0;
            let dir = -outward;
            let bvh = index.raycast(origin, dir, 20.0);
            let brute = index.raycast_brute(origin, dir, 20.0);
            match (bvh, brute) {
                (Some(a), Some(b)) => {
                    assert!(
                        a.position.distance(b.position) < 1.0e-4,
                        "BVH and brute-force hit points diverge: {a:?} vs {b:?}"
                    );
                    checked += 1;
                }
                (None, None) => {}
                (a, b) => panic!("BVH/brute disagree on hit-or-miss: {a:?} vs {b:?}"),
            }
        }
    }
    assert!(checked > 10, "the sweep should land real hits, got {checked}");
}

#[test]
fn a_patch_on_the_curved_turret_spans_several_faceted_triangles() {
    let baked = bake_vehicle(game_core::VehicleKind::T54_1951).expect("bake");
    let turret = baked.submesh(SubmeshKind::Turret).expect("turret submesh");
    let ring = baked.mounts().turret_ring.translation;
    let index = MeshContactIndex::from_mesh(&turret.mesh, ring);
    // Fire into the front of the cast dome from ahead at trunnion height.
    let y = baked.mounts().gun_trunnion.translation.y - ring.y;
    let contact =
        index.raycast(Vec3::new(0.0, y, 6.0), Vec3::NEG_Z, 12.0).expect("ray meets the dome");
    let patch = index.clip_patch(&contact, 0.25, 64);
    assert!(
        patch.triangle_count() >= 2,
        "a dome patch wraps >1 facet, got {}",
        patch.triangle_count()
    );
    // The facets face different ways — that is what "conformal" buys over a flat quad.
    let normal = |tri: usize| {
        let p = &patch.positions[tri * 3..tri * 3 + 3];
        (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero()
    };
    let first = normal(0);
    let bent = (1..patch.triangle_count()).any(|t| normal(t).dot(first) < 0.999);
    assert!(bent, "the patch bends across the casting instead of staying flat");
}

#[test]
fn the_patch_triangle_cap_is_respected() {
    let baked = bake_vehicle(game_core::VehicleKind::T54_1951).expect("bake");
    let hull = baked.submesh(SubmeshKind::Hull).expect("hull submesh");
    let index = MeshContactIndex::from_mesh(&hull.mesh, Vec3::ZERO);
    let contact =
        index.raycast(Vec3::new(0.0, 1.2, 8.0), Vec3::NEG_Z, 12.0).expect("ray meets the hull");
    // A large radius over a dense area, capped small.
    let patch = index.clip_patch(&contact, 1.5, 8);
    assert!(
        patch.triangle_count() <= 8,
        "the cap bounds the patch, got {}",
        patch.triangle_count()
    );
}
