//! The surface-mapping contract on `GeometryVertex`: a default vertex is triplanar with no authored
//! UV; `with_uv0` declares a parametric chart; and the welder must never fuse two vertices that want
//! different parametric UVs even when position, normal and smoothing match (a chart seam must hold),
//! while triplanar vertices still fuse on geometry alone.

use glam::{Vec2, Vec3};
use vehicle_geometry::{
    CLOSED_SMOOTH_MESH, GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup, SurfaceMapping,
};

fn vert(p: Vec3, n: Vec3) -> GeometryVertex {
    GeometryVertex::new(p, n, MaterialRole::CastArmor, SmoothingGroup(3))
}

#[test]
fn a_default_vertex_is_triplanar_with_no_uv() {
    let v = vert(Vec3::ZERO, Vec3::Y);
    assert_eq!(v.mapping, SurfaceMapping::Triplanar);
    assert_eq!(v.uv0, Vec2::ZERO);
}

#[test]
fn with_uv0_declares_a_parametric_chart() {
    let v = vert(Vec3::ZERO, Vec3::Y).with_uv0(Vec2::new(0.3, 0.7));
    assert_eq!(v.mapping, SurfaceMapping::ParametricUv);
    assert_eq!(v.uv0, Vec2::new(0.3, 0.7));
}

#[test]
fn the_welder_splits_parametric_vertices_with_different_uvs() {
    // Two triangles meeting at a coincident edge but on opposite sides of a UV chart seam: the shared
    // corners carry different UVs, so the welder must keep them distinct (the seam survives).
    let p0 = Vec3::ZERO;
    let p1 = Vec3::X;
    let n = Vec3::Z;
    let smooth = SmoothingGroup(5);
    let mk = |p, u| {
        GeometryVertex::new(p, n, MaterialRole::RolledArmor, smooth).with_uv0(Vec2::new(u, 0.0))
    };
    // Triangle A uses U=0 at p0; triangle B uses U=1 at the *same* p0 (chart wrap).
    let vertices = vec![
        mk(p0, 0.0),
        mk(p1, 0.0),
        mk(Vec3::new(0.0, 1.0, 0.0), 0.0),
        mk(p0, 1.0),
        mk(p1, 1.0),
        mk(Vec3::new(0.0, 1.0, 0.0), 1.0),
    ];
    let welded = GeometryMesh::new(vertices, vec![0, 1, 2, 3, 4, 5]).weld_and_smooth();
    // The two p0 corners differ only in UV, so they must not fuse.
    let at_origin = welded.vertices().iter().filter(|v| v.position == p0).count();
    assert_eq!(at_origin, 2, "a UV-seam corner must not weld shut");
}

#[test]
fn triplanar_vertices_still_fuse_on_geometry_alone() {
    // A smooth closed octahedron of triplanar vertices: coincident corners must fuse so it stays a
    // closed manifold (UV plays no part for triplanar geometry).
    let r = 1.0;
    let top = Vec3::new(0.0, r, 0.0);
    let bot = Vec3::new(0.0, -r, 0.0);
    let ring = [
        Vec3::new(r, 0.0, 0.0),
        Vec3::new(0.0, 0.0, r),
        Vec3::new(-r, 0.0, 0.0),
        Vec3::new(0.0, 0.0, -r),
    ];
    let mut verts = Vec::new();
    let mut idx = Vec::new();
    let push = |a: Vec3, b: Vec3, c: Vec3, list: &mut Vec<GeometryVertex>| {
        let base = list.len() as u32;
        for p in [a, b, c] {
            list.push(vert(p, p.normalize()));
        }
        base
    };
    for k in 0..4 {
        let (a, b) = (ring[k], ring[(k + 1) % 4]);
        let base = push(top, a, b, &mut verts);
        idx.extend_from_slice(&[base, base + 1, base + 2]);
        let base = push(bot, b, a, &mut verts);
        idx.extend_from_slice(&[base, base + 1, base + 2]);
    }
    let welded = GeometryMesh::new(verts, idx).weld_and_smooth();
    welded.validate_quality(CLOSED_SMOOTH_MESH).expect("triplanar octahedron is a closed manifold");
    assert_eq!(welded.vertices().len(), 6, "the 6 distinct corners fuse despite per-face copies");
}
