use std::collections::HashSet;

use game_core::{ApertureLobe, BreachContour};
use glam::{Mat3, Vec2, Vec3};
use vehicle_geometry::{
    GeometryMesh, GeometryVertex, MaterialRole, MeshQualitySpec, SmoothingGroup,
    TopologyExpectation, remesh_aperture,
};

/// Regular grid sheet over `[-half, half]²` with an authored height field and normals; every
/// remesh case below is this sheet shaped into the armor feature under test.
fn grid_mesh(
    half: f32,
    steps: u32,
    height: impl Fn(f32, f32) -> f32,
    normal: impl Fn(f32, f32) -> Vec3,
) -> GeometryMesh {
    let mut vertices = Vec::new();
    for y in 0..=steps {
        for x in 0..=steps {
            let px = x as f32 / steps as f32 * 2.0 * half - half;
            let py = y as f32 / steps as f32 * 2.0 * half - half;
            vertices.push(
                GeometryVertex::new(
                    Vec3::new(px, py, height(px, py)),
                    normal(px, py),
                    MaterialRole::RolledArmor,
                    SmoothingGroup::hard_edges(),
                )
                .with_uv0(Vec2::new(px, py)),
            );
        }
    }
    let mut indices = Vec::new();
    let stride = steps + 1;
    for y in 0..steps {
        for x in 0..steps {
            let a = y * stride + x;
            let b = a + 1;
            let c = a + stride;
            let d = c + 1;
            indices.extend_from_slice(&[a, b, d, a, d, c]);
        }
    }
    GeometryMesh::new(vertices, indices)
}

fn plate_grid() -> GeometryMesh {
    grid_mesh(0.5, 4, |_, _| 0.0, |_, _| Vec3::Z)
}

/// A welded joint: the seam column sits proud of both plates, a tent-profile bead along `x = 0`.
const BEAD_HEIGHT: f32 = 0.015;

fn weld_bead_plate() -> GeometryMesh {
    let bead = |x: f32, _y: f32| {
        if x.abs() < 1.0e-6 { BEAD_HEIGHT } else { 0.0 }
    };
    grid_mesh(0.5, 8, bead, |_, _| Vec3::Z)
}

/// A cast turret cheek: spherical cap of the given radius, apex at the origin, bulging toward +Z.
const CHEEK_RADIUS: f32 = 0.7;

fn cheek_height(x: f32, y: f32) -> f32 {
    (CHEEK_RADIUS * CHEEK_RADIUS - x * x - y * y).sqrt() - CHEEK_RADIUS
}

fn cast_cheek() -> GeometryMesh {
    grid_mesh(0.4, 12, cheek_height, |x, y| {
        Vec3::new(x, y, cheek_height(x, y) + CHEEK_RADIUS) / CHEEK_RADIUS
    })
}

/// A folded lip: the front plate plus a parallel back-facing sheet 60 mm behind it, well inside
/// the aperture's search radius. Only the front surface may take the hole.
const UNDERCUT_DEPTH: f32 = -0.06;

fn undercut_plate() -> GeometryMesh {
    let front = grid_mesh(0.5, 8, |_, _| 0.0, |_, _| Vec3::Z);
    let mut vertices = front.vertices().to_vec();
    let mut indices = front.indices().to_vec();
    let back = grid_mesh(0.5, 8, |_, _| UNDERCUT_DEPTH, |_, _| Vec3::NEG_Z);
    let offset = vertices.len() as u32;
    vertices.extend_from_slice(back.vertices());
    for tri in back.indices().chunks_exact(3) {
        // Reverse the winding so the sheet truly faces -Z, like a fender fold seen from behind.
        indices.extend_from_slice(&[tri[0] + offset, tri[2] + offset, tri[1] + offset]);
    }
    GeometryMesh::new(vertices, indices)
}

fn rotated(mesh: &GeometryMesh, basis: Mat3) -> GeometryMesh {
    let vertices = mesh
        .vertices()
        .iter()
        .map(|vertex| GeometryVertex {
            position: basis * vertex.position,
            normal: basis * vertex.normal,
            ..*vertex
        })
        .collect();
    GeometryMesh::new(vertices, mesh.indices().to_vec())
}

fn lobe() -> ApertureLobe {
    lobe_at(Vec3::ZERO, Vec3::Z)
}

fn lobe_at(entry: Vec3, normal: Vec3) -> ApertureLobe {
    ApertureLobe {
        entry_local: entry,
        exit_local: entry - normal * 0.10,
        entry_normal_local: normal,
        exit_normal_local: -normal,
        direction_local: -normal,
        thickness_m: 0.10,
        outer: BreachContour::new(0.14, 0.11, 0.27, 0.12),
        inner: BreachContour::new(0.18, 0.15, 0.31, 0.15),
        fracture_seed: 0x1234_5678,
    }
}

/// The acceptance gates shared by every remesh case: clean topology with a real open contour.
fn assert_clean_open(remeshed: &GeometryMesh) {
    let report = remeshed.quality_report(MeshQualitySpec {
        topology: TopologyExpectation::Open,
        min_triangle_area: 1.0e-12,
        normal_tolerance: 1.0e-3,
    });
    assert_eq!(report.invalid_indices, 0);
    assert_eq!(report.degenerate_triangles, 0);
    assert_eq!(report.non_manifold_edges, 0);
    assert_eq!(report.inconsistent_winding_edges, 0);
}

