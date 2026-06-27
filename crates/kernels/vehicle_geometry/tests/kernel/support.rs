use std::collections::HashMap;

use glam::Vec3;
use vehicle_geometry::GeometryMesh;

/// For a convex solid centred near the average of its vertices, an outward-wound triangle has a
/// geometric normal pointing away from that centre.
pub(crate) fn all_faces_point_outward(mesh: &GeometryMesh) -> bool {
    let verts = mesh.vertices();
    let centre =
        verts.iter().map(|v| v.position).fold(Vec3::ZERO, |a, b| a + b) / verts.len() as f32;
    mesh.indices().chunks_exact(3).all(|tri| {
        let a = verts[tri[0] as usize].position;
        let b = verts[tri[1] as usize].position;
        let c = verts[tri[2] as usize].position;
        let normal = (b - a).cross(c - a);
        let face_centre = (a + b + c) / 3.0;
        normal.dot(face_centre - centre) > 0.0
    })
}

/// For every position where the raw faces disagreed, the welded mesh must hold a single vertex
/// whose normal is the normalised sum of those faces' normals.
pub(crate) fn assert_smoothed_to_face_average(raw: &GeometryMesh, welded: &GeometryMesh) {
    let welded_normal: HashMap<[i64; 3], Vec3> =
        welded.vertices().iter().map(|v| (quantize_position(v.position), v.normal)).collect();

    let mut checked_a_blend = false;
    for (position, normals) in face_normals_by_position(raw) {
        let first = normals[0];
        if normals.iter().all(|n| (*n - first).length() < 1.0e-4) {
            continue;
        }
        checked_a_blend = true;
        let average = normals.iter().copied().sum::<Vec3>().normalize_or_zero();
        let got = welded_normal.get(&position).expect("welded vertex at corner position");
        assert!(
            (*got - average).length() < 1.0e-3,
            "welded normal {got:?} at {position:?} is not the face average {average:?}"
        );
    }
    assert!(checked_a_blend, "test prism should expose at least one multi-normal corner");
}

pub(crate) fn has_multi_normal_position(mesh: &GeometryMesh) -> bool {
    normals_by_position(mesh).values().any(|normals| {
        let first = normals[0];
        normals.iter().any(|n| (*n - first).length() > 1.0e-4)
    })
}

fn face_normals_by_position(mesh: &GeometryMesh) -> HashMap<[i64; 3], Vec<Vec3>> {
    let mut by_position: HashMap<[i64; 3], Vec<Vec3>> = HashMap::new();
    let verts = mesh.vertices();
    for tri in mesh.indices().chunks_exact(3) {
        let a = verts[tri[0] as usize].position;
        let b = verts[tri[1] as usize].position;
        let c = verts[tri[2] as usize].position;
        let normal = (b - a).cross(c - a).normalize_or_zero();
        for index in tri {
            by_position
                .entry(quantize_position(verts[*index as usize].position))
                .or_default()
                .push(normal);
        }
    }
    by_position
}

fn normals_by_position(mesh: &GeometryMesh) -> HashMap<[i64; 3], Vec<Vec3>> {
    let mut by_position: HashMap<[i64; 3], Vec<Vec3>> = HashMap::new();
    for vertex in mesh.vertices() {
        by_position.entry(quantize_position(vertex.position)).or_default().push(vertex.normal);
    }
    by_position
}

/// Quantise a position to a coarse grid so coincident-but-noisy vertices group together.
fn quantize_position(point: Vec3) -> [i64; 3] {
    const SCALE: f32 = 1024.0;
    [
        (point.x * SCALE).round() as i64,
        (point.y * SCALE).round() as i64,
        (point.z * SCALE).round() as i64,
    ]
}
