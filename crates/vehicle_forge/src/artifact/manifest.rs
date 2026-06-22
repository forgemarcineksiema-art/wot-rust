//! The serialized Forge artifact manifest: the JSON record of what a bake produced — the vehicle and
//! profile, the source hash, the mesh accounting, the material-family maps, the review cameras and
//! the per-vehicle surface bake. Fields are `pub(super)` so the bake in [`super`] assembles them
//! directly while the public read API stays getter-only.

use game_core::VehicleKind;
use serde::{Deserialize, Serialize};

use super::{BakeProfile, ForgeTextureManifest, ReviewCameraSet, SurfaceBakeManifest};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForgeArtifactManifest {
    pub(super) vehicle: VehicleKind,
    pub(super) vehicle_slug: String,
    pub(super) profile: BakeProfile,
    pub(super) source_family_slug: Option<String>,
    pub(super) source_hash: u64,
    pub(super) mesh_bytes: usize,
    pub(super) submeshes: Vec<ForgeSubmeshManifest>,
    pub(super) texture_maps: Vec<ForgeTextureManifest>,
    pub(super) review_cameras: ReviewCameraSet,
    /// The per-vehicle ambient-contact bake summary, absent for vehicles that bake flat.
    #[serde(default)]
    pub(super) surface_bake: Option<SurfaceBakeManifest>,
}

impl ForgeArtifactManifest {
    pub fn vehicle(&self) -> VehicleKind {
        self.vehicle
    }

    pub fn vehicle_slug(&self) -> &str {
        &self.vehicle_slug
    }

    pub fn profile(&self) -> BakeProfile {
        self.profile
    }

    pub fn source_family_slug(&self) -> Option<&str> {
        self.source_family_slug.as_deref()
    }

    pub fn source_hash(&self) -> u64 {
        self.source_hash
    }

    pub fn mesh_bytes(&self) -> usize {
        self.mesh_bytes
    }

    pub fn submeshes(&self) -> &[ForgeSubmeshManifest] {
        &self.submeshes
    }

    pub fn texture_maps(&self) -> &[ForgeTextureManifest] {
        &self.texture_maps
    }

    pub fn review_cameras(&self) -> &ReviewCameraSet {
        &self.review_cameras
    }

    /// The per-vehicle ambient-contact bake, or `None` if the vehicle bakes a flat surface shade.
    pub fn surface_bake(&self) -> Option<&SurfaceBakeManifest> {
        self.surface_bake.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForgeSubmeshManifest {
    pub(super) kind: String,
    pub(super) vertices: usize,
    pub(super) indices: usize,
    pub(super) triangles: usize,
}

impl ForgeSubmeshManifest {
    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn vertices(&self) -> usize {
        self.vertices
    }

    pub fn indices(&self) -> usize {
        self.indices
    }

    pub fn triangles(&self) -> usize {
        self.triangles
    }
}