/// "No whole large triangle may vanish": every source triangle entirely outside the local patch
/// must survive verbatim. Source vertices are a prefix of the remeshed vertices, so surviving
/// triangles keep their exact index triples.
fn assert_far_triangles_survive(
    source: &GeometryMesh,
    remeshed: &GeometryMesh,
    entry: Vec3,
    patch_radius: f32,
) {
    let surviving: HashSet<[u32; 3]> =
        remeshed.indices().chunks_exact(3).map(|tri| [tri[0], tri[1], tri[2]]).collect();
    let mut checked = 0_usize;
    for tri in source.indices().chunks_exact(3) {
        let far = tri.iter().all(|index| {
            source.vertices()[*index as usize].position.distance(entry) > patch_radius
        });
        if far {
            assert!(
                surviving.contains(&[tri[0], tri[1], tri[2]]),
                "a source triangle outside the patch was removed"
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "the case must actually have far triangles to protect");
}

fn assert_deterministic(source: &GeometryMesh, lobe: ApertureLobe) -> GeometryMesh {
    let original = source.clone();
    let first = remesh_aperture(source, lobe).expect("remesh");
    let second = remesh_aperture(source, lobe).expect("same remesh");
    assert_eq!(*source, original, "the shared source mesh must stay immutable");
    assert_eq!(first, second, "the rebuilt patch must hash stably");
    first
}

#[test]
fn local_remesh_is_deterministic_and_keeps_the_source_immutable() {
    let source = plate_grid();
    let remeshed = assert_deterministic(&source, lobe());
    assert_ne!(remeshed.indices(), source.indices());
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

#[test]
fn a_hole_across_a_weld_seam_rides_the_bead() {
    let source = weld_bead_plate();
    let remeshed = assert_deterministic(&source, lobe());
    assert_clean_open(&remeshed);
    assert_far_triangles_survive(&source, &remeshed, Vec3::ZERO, 0.35);
    let ring_peak = remeshed.vertices()[source.vertex_count()..]
        .iter()
        .map(|vertex| vertex.position.z)
        .fold(f32::MIN, f32::max);
    assert!(
        ring_peak > BEAD_HEIGHT * 0.5,
        "the contour crossing the seam must climb the weld bead instead of \
         flattening onto the plate plane (peak {ring_peak})"
    );
}

#[test]
fn a_hole_in_a_cast_cheek_stays_on_the_curved_surface() {
    let source = cast_cheek();
    let remeshed = assert_deterministic(&source, lobe());
    assert_clean_open(&remeshed);
    assert_far_triangles_survive(&source, &remeshed, Vec3::ZERO, 0.35);
    for vertex in &remeshed.vertices()[source.vertex_count()..] {
        let expected = cheek_height(vertex.position.x, vertex.position.y);
        assert!(
            (vertex.position.z - expected).abs() < 0.006,
            "a contour vertex left the casting surface by {} m at {:?}",
            (vertex.position.z - expected).abs(),
            vertex.position
        );
    }
}

#[test]
fn an_undercut_back_surface_is_never_grabbed() {
    let source = undercut_plate();
    let front_triangles = source.indices().len() / 2;
    let remeshed = assert_deterministic(&source, lobe());
    assert_clean_open(&remeshed);
    // Every back-sheet triangle survives verbatim: the aperture may only open the front plate.
    let surviving: HashSet<[u32; 3]> =
        remeshed.indices().chunks_exact(3).map(|tri| [tri[0], tri[1], tri[2]]).collect();
    for tri in source.indices()[front_triangles..].chunks_exact(3) {
        assert!(
            surviving.contains(&[tri[0], tri[1], tri[2]]),
            "a back-facing fold triangle was rebuilt by a front-surface aperture"
        );
    }
    for vertex in &remeshed.vertices()[source.vertex_count()..] {
        assert!(
            vertex.position.z > UNDERCUT_DEPTH * 0.25,
            "a contour vertex sank toward the undercut sheet at {:?}",
            vertex.position
        );
    }
}

#[test]
fn an_off_axis_mantlet_hole_opens_cleanly() {
    let basis = Mat3::from_rotation_x(25_f32.to_radians());
    let source = rotated(&cast_cheek(), basis);
    let aperture = lobe_at(Vec3::ZERO, basis * Vec3::Z);
    let remeshed = assert_deterministic(&source, aperture);
    assert_clean_open(&remeshed);
    assert_far_triangles_survive(&source, &remeshed, Vec3::ZERO, 0.35);
    let inverse = basis.transpose();
    for vertex in &remeshed.vertices()[source.vertex_count()..] {
        let local = inverse * vertex.position;
        let expected = cheek_height(local.x, local.y);
        assert!(
            (local.z - expected).abs() < 0.006,
            "an off-axis contour vertex left the mantlet surface by {} m",
            (local.z - expected).abs()
        );
    }
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
