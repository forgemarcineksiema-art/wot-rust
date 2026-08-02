//! THE MESH-QUALITY CONTRACT, RUN OVER THE FLEET.
//!
//! `vehicle_geometry::quality` has always defined what a valid mesh is, and `mesh_quality.rs` has
//! always proved the audit itself is right — on tetrahedra and quads. Nothing ran it over a
//! *vehicle*. So seven of eight guns shipped with 24–28 inconsistently wound edges: the Forge
//! Studio report printed `DEFECTS`, every ratio, budget and silhouette gate stayed green, and no
//! test failed. A contract nobody runs on the real thing is a document, not a gate.
//!
//! This file is that gate. It is deliberately blunt and total: every submesh of every vehicle,
//! every defect class the audit can name.

use game_core::VehicleKind;
use vehicle_geometry::{BakedVehicle, MeshQualityReport, OPEN_OR_CLOSED_MESH, SubmeshKind};
use vehicle_recipes::bake_vehicle;

/// Winding defects that exist TODAY and are recorded rather than asserted away, in the
/// FLOOR/TARGET form the art-direction programme uses: the count is a CEILING, so the defect can
/// only ever shrink, and it is printed on every run so it cannot be forgotten.
///
/// **The list is EMPTY: the whole fleet winds consistently.** The last entry was the IS-3 hull's
/// 2 edges, booked as a shape decision at the pike/tub-step junction. It was not one — the fold,
/// the tub step corner and the step corner are exactly collinear (both pike planes pass through
/// the fold and share the plan sweep), so the underside boundary order is forced by the geometry.
/// Walking that boundary instead of fanning across it tiles the region once and the debt went to
/// zero; `is3_hull.rs` locks the collinearity so the walk stays derivable.
///
/// Keep the mechanism, not the entry. The next real debt gets booked here with its reason.
const RECORDED_WINDING_CEILING: &[(VehicleKind, SubmeshKind, usize)] = &[];

fn ceiling(kind: VehicleKind, sub: SubmeshKind) -> usize {
    RECORDED_WINDING_CEILING
        .iter()
        .find(|(k, s, _)| *k == kind && *s == sub)
        .map_or(0, |(_, _, allowed)| *allowed)
}

fn audit(vehicle: &BakedVehicle, source: &str) {
    let kind = vehicle.kind();
    for submesh in vehicle.submeshes() {
        let report: MeshQualityReport = submesh.mesh.quality_report(OPEN_OR_CLOSED_MESH);
        let where_ = format!("{source} {kind:?} {:?}", submesh.kind);

        // These are never acceptable at any count: they mean the mesh is not a surface.
        assert_eq!(
            report.invalid_indices, 0,
            "{where_}: {} indices point at no vertex",
            report.invalid_indices
        );
        assert_eq!(
            report.non_finite_vertices, 0,
            "{where_}: {} vertices carry NaN/inf position, normal or shade",
            report.non_finite_vertices
        );
        assert_eq!(
            report.non_manifold_edges, 0,
            "{where_}: {} edges are shared by three or more triangles",
            report.non_manifold_edges
        );
        assert_eq!(
            report.degenerate_triangles, 0,
            "{where_}: {} triangles have no area — they shade as black slivers and break tangents",
            report.degenerate_triangles
        );
        assert_eq!(
            report.non_unit_normals, 0,
            "{where_}: {} vertex normals are not unit length",
            report.non_unit_normals
        );

        // Winding is the one class with recorded debt. A face wound against its neighbours has an
        // inward normal: it lights backwards, or vanishes under backface culling.
        let allowed = ceiling(kind, submesh.kind);
        assert!(
            report.inconsistent_winding_edges <= allowed,
            "{where_}: {} inconsistently wound edges, past its recorded ceiling {allowed} — \
             faces are wound against their neighbours and light backwards",
            report.inconsistent_winding_edges
        );
        if report.inconsistent_winding_edges > 0 {
            println!(
                "MESH DEBT {where_}: {} inconsistently wound edges (ceiling {allowed}, target 0)",
                report.inconsistent_winding_edges
            );
        }
    }
}

/// The procedural recipes: every vehicle the lineup bakes, including the test-only prototype.
#[test]
fn every_procedural_vehicle_mesh_obeys_the_quality_contract() {
    for kind in VehicleKind::ALL {
        let baked = bake_vehicle(kind).unwrap_or_else(|e| panic!("{kind:?} should bake: {e}"));
        audit(&baked, "procedural");
    }
}

/// A recorded ceiling that no longer bites is a lie about the state of the fleet: it reads as
/// "this is still broken" long after someone fixed it. Every entry must still be reached.
#[test]
fn every_recorded_winding_ceiling_is_still_earned() {
    for (kind, sub, allowed) in RECORDED_WINDING_CEILING {
        let baked = bake_vehicle(*kind).expect("recorded vehicle bakes");
        let submesh = baked
            .submesh(*sub)
            .unwrap_or_else(|| panic!("{kind:?} has no {sub:?} submesh to record debt against"));
        let measured = submesh.mesh.quality_report(OPEN_OR_CLOSED_MESH).inconsistent_winding_edges;
        assert_eq!(
            measured, *allowed,
            "{kind:?} {sub:?} now measures {measured} inconsistently wound edges, not the \
             recorded {allowed} — if this is a FIX, drop the entry; if it is a regression, the \
             other gate already told you"
        );
    }
}
