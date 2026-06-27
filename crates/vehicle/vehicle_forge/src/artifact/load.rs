use std::{fs, path::Path};

use crate::ReferencePack;

use super::{
    ArtifactError, ForgeArtifact, ForgeArtifactManifest, mesh_payload, review_images, texture_maps,
};

impl ForgeArtifact {
    pub fn read_from_dir(path: &Path) -> Result<Self, ArtifactError> {
        let manifest: ForgeArtifactManifest =
            serde_json::from_slice(&fs::read(path.join("manifest.json"))?)?;
        let mesh_payload = fs::read(path.join("meshes.bin"))?;
        let baked = mesh_payload::decode(manifest.vehicle(), &mesh_payload)?;
        let reference = ReferencePack::for_vehicle(manifest.vehicle())
            .ok_or(ArtifactError::MissingReferencePack(manifest.vehicle()))?;
        let report = reference
            .measure_baked_vehicle(&baked)
            .ok_or(ArtifactError::RatioReportRejected(manifest.vehicle()))?;
        let texture_maps = read_texture_maps(path, &manifest)?;
        let review_images = read_review_images(path, &manifest)?;
        Ok(Self { manifest, mesh_payload, texture_maps, review_images, report })
    }
}

fn read_texture_maps(
    path: &Path,
    manifest: &ForgeArtifactManifest,
) -> Result<Vec<texture_maps::BakedTextureMap>, ArtifactError> {
    manifest
        .texture_maps()
        .iter()
        .map(|map| {
            let bytes = fs::read(path.join(map.file()))?;
            Ok(texture_maps::BakedTextureMap::from_loaded(map.clone(), bytes))
        })
        .collect()
}

fn read_review_images(
    path: &Path,
    manifest: &ForgeArtifactManifest,
) -> Result<Vec<review_images::BakedReviewImage>, ArtifactError> {
    manifest
        .review_cameras()
        .cameras()
        .iter()
        .map(|camera| {
            let file = camera.file_name();
            let bytes = fs::read(path.join("review").join(file))?;
            Ok(review_images::BakedReviewImage::from_loaded(file, bytes))
        })
        .collect()
}
