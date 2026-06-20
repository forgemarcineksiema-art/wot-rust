//! A vehicle part: its shape source (which generator builds it), and how it merges into a submesh.
//!
//! The routing here is the whole point of the hybrid: a flat armour plate goes to the exact convex
//! CAD generator (`solid`), a cast casting goes to the SDF + Surface Nets generator (`sdf_mesh`).
//! Future round parts (barrel, road wheels) add a `Revolved` arm. Every shape ends as a
//! `GeometryMesh`, so the rest of the Forge pipeline never learns which generator made it.

use glam::Vec3;
use sdf::Sdf;
use solid::ConvexSolid;
use vehicle_geometry::{GeometryMesh, MaterialRole, SmoothingGroup, SubmeshKind};

/// How a part's geometry is generated.
pub enum PartShape {
    /// Flat armour plates: an exact convex solid (crisp edges, exact slopes, few triangles).
    Plates(ConvexSolid),
    /// A cast casting: an SDF meshed to a triangle budget (smooth, organic).
    Cast { sdf: Sdf, min: Vec3, max: Vec3, budget: usize },
    /// A pre-built mesh from any other generator (e.g. a revolved barrel or wheel train).
    Mesh(GeometryMesh),
}

/// One part of a vehicle: where it lands (hull/turret/gun), its material, and its shape source.
pub struct VehiclePart {
    pub submesh: SubmeshKind,
    pub material: MaterialRole,
    pub smoothing: SmoothingGroup,
    pub shape: PartShape,
}

impl VehiclePart {
    /// Build this part's triangle mesh through its chosen generator.
    pub fn mesh(&self) -> GeometryMesh {
        match &self.shape {
            PartShape::Plates(solid) => solid.to_mesh(self.material, self.smoothing),
            PartShape::Cast { sdf, min, max, budget } => {
                sdf_mesh::mesh_within_budget(
                    sdf,
                    *min,
                    *max,
                    *budget,
                    self.material,
                    self.smoothing,
                )
                .0
            }
            PartShape::Mesh(mesh) => mesh.clone(),
        }
    }
}
