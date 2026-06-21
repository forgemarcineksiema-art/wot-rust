//! The shared mesh-quality contract: one renderer-neutral audit every generator can be tested
//! against. These lock the edge-accounting and finiteness rules described in the kernel program.

use glam::Vec3;
use vehicle_geometry::{
    GeometryMesh, GeometryVertex, MaterialRole, MeshQualityError, MeshQualitySpec, SmoothingGroup,
    TopologyExpectation,
};

const CLOSED: MeshQualitySpec = MeshQualitySpec {
    topology: TopologyExpectation::ClosedManifold,
    min_triangle_area: 1.0e-10,
    normal_tolerance: 1.0e-3,
};

const ANY: MeshQualitySpec = MeshQualitySpec {
    topology: TopologyExpectation::Any,
    min_triangle_area: 1.0e-10,
    normal_tolerance: 1.0e-3,
};

fn vert(p: Vec3) -> GeometryVertex {
    // A unit normal so the normal audit is satisfied by construction; geometry under test is
    // the position/index data.
    GeometryVertex::new(p, Vec3::Y, MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
}

/// A closed tetrahedron: four faces, every edge shared by exactly two oppositely wound triangles.
fn tetrahedron() -> GeometryMesh {
    let v = [
        vert(Vec3::new(0.0, 0.0, 0.0)),
        vert(Vec3::new(1.0, 0.0, 0.0)),
        vert(Vec3::new(0.0, 1.0, 0.0)),
        vert(Vec3::new(0.0, 0.0, 1.0)),
    ];
    // Outward-wound faces of the tetra.
    let indices = vec![0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3];
    GeometryMesh::new(v.to_vec(), indices)
}

/// A single flat quad (two triangles): open, with four boundary edges and one shared interior edge.
fn open_quad() -> GeometryMesh {
    let v = [
        vert(Vec3::new(0.0, 0.0, 0.0)),
        vert(Vec3::new(1.0, 0.0, 0.0)),
        vert(Vec3::new(1.0, 0.0, 1.0)),
        vert(Vec3::new(0.0, 0.0, 1.0)),
    ];
    let indices = vec![0, 1, 2, 0, 2, 3];
    GeometryMesh::new(v.to_vec(), indices)
}

#[test]
fn closed_tetrahedron_validates_as_a_closed_manifold() {
    let report = tetrahedron().validate_quality(CLOSED).expect("clean closed manifold");
    assert_eq!(report.triangles, 4);
    assert_eq!(report.boundary_edges, 0);
    assert_eq!(report.non_manifold_edges, 0);
    assert_eq!(report.inconsistent_winding_edges, 0);
    assert_eq!(report.degenerate_triangles, 0);
}

#[test]
fn open_quad_validates_as_open_but_clean() {
    let report = open_quad().validate_quality(ANY).expect("clean open mesh");
    assert_eq!(report.boundary_edges, 4);
    assert_eq!(report.non_manifold_edges, 0);
}

#[test]
fn open_quad_fails_closed_topology_with_boundary_edges() {
    let err = open_quad().validate_quality(CLOSED).unwrap_err();
    assert_eq!(err, MeshQualityError::UnexpectedBoundaryEdges(4));
}

#[test]
fn an_out_of_range_index_is_an_invalid_index() {
    let mut mesh = tetrahedron();
    let indices: Vec<u32> = mesh.indices().iter().copied().chain([99]).collect();
    // Append a degenerate-but-out-of-range triangle: indices count stays a multiple of three.
    let indices: Vec<u32> = indices.into_iter().chain([99, 99]).collect();
    mesh = GeometryMesh::new(mesh.vertices().to_vec(), indices);
    let err = mesh.validate_quality(ANY).unwrap_err();
    assert!(matches!(err, MeshQualityError::InvalidIndices(n) if n >= 3));
}

#[test]
fn a_trailing_index_remainder_counts_as_invalid() {
    let mut indices = tetrahedron().indices().to_vec();
    indices.push(0); // one extra index -> remainder of 1
    let mesh = GeometryMesh::new(tetrahedron().vertices().to_vec(), indices);
    let report = mesh.quality_report(ANY);
    assert!(report.invalid_indices >= 1);
}

#[test]
fn a_repeated_index_triangle_is_degenerate() {
    let v = [vert(Vec3::ZERO), vert(Vec3::X), vert(Vec3::Y)];
    let mesh = GeometryMesh::new(v.to_vec(), vec![0, 1, 1]);
    let report = mesh.quality_report(ANY);
    assert_eq!(report.degenerate_triangles, 1);
}

#[test]
fn a_zero_area_three_point_triangle_is_degenerate() {
    let v = [
        vert(Vec3::new(0.0, 0.0, 0.0)),
        vert(Vec3::new(1.0, 0.0, 0.0)),
        vert(Vec3::new(2.0, 0.0, 0.0)),
    ];
    let mesh = GeometryMesh::new(v.to_vec(), vec![0, 1, 2]);
    assert_eq!(mesh.quality_report(ANY).degenerate_triangles, 1);
}

#[test]
fn a_non_manifold_edge_used_by_three_triangles_is_flagged() {
    // A shared edge (0,1) used by three fans.
    let v = [
        vert(Vec3::new(0.0, 0.0, 0.0)),
        vert(Vec3::new(1.0, 0.0, 0.0)),
        vert(Vec3::new(0.0, 1.0, 0.0)),
        vert(Vec3::new(0.0, -1.0, 0.0)),
        vert(Vec3::new(0.0, 0.0, 1.0)),
    ];
    let indices = vec![0, 1, 2, 0, 1, 3, 0, 1, 4];
    let mesh = GeometryMesh::new(v.to_vec(), indices);
    assert!(mesh.quality_report(ANY).non_manifold_edges >= 1);
    assert_eq!(
        mesh.validate_quality(ANY).unwrap_err(),
        MeshQualityError::NonManifoldEdges(mesh.quality_report(ANY).non_manifold_edges),
    );
}

#[test]
fn a_same_direction_shared_edge_is_inconsistent_winding() {
    // Two triangles sharing edge 1->2 in the SAME direction (both list 1 then 2).
    let v = [
        vert(Vec3::new(0.0, 0.0, 0.0)),
        vert(Vec3::new(1.0, 0.0, 0.0)),
        vert(Vec3::new(1.0, 0.0, 1.0)),
        vert(Vec3::new(2.0, 0.0, 0.5)),
    ];
    let indices = vec![0, 1, 2, 1, 2, 3];
    let mesh = GeometryMesh::new(v.to_vec(), indices);
    assert_eq!(mesh.quality_report(ANY).inconsistent_winding_edges, 1);
}

#[test]
fn non_finite_position_normal_and_shade_are_flagged() {
    let mut bad_pos = vert(Vec3::new(f32::NAN, 0.0, 0.0));
    let report_pos = GeometryMesh::new(vec![bad_pos, vert(Vec3::X), vert(Vec3::Y)], vec![0, 1, 2])
        .quality_report(ANY);
    assert_eq!(report_pos.non_finite_vertices, 1);

    bad_pos = vert(Vec3::ZERO);
    bad_pos.normal = Vec3::new(f32::INFINITY, 0.0, 0.0);
    let report_normal =
        GeometryMesh::new(vec![bad_pos, vert(Vec3::X), vert(Vec3::Y)], vec![0, 1, 2])
            .quality_report(ANY);
    assert_eq!(report_normal.non_finite_vertices, 1);

    let mut bad_shade = vert(Vec3::ZERO);
    bad_shade.surface_shade = f32::NAN;
    let report_shade =
        GeometryMesh::new(vec![bad_shade, vert(Vec3::X), vert(Vec3::Y)], vec![0, 1, 2])
            .quality_report(ANY);
    assert_eq!(report_shade.non_finite_vertices, 1);
}

#[test]
fn a_non_unit_normal_is_flagged() {
    let mut v = vert(Vec3::ZERO);
    v.normal = Vec3::new(0.0, 2.0, 0.0);
    let report =
        GeometryMesh::new(vec![v, vert(Vec3::X), vert(Vec3::Y)], vec![0, 1, 2]).quality_report(ANY);
    assert_eq!(report.non_unit_normals, 1);
}
