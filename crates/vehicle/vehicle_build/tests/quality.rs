//! Geometry quality gates for the hybrid T-54: the baked mesh must be free of degenerate triangles,
//! wind its faces outward, and stay finite at every LOD tier. These lock the Stage 3 forms against
//! regressions that a triangle count alone would miss.

use glam::Vec3;
use vehicle_build::t54_description;
use vehicle_geometry::{BakedVehicle, GeometryMesh, LodLevel, reduce_vehicle};

fn lod_asset(lod: LodLevel) -> BakedVehicle {
    let built = t54_description().build_lod(lod);
    match lod {
        LodLevel::Lod0 => built,
        other => reduce_vehicle(&built, other),
    }
}

fn triangles(mesh: &GeometryMesh) -> impl Iterator<Item = [Vec3; 3]> + '_ {
    let v = mesh.vertices();
    mesh.indices().chunks_exact(3).map(move |t| {
        [v[t[0] as usize].position, v[t[1] as usize].position, v[t[2] as usize].position]
    })
}

#[test]
fn no_degenerate_triangles_at_any_lod() {
    for lod in [LodLevel::Lod0, LodLevel::Lod1, LodLevel::Lod2] {
        let baked = lod_asset(lod);
        for submesh in baked.submeshes() {
            for [a, b, c] in triangles(&submesh.mesh) {
                let area = 0.5 * (b - a).cross(c - a).length();
                assert!(
                    area > 1.0e-7,
                    "{:?} {:?} has a degenerate triangle (area {area:.2e})",
                    lod,
                    submesh.kind
                );
            }
        }
    }
}

#[test]
fn positions_and_normals_stay_finite_at_any_lod() {
    for lod in [LodLevel::Lod0, LodLevel::Lod1, LodLevel::Lod2] {
        let baked = lod_asset(lod);
        for submesh in baked.submeshes() {
            for v in submesh.mesh.vertices() {
                assert!(v.position.is_finite(), "{:?} {:?} non-finite position", lod, submesh.kind);
                assert!(v.normal.is_finite(), "{:?} {:?} non-finite normal", lod, submesh.kind);
            }
        }
    }
}

#[test]
fn faces_wind_outward() {
    // Each triangle's geometric normal (b-a)x(c-a) should agree with the outward vertex normals the
    // generators authored. A solid majority must agree, or winding has been flipped somewhere.
    let baked = lod_asset(LodLevel::Lod0);
    for submesh in baked.submeshes() {
        let v = submesh.mesh.vertices();
        let mut agree = 0usize;
        let mut total = 0usize;
        for t in submesh.mesh.indices().chunks_exact(3) {
            let (i, j, k) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let face = (v[j].position - v[i].position).cross(v[k].position - v[i].position);
            let outward = v[i].normal + v[j].normal + v[k].normal;
            if face.length_squared() > 1.0e-12 && outward.length_squared() > 1.0e-12 {
                total += 1;
                if face.dot(outward) > 0.0 {
                    agree += 1;
                }
            }
        }
        let ratio = agree as f32 / total.max(1) as f32;
        assert!(
            ratio > 0.97,
            "{:?} winds outward for only {:.1}% of faces",
            submesh.kind,
            ratio * 100.0
        );
    }
}

#[test]
fn the_lod0_bake_stays_within_the_class_triangle_budget() {
    let total: usize =
        lod_asset(LodLevel::Lod0).submeshes().iter().map(|s| s.mesh.triangle_count()).sum();
    assert!(
        total < vehicle_build::MEDIUM_LOD0_TRI_BUDGET,
        "LOD0 {total} exceeds the medium budget {}",
        vehicle_build::MEDIUM_LOD0_TRI_BUDGET
    );
}
