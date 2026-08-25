//! Part importance and the shared clustering reducer behind level-of-detail.
//!
//! Reduction is not uniform across a vehicle: a part that carries the silhouette or bears a mount
//! must keep its shape far longer than a bolt-on detail. So each part declares a [`PartImportance`]
//! that scales the clustering cell — silhouette and mount-critical parts cluster on a *finer* grid
//! (they shed fewer triangles), detail parts on a coarser one. The clustering itself is the
//! Rossignac centroid collapse, and a post-reduction [`audit_reduction`] proves the tier never
//! expanded the silhouette, dropped a required group to nothing, or moved a mount frame.

use std::collections::HashMap;

use glam::{Vec2, Vec3};

use crate::mesh::{GeometryVertex, MaterialRole, SmoothingGroup, SurfaceMapping};
use crate::{BakedVehicle, GeometryMesh, MeshBounds};

/// How aggressively a part may be reduced. Mirrors the authoring `PartLod` policy, but expresses the
/// *reduction* consequence: how fine a clustering grid the part is held to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartImportance {
    /// Defines the recognisable outline — reduced gently.
    Silhouette,
    /// Bears a mount/pose frame — reduced gently and never dropped.
    MountCritical,
    /// Bolt-on detail — reduced hardest (where it survives the tier at all).
    Detail,
}

impl PartImportance {
    /// Multiplier on the tier's base clustering cell. Below 1.0 means a finer grid (less collapse);
    /// above 1.0 a coarser grid (more collapse). Silhouette and mount-critical share the fine scale.
    pub fn cell_scale(self) -> f32 {
        match self {
            Self::Silhouette | Self::MountCritical => 0.62,
            Self::Detail => 1.4,
        }
    }
}

/// Cluster `mesh` against a uniform grid of `cell` metres (per material + smoothing group, so hard
/// edges and material boundaries are preserved) and re-weld for clean normals. The shared reducer
/// behind both the global and the part-aware LOD paths.
pub fn reduce_mesh(mesh: &GeometryMesh, cell: f32) -> GeometryMesh {
    cluster(mesh, cell.max(1.0e-4)).weld_and_smooth()
}

/// Why a reduced tier failed its post-reduction audit against the LOD0 base.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum LodAuditError {
    #[error("submesh {kind:?} expanded beyond the LOD0 bounds by {overshoot:.4} m")]
    BoundsExpanded { kind: crate::SubmeshKind, overshoot: f32 },
    #[error("required submesh {kind:?} collapsed to zero triangles")]
    RequiredPartCollapsed { kind: crate::SubmeshKind },
    #[error("mount frames moved during reduction")]
    MountFramesMoved,
}

/// Audit a reduced bake against its LOD0 base: clustering may only pull the silhouette inward, never
/// drop a group that LOD0 carried, and never touch the (geometry-independent) mount frames.
pub fn audit_reduction(base: &BakedVehicle, reduced: &BakedVehicle) -> Result<(), LodAuditError> {
    if base.mounts() != reduced.mounts() {
        return Err(LodAuditError::MountFramesMoved);
    }
    for sub in base.submeshes() {
        let Some(out) = reduced.submeshes().iter().find(|s| s.kind == sub.kind) else {
            return Err(LodAuditError::RequiredPartCollapsed { kind: sub.kind });
        };
        if out.mesh.triangle_count() == 0 {
            return Err(LodAuditError::RequiredPartCollapsed { kind: sub.kind });
        }
        if let (Some(b), Some(o)) = (sub.mesh.bounds(), out.mesh.bounds()) {
            let overshoot = expansion(&b, &o);
            if overshoot > 1.0e-3 {
                return Err(LodAuditError::BoundsExpanded { kind: sub.kind, overshoot });
            }
        }
    }
    Ok(())
}

/// How far `inner` pokes outside `outer` on its worst axis (0 if fully contained).
fn expansion(outer: &MeshBounds, inner: &MeshBounds) -> f32 {
    let over_max = (inner.max - outer.max).max_element();
    let over_min = (outer.min - inner.min).max_element();
    over_max.max(over_min).max(0.0)
}

struct Cluster {
    position: Vec3,
    normal: Vec3,
    shade: f32,
    count: f32,
    material: MaterialRole,
    smoothing: SmoothingGroup,
    // The parametric chart of the FIRST vertex to land in this cell. A cell is per (grid, material,
    // smoothing) and smaller than a chart, so its vertices share one mapping and near-identical uv0
    // — averaging would smear a seam, so the representative simply inherits the first. Without this,
    // rebuilding the representative through `GeometryVertex::new` reset every LOD vertex to
    // `Triplanar`/zero, so plates, wheels, track and fenders that carry an authored uv0 at LOD0 fell
    // to triplanar projection at distance and lost their tangent-space normal detail — a visible
    // mapping pop at the LOD switch.
    uv0: Vec2,
    mapping: SurfaceMapping,
}

/// Collapse vertices sharing a grid cell + material + smoothing group into their centroid, then
/// rebuild the index buffer dropping any triangle that lost two or more distinct corners.
fn cluster(mesh: &GeometryMesh, cell: f32) -> GeometryMesh {
    let inv = 1.0 / cell;
    let mut clusters: Vec<Cluster> = Vec::new();
    let mut lookup: HashMap<(i64, i64, i64, MaterialRole, u16), usize> = HashMap::new();
    let mut remap: Vec<u32> = Vec::with_capacity(mesh.vertex_count());

    for vertex in mesh.vertices() {
        let key = (
            (vertex.position.x * inv).floor() as i64,
            (vertex.position.y * inv).floor() as i64,
            (vertex.position.z * inv).floor() as i64,
            vertex.material,
            vertex.smoothing.0,
        );
        let index = *lookup.entry(key).or_insert_with(|| {
            clusters.push(Cluster {
                position: Vec3::ZERO,
                normal: Vec3::ZERO,
                shade: 0.0,
                count: 0.0,
                material: vertex.material,
                smoothing: vertex.smoothing,
                uv0: vertex.uv0,
                mapping: vertex.mapping,
            });
            clusters.len() - 1
        });
        let acc = &mut clusters[index];
        acc.position += vertex.position;
        acc.normal += vertex.normal;
        acc.shade += vertex.surface_shade;
        acc.count += 1.0;
        remap.push(index as u32);
    }

    let representatives: Vec<GeometryVertex> = clusters
        .iter()
        .map(|c| {
            let mut rep =
                GeometryVertex::new(c.position / c.count, c.normal, c.material, c.smoothing)
                    .with_surface_shade(c.shade / c.count);
            // Carry the authored parametric chart through the reduction; a Triplanar cell restores
            // exactly the old `new()` default (zero uv0, Triplanar), so nothing changes for it.
            rep.uv0 = c.uv0;
            rep.mapping = c.mapping;
            rep
        })
        .collect();

    let mut compact_of: Vec<u32> = vec![u32::MAX; representatives.len()];
    let mut out_vertices: Vec<GeometryVertex> = Vec::new();
    let mut out_indices: Vec<u32> = Vec::new();
    for tri in mesh.indices().chunks_exact(3) {
        let [a, b, c] = [remap[tri[0] as usize], remap[tri[1] as usize], remap[tri[2] as usize]];
        if a == b || b == c || a == c {
            continue;
        }
        for cluster_index in [a, b, c] {
            let slot = &mut compact_of[cluster_index as usize];
            if *slot == u32::MAX {
                *slot = out_vertices.len() as u32;
                out_vertices.push(representatives[cluster_index as usize]);
            }
            out_indices.push(*slot);
        }
    }

    GeometryMesh::new(out_vertices, out_indices)
}
