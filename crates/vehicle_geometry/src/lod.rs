//! Deterministic level-of-detail reduction for baked vehicles.
//!
//! LOD0 is the authored mesh exactly as the recipes build it. LOD1/LOD2 are produced by
//! **vertex clustering** (Rossignac-style): space is partitioned into a uniform grid sized as a
//! fraction of the vehicle's body extent, every vertex in a cell (of the same material and
//! smoothing group) collapses to the cell centroid, and triangles that degenerate to a line or
//! point are dropped. This is fully deterministic and — because each representative is the
//! centroid of its cluster — it can only pull the silhouette *inward*, so a LOD never pokes
//! outside the gameplay hitbox that LOD0 already respected. Mount frames are geometry-independent,
//! so they are carried across every level unchanged.

use std::collections::HashMap;

use glam::Vec3;

use crate::mesh::{GeometryVertex, MaterialRole, SmoothingGroup};
use crate::{BakedVehicle, GeometryMesh, MeshBounds, Submesh};

/// The bake detail tiers. LOD0 is near/garage detail; LOD2 is the distant silhouette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LodLevel {
    Lod0,
    Lod1,
    Lod2,
}

impl LodLevel {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Lod0 => "lod0",
            Self::Lod1 => "lod1",
            Self::Lod2 => "lod2",
        }
    }

    /// Grid cell size as a fraction of the body's largest extent. `None` means "no reduction".
    fn cluster_fraction(self) -> Option<f32> {
        match self {
            Self::Lod0 => None,
            Self::Lod1 => Some(0.045),
            Self::Lod2 => Some(0.090),
        }
    }
}

/// Produce the baked vehicle at the requested detail level. LOD0 returns the authored bake; the
/// reduced levels cluster every submesh against a body-relative grid and re-weld for clean normals.
pub fn reduce_vehicle(base: &BakedVehicle, level: LodLevel) -> BakedVehicle {
    let Some(fraction) = level.cluster_fraction() else {
        return base.clone();
    };
    let extent = base
        .body_bounds()
        .map(|b: MeshBounds| (b.max - b.min).max_element())
        .unwrap_or(1.0)
        .max(1.0e-3);
    let cell = (extent * fraction).max(1.0e-4);
    let submeshes: Vec<Submesh> = base
        .submeshes()
        .iter()
        .map(|submesh| Submesh {
            kind: submesh.kind,
            mesh: cluster(&submesh.mesh, cell).weld_and_smooth(),
        })
        .collect();
    BakedVehicle::new(base.kind(), submeshes, base.mounts().clone())
}

struct Cluster {
    position: Vec3,
    normal: Vec3,
    shade: f32,
    count: f32,
    material: MaterialRole,
    smoothing: SmoothingGroup,
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
            GeometryVertex::new(c.position / c.count, c.normal, c.material, c.smoothing)
                .with_surface_shade(c.shade / c.count)
        })
        .collect();

    // Remap triangles and compact to only the vertices the surviving triangles still reference.
    let mut compact_of: Vec<u32> = vec![u32::MAX; representatives.len()];
    let mut out_vertices: Vec<GeometryVertex> = Vec::new();
    let mut out_indices: Vec<u32> = Vec::new();
    for tri in mesh.indices().chunks_exact(3) {
        let [a, b, c] = [
            remap[tri[0] as usize],
            remap[tri[1] as usize],
            remap[tri[2] as usize],
        ];
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
