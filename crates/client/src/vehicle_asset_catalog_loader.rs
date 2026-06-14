use std::path::Path;

use anyhow::{Context, Result};
use renderer_api::{MaterialHandle, VehicleMaterialDescriptor};
use vehicle_forge::{ForgeArtifact, ForgeTextureManifest};
use vehicle_geometry::SubmeshKind;

use crate::vehicle_asset_catalog::{VehicleAssetCatalog, VehicleAssetEntry};

impl VehicleAssetCatalog {
    pub fn load_forge_artifact_dir(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let artifact = ForgeArtifact::read_from_dir(path.as_ref()).with_context(|| {
            format!("failed to load Forge artifact from {}", path.as_ref().display())
        })?;
        let vehicle = artifact.baked_vehicle()?;
        let kind = vehicle.kind();
        let material = self.material_from_artifact(&artifact)?;
        let mounts = vehicle.mounts();
        let hull = vehicle.submesh(SubmeshKind::Hull).context("artifact is missing hull mesh")?;
        let turret =
            vehicle.submesh(SubmeshKind::Turret).context("artifact is missing turret mesh")?;
        let gun = vehicle.submesh(SubmeshKind::Gun).context("artifact is missing gun mesh")?;

        let entry = VehicleAssetEntry {
            hull: self.register_vehicle_mesh(kind, SubmeshKind::Hull, &hull.mesh, glam::Vec3::ZERO),
            turret: self.register_vehicle_mesh(
                kind,
                SubmeshKind::Turret,
                &turret.mesh,
                mounts.turret_ring.translation,
            ),
            gun: self.register_vehicle_mesh(
                kind,
                SubmeshKind::Gun,
                &gun.mesh,
                mounts.gun_trunnion.translation,
            ),
            material,
        };
        self.vehicles.insert(kind, entry);
        Ok(())
    }

    fn material_from_artifact(&mut self, artifact: &ForgeArtifact) -> Result<MaterialHandle> {
        let kind = artifact.manifest().vehicle();
        if let Some(handle) = self.material_handles.get(&kind) {
            return Ok(*handle);
        }
        let maps = artifact.manifest().texture_maps();
        let descriptor = VehicleMaterialDescriptor::pbr_lite(
            artifact.manifest().vehicle_slug(),
            required_map(maps, "albedo")?,
            required_map(maps, "normal")?,
            required_map(maps, "ao_roughness_metalness")?,
            optional_map(maps, "cavity"),
        );
        let handle = MaterialHandle(self.materials.len() as u32);
        self.materials.push(descriptor);
        self.material_handles.insert(kind, handle);
        Ok(handle)
    }
}

fn required_map(maps: &[ForgeTextureManifest], semantic: &str) -> Result<String> {
    optional_map(maps, semantic).with_context(|| format!("artifact is missing {semantic} map"))
}

fn optional_map(maps: &[ForgeTextureManifest], semantic: &str) -> Option<String> {
    maps.iter().find(|map| map.semantic() == semantic).map(|map| map.file().to_string())
}
