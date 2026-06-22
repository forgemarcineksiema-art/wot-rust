//! The vertex-weld identity behind [`GeometryMesh::weld_and_smooth`](crate::GeometryMesh).
//!
//! Two vertices fuse only when they agree on everything that must stay shared. Hard edges fold the
//! (quantised) normal into the key so coincident faces with different normals keep their crisp seam;
//! smooth groups omit it so corners average into round castings. The mapping mode and (quantised)
//! parametric `uv0` are always part of the key, so a textured chart boundary never welds shut even
//! when position, normal and smoothing match.

use glam::{Vec2, Vec3};

use crate::mesh::{GeometryVertex, MaterialRole, SmoothingGroup, SurfaceMapping};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct WeldKey {
    position: [i64; 3],
    smoothing: SmoothingGroup,
    material: MaterialRole,
    normal: Option<[i64; 3]>,
    mapping: SurfaceMapping,
    uv0: Option<[i64; 2]>,
}

pub(crate) fn weld_key(vertex: &GeometryVertex) -> WeldKey {
    let normal =
        (vertex.smoothing == SmoothingGroup::hard_edges()).then(|| quantize(vertex.normal));
    // Only parametric charts carry a meaningful UV; triplanar vertices share the zero UV, so they
    // still fuse on position/normal alone.
    let uv0 = (vertex.mapping == SurfaceMapping::ParametricUv).then(|| quantize_uv(vertex.uv0));
    WeldKey {
        position: quantize(vertex.position),
        smoothing: vertex.smoothing,
        material: vertex.material,
        normal,
        mapping: vertex.mapping,
        uv0,
    }
}

/// Snap a UV to the same sub-unit grid so coincident parametric corners with equal UVs still weld.
fn quantize_uv(value: Vec2) -> [i64; 2] {
    const WELD_SCALE: f32 = 4096.0;
    [(value.x * WELD_SCALE).round() as i64, (value.y * WELD_SCALE).round() as i64]
}

/// Snap to a sub-millimetre grid so vertices the kernel meant to share a position weld together
/// despite float noise, without merging genuinely separate detail.
fn quantize(value: Vec3) -> [i64; 3] {
    const WELD_SCALE: f32 = 4096.0;
    [
        (value.x * WELD_SCALE).round() as i64,
        (value.y * WELD_SCALE).round() as i64,
        (value.z * WELD_SCALE).round() as i64,
    ]
}
