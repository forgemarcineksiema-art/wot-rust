use std::{fs, path::Path};

use game_core::VehicleKind;
use vehicle_geometry::{BakedVehicle, reduce_vehicle};

use crate::{RatioReport, authoritative_baked_vehicle};

use super::{
    ArtifactError, BakeProfile, ForgeArtifactManifest, ForgeSubmeshManifest, forge_vehicle_slug,
    mesh_payload, review_images, texture_maps,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ForgeArtifact {
    pub(super) manifest: ForgeArtifactManifest,
    pub(super) mesh_payload: Vec<u8>,
    pub(super) texture_maps: Vec<texture_maps::BakedTextureMap>,
    pub(super) review_images: Vec<review_images::BakedReviewImage>,
    pub(super) report: RatioReport,
}

impl ForgeArtifact {
    pub fn bake(vehicle: VehicleKind, profile: BakeProfile) -> Result<Self, ArtifactError> {
        Self::bake_from_baked(vehicle, profile, authoritative_baked_vehicle(vehicle)?)
    }

    pub fn bake_from_baked(
        vehicle: VehicleKind,
        profile: BakeProfile,
        baked: BakedVehicle,
    ) -> Result<Self, ArtifactError> {
        let spec = crate::registry::forge_spec(vehicle)
            .ok_or(ArtifactError::MissingReferencePack(vehicle))?;
        let baked = reduce_vehicle(&baked, profile.lod_level());
        let reference = (spec.reference_pack)();
        let report = reference
            .measure_baked_vehicle(&baked)
            .ok_or(ArtifactError::RatioReportRejected(vehicle))?;
        let mesh_payload = mesh_payload::encode(&baked)?;
        let texture_maps = texture_maps::bake_default_set()?;
        let review_cameras = (spec.review_cameras)();
        let review_images = review_images::bake_review_images(&baked, &review_cameras)?;
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
            texture_maps: texture_maps.iter().map(|map| map.manifest().clone()).collect(),
            review_cameras,
        };
        Ok(Self { manifest, mesh_payload, texture_maps, review_images, report })
    }

    pub fn manifest(&self) -> &ForgeArtifactManifest {
        &self.manifest
    }
    pub fn baked_vehicle(&self) -> Result<BakedVehicle, ArtifactError> {
        Ok(mesh_payload::decode(self.manifest.vehicle(), &self.mesh_payload)?)
    }
    pub fn mesh_payload(&self) -> &[u8] {
        &self.mesh_payload
    }
    pub fn texture_payload(&self, file: &str) -> Option<&[u8]> {
        self.texture_maps
            .iter()
            .find(|map| map.manifest().file() == file)
            .map(texture_maps::BakedTextureMap::bytes)
    }
    pub fn review_image_payload(&self, file: &str) -> Option<&[u8]> {
        self.review_images
            .iter()
            .find(|image| image.file() == file)
            .map(review_images::BakedReviewImage::bytes)
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
        for map in &self.texture_maps {
            fs::write(out.join(map.manifest().file()), map.bytes())?;
        }
        let review_dir = out.join("review");
        for image in &self.review_images {
            fs::write(review_dir.join(image.file()), image.bytes())?;
        }
        fs::write(out.join("report.md"), self.report_markdown())?;
        Ok(())
    }
}
