use std::io;

use game_core::VehicleKind;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{BakeProfile, ForgeTextureManifest, ReviewCameraSet, SurfaceBakeManifest};

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error(transparent)]
    Bake(#[from] vehicle_geometry::BakeError),
    #[error("no ReferencePack for {0:?}")]
    MissingReferencePack(VehicleKind),
    #[error("ratio report rejected {0:?} for the selected ReferencePack")]
    RatioReportRejected(VehicleKind),
    #[error("unknown bake profile: {0}")]
    UnknownProfile(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForgeArtifactManifest {
    pub(crate) vehicle: VehicleKind,
    pub(crate) vehicle_slug: String,
    pub(crate) profile: BakeProfile,
    pub(crate) source_family_slug: Option<String>,
    pub(crate) source_hash: u64,
    pub(crate) mesh_bytes: usize,
    pub(crate) submeshes: Vec<ForgeSubmeshManifest>,
    pub(crate) texture_maps: Vec<ForgeTextureManifest>,
    pub(crate) review_cameras: ReviewCameraSet,
    /// The per-vehicle ambient-contact bake summary, absent for vehicles that bake flat.
    #[serde(default)]
    pub(crate) surface_bake: Option<SurfaceBakeManifest>,
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
    pub(crate) kind: String,
    pub(crate) vertices: usize,
    pub(crate) indices: usize,
    pub(crate) triangles: usize,
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
