//! Shared runtime material used when no current Forge artifact supplies a vehicle override.

use game_core::VehicleKind;
use renderer_api::{
    MaterialHandle, VehicleMaterialDescriptor, VehicleMaterialFamilies, VehicleMaterialMaps,
    VehicleTextureMap,
};
use vehicle_forge::{DefaultMaterialMap, default_material_families};

use super::asset_catalog::VehicleAssetCatalog;

const DEFAULT_MATERIAL_LABEL: &str = "default_vehicle_material_families";

pub(super) fn material_for(catalog: &mut VehicleAssetCatalog, kind: VehicleKind) -> MaterialHandle {
    if let Some(handle) = catalog.material_handles.get(&kind) {
        return *handle;
    }

    let handle = catalog
        .materials
        .iter()
        .position(|material| material.label() == DEFAULT_MATERIAL_LABEL)
        .map(|index| MaterialHandle(index as u32))
        .unwrap_or_else(|| register_default_material(catalog));
    catalog.material_handles.insert(kind, handle);
    handle
}

pub(super) fn is_default_handle(catalog: &VehicleAssetCatalog, handle: MaterialHandle) -> bool {
    catalog
        .materials
        .get(handle.0 as usize)
        .is_some_and(|material| material.label() == DEFAULT_MATERIAL_LABEL)
}

fn register_default_material(catalog: &mut VehicleAssetCatalog) -> MaterialHandle {
    let handle = MaterialHandle(catalog.materials.len() as u32);
    catalog.materials.push(VehicleMaterialDescriptor::pbr_lite(
        DEFAULT_MATERIAL_LABEL,
        "albedo.png",
        "normal.png",
        "ao_roughness.png",
        Some("cavity.png"),
    ));
    catalog.pending_materials.push((handle, renderer_material_families()));
    handle
}

fn renderer_material_families() -> VehicleMaterialFamilies {
    let families = default_material_families()
        .iter()
        .map(|family| {
            VehicleMaterialMaps::new(
                renderer_map(family.albedo()),
                renderer_map(family.normal()),
                renderer_map(family.ao_roughness_metalness()),
                Some(renderer_map(family.cavity())),
            )
        })
        .collect();
    VehicleMaterialFamilies::new(families)
}

fn renderer_map(map: &DefaultMaterialMap) -> VehicleTextureMap {
    VehicleTextureMap::new(map.width(), map.height(), map.rgba().to_vec())
}
