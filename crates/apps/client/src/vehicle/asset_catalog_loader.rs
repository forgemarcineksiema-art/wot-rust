use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use renderer_api::{
    MaterialHandle, VehicleMaterialDescriptor, VehicleMaterialFamilies, VehicleMaterialMaps,
    VehicleTextureMap,
};
use vehicle_forge::{ForgeArtifact, MaterialFamily, authoritative_baked_vehicle};
use vehicle_geometry::{SubmeshKind, reduce_vehicle};

use super::asset_catalog::{VehicleAssetCatalog, VehicleAssetEntry};

impl VehicleAssetCatalog {
    pub fn load_forge_artifact_tree(&mut self, root: impl AsRef<Path>) -> Result<usize> {
        let root = root.as_ref();
        if !root.exists() {
            return Ok(0);
        }
        if root.join("manifest.json").is_file() {
            return Ok(self.load_forge_artifact_dir(root)? as usize);
        }
        let mut artifact_dirs = artifact_child_dirs(root)?;
        artifact_dirs.sort();
        let mut loaded = 0;
        for dir in artifact_dirs {
            if self.load_forge_artifact_dir(&dir)? {
                loaded += 1;
            }
        }
        Ok(loaded)
    }

    pub fn load_forge_artifact_dir(&mut self, path: impl AsRef<Path>) -> Result<bool> {
        let artifact = ForgeArtifact::read_from_dir(path.as_ref()).with_context(|| {
            format!("failed to load Forge artifact from {}", path.as_ref().display())
        })?;
        if !artifact_matches_current_geometry(&artifact)? {
            return Ok(false);
        }
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
            running_gear: self.register_running_gear(kind),
        };
        self.vehicles.insert(kind, entry);
        Ok(true)
    }

    fn material_from_artifact(&mut self, artifact: &ForgeArtifact) -> Result<MaterialHandle> {
        let kind = artifact.manifest().vehicle();
        if let Some(handle) = self.material_handles.get(&kind) {
            return Ok(*handle);
        }
        // Decode every role family in material_id layer order; a missing cavity layer simply leaves
        // that role on the renderer's neutral cavity. The descriptor records the representative
        // (first-layer) files for the artifact label.
        let mut families = Vec::with_capacity(VehicleMaterialFamilies::LAYERS);
        for family in MaterialFamily::ALL {
            let slug = family.slug();
            families.push(VehicleMaterialMaps::new(
                decode_required(artifact, &format!("{slug}_albedo.png"))?,
                decode_required(artifact, &format!("{slug}_normal.png"))?,
                decode_required(artifact, &format!("{slug}_ao_roughness_metalness.png"))?,
                decode_optional(artifact, &format!("{slug}_cavity.png")),
            ));
        }
        let lead = MaterialFamily::ALL[0].slug();
        let descriptor = VehicleMaterialDescriptor::pbr_lite(
            artifact.manifest().vehicle_slug(),
            format!("{lead}_albedo.png"),
            format!("{lead}_normal.png"),
            format!("{lead}_ao_roughness_metalness.png"),
            Some(format!("{lead}_cavity.png")),
        );
        let handle = MaterialHandle(self.materials.len() as u32);
        self.materials.push(descriptor);
        self.material_handles.insert(kind, handle);
        self.pending_materials.push((handle, VehicleMaterialFamilies::new(families)));
        Ok(handle)
    }
}

fn artifact_matches_current_geometry(artifact: &ForgeArtifact) -> Result<bool> {
    let manifest = artifact.manifest();
    let current = reduce_vehicle(
        &authoritative_baked_vehicle(manifest.vehicle())?,
        manifest.profile().lod_level(),
    );
    Ok(current.deterministic_hash() == manifest.source_hash())
}

fn decode_required(artifact: &ForgeArtifact, file: &str) -> Result<VehicleTextureMap> {
    decode_optional(artifact, file)
        .with_context(|| format!("artifact texture {file} is missing or could not be decoded"))
}

fn decode_optional(artifact: &ForgeArtifact, file: &str) -> Option<VehicleTextureMap> {
    let bytes = artifact.texture_payload(file)?;
    decode_png_rgba8(bytes)
}

fn decode_png_rgba8(bytes: &[u8]) -> Option<VehicleTextureMap> {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buffer).ok()?;
    let (width, height) = (info.width, info.height);
    let rgba = match info.color_type {
        png::ColorType::Rgba => buffer[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => buffer[..info.buffer_size()]
            .chunks_exact(3)
            .flat_map(|px| [px[0], px[1], px[2], 255])
            .collect(),
        png::ColorType::Grayscale => {
            buffer[..info.buffer_size()].iter().flat_map(|&v| [v, v, v, 255]).collect()
        }
        _ => return None,
    };
    if rgba.len() != width as usize * height as usize * 4 {
        return None;
    }
    Some(VehicleTextureMap::new(width, height, rgba))
}

fn artifact_child_dirs(root: &Path) -> Result<Vec<PathBuf>> {
    let mut dirs = Vec::new();
    for entry in std::fs::read_dir(root)
        .with_context(|| format!("failed to read Forge artifact root {}", root.display()))?
    {
        let path = entry?.path();
        if path.join("manifest.json").is_file() {
            dirs.push(path);
        }
    }
    Ok(dirs)
}
