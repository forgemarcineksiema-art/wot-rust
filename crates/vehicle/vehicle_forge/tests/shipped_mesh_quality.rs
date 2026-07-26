//! The mesh-quality contract on the geometry the GAME ships.
//!
//! `vehicle_geometry::tests::fleet_mesh_quality` audits the procedural recipes, but it cannot see
//! `vehicle_build`, so the one vehicle that does NOT bake through those recipes — the hybrid T-54,
//! the benchmark the whole fleet is judged against — sat outside every mesh-quality gate. This
//! file closes that seam by auditing whatever [`authoritative_baked_vehicle`] resolves to, which
//! is by definition the mesh the garage, the battle and the artifact all draw.
//!
//! Keep both gates. The procedural one catches a recipe regression on a vehicle the seam does not
//! route yet; this one catches the source that actually reaches the screen.

use game_core::VehicleKind;
use vehicle_forge::authoritative_baked_vehicle;
use vehicle_geometry::OPEN_OR_CLOSED_MESH;

/// No winding debt survives on the shipped meshes. This mirrored
/// `vehicle_geometry::tests::fleet_mesh_quality::RECORDED_WINDING_CEILING`, whose last entry was
/// the IS-3 hull's overlapping sponson-underside windows; that overlap turned out to be forced
/// geometry rather than a shape decision and went to zero. Debt gets booked in THAT list, with
/// its reason — here the bar is simply zero, on the meshes the game actually draws.
#[test]
fn every_shipped_vehicle_mesh_obeys_the_quality_contract() {
    for kind in VehicleKind::PLAYABLE {
        let baked =
            authoritative_baked_vehicle(kind).unwrap_or_else(|e| panic!("{kind:?} bakes: {e}"));
        let mut winding = 0;
        for submesh in baked.submeshes() {
            let report = submesh.mesh.quality_report(OPEN_OR_CLOSED_MESH);
            let where_ = format!("shipped {kind:?} {:?}", submesh.kind);
            assert_eq!(report.invalid_indices, 0, "{where_}: indices point at no vertex");
            assert_eq!(report.non_finite_vertices, 0, "{where_}: non-finite vertex data");
            assert_eq!(report.non_manifold_edges, 0, "{where_}: non-manifold edges");
            assert_eq!(report.degenerate_triangles, 0, "{where_}: zero-area triangles");
            assert_eq!(report.non_unit_normals, 0, "{where_}: normals are not unit length");
            winding += report.inconsistent_winding_edges;
        }
        assert_eq!(
            winding, 0,
            "shipped {kind:?}: {winding} inconsistently wound edges across its submeshes — those \
             faces light backwards or vanish under backface culling"
        );
    }
}

/// The seam this file exists for: at least one playable vehicle must resolve to something the
/// procedural gate never sees, or the two gates are the same gate and one of them is dead weight.
#[test]
fn at_least_one_shipped_vehicle_differs_from_its_procedural_recipe() {
    let differs = VehicleKind::PLAYABLE.into_iter().any(|kind| {
        let shipped = authoritative_baked_vehicle(kind).expect("shipped bake");
        let procedural = vehicle_geometry::bake_vehicle(kind).expect("procedural bake");
        shipped.deterministic_hash() != procedural.deterministic_hash()
    });
    assert!(
        differs,
        "no playable vehicle leaves the procedural path — this gate no longer covers anything the \
         vehicle_geometry gate misses"
    );
}
