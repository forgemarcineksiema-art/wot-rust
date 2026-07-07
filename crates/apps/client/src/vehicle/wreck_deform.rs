//! Runtime wreck deformation: a knocked-out hull is DENTED where it was penetrated. Uses the
//! (bake-only until now) `deform` kernel at death, once per wreck, on a per-instance copy of the
//! hull mesh — presentation only. The kernel's contract is preserved: it displaces vertices along
//! their own normals within a tolerance and never sees a hitbox, armor facet, or mount, so a
//! deformed wreck cannot change collision, armor, or module truth. The mesh copy is registered
//! through the same upload path as every other vehicle mesh (see `asset_catalog`).

use deform::{DeformationKind, DeformationSpec, deform};
use glam::Vec3;
use vehicle_geometry::GeometryMesh;

/// How deep a penetration dents the hull, and the tolerance that clamps the kernel — deep enough
/// to read as battle-beaten, shallow enough to stay a dent, not a hole.
const DENT_AMPLITUDE_M: f32 = 0.055;
const DENT_TOLERANCE_M: f32 = 0.07;
/// How far a dent reaches around its center.
const DENT_RADIUS_M: f32 = 0.35;
/// At most this many dents — the deepest/most recent penetrations. Bounds the per-death work.
const MAX_DENTS: usize = 3;

/// Build a dented copy of the hull for a wreck, denting at up to [`MAX_DENTS`] of its penetration
/// points (hull-local). Returns `None` when there is nothing to dent, so the caller keeps the
/// shared base mesh. Deterministic in `(base, penetrations, seed)`.
pub(crate) fn dented_hull(
    base: &GeometryMesh,
    penetrations_local: &[Vec3],
    seed: u64,
) -> Option<GeometryMesh> {
    if penetrations_local.is_empty() {
        return None;
    }
    let mut mesh = base.clone();
    for (index, &center) in penetrations_local.iter().take(MAX_DENTS).enumerate() {
        let spec = DeformationSpec {
            kind: DeformationKind::ShallowDent,
            center,
            radius: DENT_RADIUS_M,
            amplitude: DENT_AMPLITUDE_M,
            seed: seed ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
        };
        mesh = deform(&mesh, &spec, DENT_TOLERANCE_M);
    }
    Some(mesh)
}

#[cfg(test)]
mod tests {
    use vehicle_forge::authoritative_baked_vehicle;
    use vehicle_geometry::SubmeshKind;

    use super::*;

    fn t54_hull() -> GeometryMesh {
        authoritative_baked_vehicle(game_core::VehicleKind::T54_1951)
            .expect("bake")
            .submesh(SubmeshKind::Hull)
            .expect("hull")
            .mesh
            .clone()
    }

    #[test]
    fn no_penetrations_leaves_the_hull_shared() {
        assert!(dented_hull(&t54_hull(), &[], 7).is_none());
    }

    #[test]
    fn a_penetration_dents_the_hull_within_tolerance_and_never_moves_the_bounds_out() {
        let base = t54_hull();
        // Dent exactly at a real hull vertex, so the deformation is guaranteed to bite.
        let center = base.vertices()[0].position;
        let dent = dented_hull(&base, &[center], 42).expect("a dent");

        assert_ne!(dent, base, "the hull was actually deformed");
        // The deform kernel clamps each vertex to +/- tolerance along its normal (re-welding may
        // change the vertex count, so we compare BOUNDS, not per-vertex): the dent never balloons
        // the silhouette past the authored gameplay volume — collision-safe by construction.
        let (b0, b1) = (base.bounds().unwrap(), dent.bounds().unwrap());
        assert!(
            (b1.max - b0.max).max_element() <= DENT_TOLERANCE_M + 1.0e-3,
            "a dent never grows the hull's max bound"
        );
        assert!(
            (b0.min - b1.min).max_element() <= DENT_TOLERANCE_M + 1.0e-3,
            "a dent never grows the hull's min bound"
        );
    }

    #[test]
    fn denting_is_deterministic_in_seed_and_points() {
        let base = t54_hull();
        let points = [Vec3::new(0.3, 1.1, 1.2), Vec3::new(-0.4, 0.9, -1.0)];
        let a = dented_hull(&base, &points, 99).expect("dent a");
        let b = dented_hull(&base, &points, 99).expect("dent b");
        for (p, q) in a.vertices().iter().zip(b.vertices()) {
            assert_eq!(p.position, q.position, "same inputs dent identically");
        }
    }
}
