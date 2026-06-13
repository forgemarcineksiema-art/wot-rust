use std::{fs, io, path::Path};

use game_core::VehicleKind;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use vehicle_geometry::bake_vehicle;

use crate::{RatioReport, ReferencePack};

mod mesh_payload;
mod review;

pub use review::{ReviewCamera, ReviewCameraSet, ReviewCameraSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BakeProfile {
    Lod0,
    Lod1,
    Lod2,
}

impl BakeProfile {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Lod0 => "lod0",
            Self::Lod1 => "lod1",
            Self::Lod2 => "lod2",
        }
    }
}

impl std::str::FromStr for BakeProfile {
    type Err = ArtifactError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "lod0" => Ok(Self::Lod0),
            "lod1" => Ok(Self::Lod1),
            "lod2" => Ok(Self::Lod2),
            other => Err(ArtifactError::UnknownProfile(other.to_string())),
        }
    }
}

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
    vehicle: VehicleKind,
    vehicle_slug: String,
    profile: BakeProfile,
    source_family_slug: Option<String>,
    source_hash: u64,
    mesh_bytes: usize,
    submeshes: Vec<ForgeSubmeshManifest>,
    review_cameras: ReviewCameraSet,
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

    pub fn review_cameras(&self) -> &ReviewCameraSet {
        &self.review_cameras
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForgeSubmeshManifest {
    kind: String,
    vertices: usize,
    indices: usize,
    triangles: usize,
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

#[derive(Debug, Clone, PartialEq)]
pub struct ForgeArtifact {
    manifest: ForgeArtifactManifest,
    mesh_payload: Vec<u8>,
    report: RatioReport,
}

impl ForgeArtifact {
    pub fn bake(vehicle: VehicleKind, profile: BakeProfile) -> Result<Self, ArtifactError> {
        let baked = bake_vehicle(vehicle)?;
        let reference = ReferencePack::for_vehicle(vehicle)
            .ok_or(ArtifactError::MissingReferencePack(vehicle))?;
        let report = reference
            .measure_baked_vehicle(&baked)
            .ok_or(ArtifactError::RatioReportRejected(vehicle))?;
        let mesh_payload = mesh_payload::encode(&baked)?;
        let manifest = ForgeArtifactManifest {
            vehicle,
            vehicle_slug: forge_vehicle_slug(vehicle).to_string(),
            profile,
            source_family_slug: Some(reference.family_slug().to_string()),
            source_hash: baked.deterministic_hash(),
            mesh_bytes: mesh_payload.len(),
            submeshes: baked
                .submeshes()
                .iter()
                .map(|submesh| ForgeSubmeshManifest {
                    kind: mesh_payload::submesh_kind_name(submesh.kind).to_string(),
                    vertices: submesh.mesh.vertex_count(),
                    indices: submesh.mesh.indices().len(),
                    triangles: submesh.mesh.triangle_count(),
                })
                .collect(),
            review_cameras: ReviewCameraSet::standard_vehicle_review(),
        };
        Ok(Self { manifest, mesh_payload, report })
    }

    pub fn manifest(&self) -> &ForgeArtifactManifest {
        &self.manifest
    }

    pub fn mesh_payload(&self) -> &[u8] {
        &self.mesh_payload
    }

    pub fn report(&self) -> &RatioReport {
        &self.report
    }

    pub fn report_markdown(&self) -> String {
        self.report.markdown_summary()
    }

    pub fn write_to_dir(&self, out: &Path) -> Result<(), ArtifactError> {
        fs::create_dir_all(out)?;
        fs::create_dir_all(out.join("review"))?;
        fs::write(out.join("manifest.json"), serde_json::to_string_pretty(&self.manifest)?)?;
        fs::write(out.join("meshes.bin"), &self.mesh_payload)?;
        fs::write(out.join("report.md"), self.report_markdown())?;
        Ok(())
    }
}

pub fn forge_vehicle_slug(kind: VehicleKind) -> &'static str {
    match kind {
        VehicleKind::PrototypeMedium => "prototype-medium",
        VehicleKind::T54_1951 => "t54-1951",
        VehicleKind::T55A => "t55a",
        VehicleKind::TigerI => "tiger-i-ausf-e",
        VehicleKind::TigerII => "tiger-ii-ausf-b",
        VehicleKind::Jagdtiger => "jagdtiger",
        VehicleKind::PantherII => "panther-ii",
    }
}
