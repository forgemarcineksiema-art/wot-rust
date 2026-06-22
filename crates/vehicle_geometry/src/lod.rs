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

pub mod importance;

pub use importance::{LodAuditError, PartImportance, audit_reduction, reduce_mesh};

use crate::{BakedVehicle, MeshBounds, Submesh};

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
    /// Public so a part-aware reducer can derive each part's cell from the same tier base.
    pub fn cluster_fraction(self) -> Option<f32> {
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
        .map(|submesh| Submesh { kind: submesh.kind, mesh: reduce_mesh(&submesh.mesh, cell) })
        .collect();
    BakedVehicle::new(base.kind(), submeshes, *base.mounts())
}
