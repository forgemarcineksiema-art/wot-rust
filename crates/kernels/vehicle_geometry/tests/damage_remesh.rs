use game_core::{ApertureLobe, BreachContour};
use glam::{Vec2, Vec3};
use vehicle_geometry::{
    GeometryMesh, GeometryVertex, MaterialRole, MeshQualitySpec, SmoothingGroup,
    TopologyExpectation, remesh_aperture,
};

fn plate_grid() -> GeometryMesh {
    let mut vertices = Vec::new();
    for y in 0..=4 {
        for x in 0..=4 {
            vertices.push(
                GeometryVertex::new(
                    Vec3::new(x as f32 * 0.25 - 0.5, y as f32 * 0.25 - 0.5, 0.0),
                    Vec3::Z,
                    MaterialRole::RolledArmor,
                    SmoothingGroup::hard_edges(),
                )
                .with_uv0(Vec2::new(x as f32, y as f32)),
            );
        }
    }
    let mut indices = Vec::new();
    for y in 0..4_u32 {
        for x in 0..4_u32 {
            let a = y * 5 + x;
            let b = a + 1;
            let c = a + 5;
            let d = c + 1;
            indices.extend_from_slice(&[a, b, d, a, d, c]);
        }
    }
    GeometryMesh::new(vertices, indices)
}

fn lobe() -> ApertureLobe {
    ApertureLobe {
        entry_local: Vec3::ZERO,
        exit_local: Vec3::new(0.0, 0.0, -0.10),
        entry_normal_local: Vec3::Z,
        exit_normal_local: Vec3::NEG_Z,
        direction_local: Vec3::NEG_Z,
        thickness_m: 0.10,
        outer: BreachContour::new(0.14, 0.11, 0.27, 0.12),
        inner: BreachContour::new(0.18, 0.15, 0.31, 0.15),
        fracture_seed: 0x1234_5678,
    }
}

#[test]
fn local_remesh_is_deterministic_and_keeps_the_source_immutable() {
    let source = plate_grid();
    let original = source.clone();
    let first = remesh_aperture(&source, lobe()).expect("remesh");
    let second = remesh_aperture(&source, lobe()).expect("same remesh");
    assert_eq!(source, original);
    assert_eq!(first, second);
    assert_ne!(first.indices(), source.indices());
}

#[test]
fn local_remesh_opens_the_center_without_bad_topology() {
    let remeshed = remesh_aperture(&plate_grid(), lobe()).expect("remesh");
    for tri in remeshed.indices().chunks_exact(3) {
        let points =
            [tri[0], tri[1], tri[2]].map(|index| remeshed.vertices()[index as usize].position);
        assert!(!contains_origin(points), "no rebuilt steel triangle may close the aperture");
    }
    let report = remeshed.quality_report(MeshQualitySpec {
        topology: TopologyExpectation::Open,
        min_triangle_area: 1.0e-12,
        normal_tolerance: 1.0e-3,
    });
    assert_eq!(report.invalid_indices, 0);
    assert_eq!(report.degenerate_triangles, 0);
    assert_eq!(report.non_manifold_edges, 0);
    assert_eq!(report.inconsistent_winding_edges, 0);
    assert!(report.boundary_edges >= 32, "the contour is a real open boundary");
}

fn contains_origin(points: [Vec3; 3]) -> bool {
    let signs = (0..3).map(|index| {
        let a = points[index].truncate();
        let b = points[(index + 1) % 3].truncate();
        (b - a).perp_dot(-a)
    });
    let mut positive = false;
    let mut negative = false;
    for sign in signs {
        positive |= sign > 1.0e-6;
        negative |= sign < -1.0e-6;
    }
    !(positive && negative)
}
