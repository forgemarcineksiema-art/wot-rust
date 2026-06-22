//! A vehicle part: its shape source (which generator builds it), and how it merges into a submesh.
//!
//! The routing here is the whole point of the hybrid: a flat armour plate goes to the exact convex
//! CAD generator (`solid`), a cast casting goes to the SDF + Surface Nets generator (`sdf_mesh`).
//! Future round parts (barrel, road wheels) add a `Revolved` arm. Every shape ends as a
//! `GeometryMesh`, so the rest of the Forge pipeline never learns which generator made it.

use glam::Vec3;
use sdf::Sdf;
use solid::ConvexSolid;
use vehicle_geometry::{GeometryMesh, LodLevel, MaterialRole, SmoothingGroup, SubmeshKind};

/// A stable semantic identity for a part or attachment, independent of merged-mesh vertex indices.
/// Anchors and the Forge graph key off this, so a part keeps its identity across LOD and bakes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartKey(pub &'static str);

/// How a part survives the LOD tiers. Characteristic silhouette forms are kept (then decimated) at
/// every tier; mount-bearing parts are always kept so the pose chain survives; detail fittings and
/// track links are dropped from LOD1 down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartLod {
    Silhouette,
    MountCritical,
    Detail,
}

impl PartLod {
    /// Whether a part with this policy is present in the given LOD tier.
    pub fn kept_at(self, lod: LodLevel) -> bool {
        match lod {
            LodLevel::Lod0 => true,
            LodLevel::Lod1 | LodLevel::Lod2 => self != PartLod::Detail,
        }
    }
}

/// The maximum visual-only surface displacement a part permits, in metres. Bake-only deformation
/// (cast asymmetry, a shallow dent, surface wear) clamps to this, so a visual tweak can never push a
/// vertex far enough to change the part's authored gameplay volume, armour facets or mount frames.
/// Gameplay-exact armour plates declare `NONE`; only soft visual surfaces open a tolerance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisualTolerance(pub f32);

impl VisualTolerance {
    /// No visual deformation permitted — the default for gameplay-exact plates.
    pub const NONE: Self = Self(0.0);

    /// The non-negative tolerance magnitude, ignoring a stray sign or NaN.
    pub fn metres(self) -> f32 {
        if self.0.is_finite() { self.0.abs() } else { 0.0 }
    }

    /// Clamp a proposed signed displacement into `[-tolerance, +tolerance]`.
    pub fn clamp(self, displacement: f32) -> f32 {
        let t = self.metres();
        displacement.clamp(-t, t)
    }
}

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
    pub lod: PartLod,
}

impl VehiclePart {
    /// Build this part's triangle mesh through its chosen generator.
    pub fn mesh(&self) -> GeometryMesh {
        match &self.shape {
            PartShape::Plates(solid) => solid
                .to_mesh(self.material, self.smoothing)
                .expect("a vehicle's plate solids are valid bounded armour"),
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
