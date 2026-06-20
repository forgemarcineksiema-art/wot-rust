use std::collections::{BTreeMap, HashMap};

use game_core::{ModuleSlot, TankId, VehicleKind};
use glam::{Mat4, Vec3};
use net::TankSnapshot;
use renderer_api::{
    MaterialHandle, MeshHandle, RenderObject, VehicleMaterialDescriptor, VehicleMaterialMaps,
    VehicleMeshAsset,
};
use vehicle_forge::{BakeProfile, bake_production_vehicle};
use vehicle_geometry::{GeometryMesh, SubmeshKind};

use crate::vehicle_pbr_mesh::vehicle_submesh_vertices;
use crate::vehicle_pose::VehiclePose;
use crate::vehicle_variation::VehicleVariation;

#[derive(Debug, Default)]
pub struct VehicleAssetCatalog {
    mesh_labels: BTreeMap<String, MeshHandle>,
    meshes: Vec<(String, VehicleMeshAsset)>,
    pub(crate) materials: Vec<VehicleMaterialDescriptor>,
    pub(crate) material_handles: HashMap<VehicleKind, MaterialHandle>,
    pub(crate) vehicles: HashMap<VehicleKind, VehicleAssetEntry>,
    pending_meshes: Vec<(MeshHandle, VehicleMeshAsset)>,
    pub(crate) pending_materials: Vec<(MaterialHandle, VehicleMaterialMaps)>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct VehicleAssetEntry {
    pub(crate) hull: MeshHandle,
    pub(crate) turret: MeshHandle,
    pub(crate) gun: MeshHandle,
    pub(crate) material: MaterialHandle,
}

impl VehicleAssetCatalog {
    pub fn take_pending_vehicle_meshes(&mut self) -> Vec<(MeshHandle, VehicleMeshAsset)> {
        std::mem::take(&mut self.pending_meshes)
    }

    pub fn take_pending_vehicle_materials(&mut self) -> Vec<(MaterialHandle, VehicleMaterialMaps)> {
        std::mem::take(&mut self.pending_materials)
    }

    pub fn vehicle_material(&self, handle: MaterialHandle) -> Option<&VehicleMaterialDescriptor> {
        self.materials.get(handle.0 as usize)
    }

    pub fn cached_vehicle_count(&self) -> usize {
        self.vehicles.len()
    }

    pub fn material_count(&self) -> usize {
        self.material_handles.len()
    }

    fn vehicle_entry(&mut self, kind: VehicleKind) -> Option<VehicleAssetEntry> {
        if let Some(entry) = self.vehicles.get(&kind) {
            return Some(*entry);
        }
        let vehicle = bake_production_vehicle(kind, BakeProfile::Lod0).ok()?;
        let turret_ring = vehicle.mounts().turret_ring.translation;
        let trunnion = vehicle.mounts().gun_trunnion.translation;
        let hull = vehicle.submesh(SubmeshKind::Hull)?;
        let turret = vehicle.submesh(SubmeshKind::Turret)?;
        let gun = vehicle.submesh(SubmeshKind::Gun)?;
        let entry = VehicleAssetEntry {
            hull: self.register_vehicle_mesh(kind, SubmeshKind::Hull, &hull.mesh, Vec3::ZERO),
            turret: self.register_vehicle_mesh(
                kind,
                SubmeshKind::Turret,
                &turret.mesh,
                turret_ring,
            ),
            gun: self.register_vehicle_mesh(kind, SubmeshKind::Gun, &gun.mesh, trunnion),
            material: self.material(kind),
        };
        self.vehicles.insert(kind, entry);
        Some(entry)
    }

    pub(crate) fn register_vehicle_mesh(
        &mut self,
        kind: VehicleKind,
        submesh: SubmeshKind,
        mesh: &GeometryMesh,
        pivot: Vec3,
    ) -> MeshHandle {
        let label = format!("{}_{}_vehicle", kind.slug(), submesh_label(submesh));
        if let Some(handle) = self.mesh_labels.get(&label).copied() {
            let asset = vehicle_mesh_asset_from_geometry(mesh, pivot);
            if let Some((_, stored)) = self.meshes.get_mut(handle.0 as usize) {
                *stored = asset.clone();
            }
            self.pending_meshes.push((handle, asset));
            return handle;
        }
        let handle = MeshHandle(self.meshes.len() as u32);
        let asset = vehicle_mesh_asset_from_geometry(mesh, pivot);
        self.mesh_labels.insert(label.clone(), handle);
        self.meshes.push((label, asset.clone()));
        self.pending_meshes.push((handle, asset));
        handle
    }

    fn material(&mut self, kind: VehicleKind) -> MaterialHandle {
        *self.material_handles.entry(kind).or_insert_with(|| {
            let handle = MaterialHandle(self.materials.len() as u32);
            self.materials.push(VehicleMaterialDescriptor::pbr_lite(
                kind.slug(),
                "albedo.png",
                "normal.png",
                "ao_roughness.png",
                Some("cavity.png"),
            ));
            handle
        })
    }
}

pub fn tank_vehicle_render_objects(
    catalog: &mut VehicleAssetCatalog,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
) -> Vec<RenderObject> {
    tank_vehicle_render_objects_with_variation(
        catalog,
        snapshot,
        hull_color,
        &VehicleVariation::from_snapshot(snapshot),
    )
}

/// As [`tank_vehicle_render_objects`], but with an explicit runtime variation state (camo, dirt,
/// snow, broken tracks, destroyed modules). The base render path uses the snapshot-derived
/// variation; callers carrying richer per-tank cosmetic state pass it here.
pub fn tank_vehicle_render_objects_with_variation(
    catalog: &mut VehicleAssetCatalog,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
    variation: &VehicleVariation,
) -> Vec<RenderObject> {
    let entry = catalog.vehicle_entry(snapshot.vehicle).expect("vehicle must have baked geometry");
    let pose = VehiclePose::from_snapshot(snapshot);
    let hull_transform =
        Mat4::from_translation(pose.hull_translation()) * Mat4::from_mat3(pose.hull_basis());
    let turret_transform =
        Mat4::from_translation(pose.turret_translation()) * Mat4::from_mat3(pose.turret_basis());
    let gun_transform =
        Mat4::from_translation(pose.gun_translation()) * Mat4::from_mat3(pose.gun_basis());

    // The cosmetic overlays (camo/dirt/snow) recolour the whole vehicle; per-module damage then
    // darkens the submeshes whose modules are knocked out.
    let surface = variation.surface_tint(hull_color);

    vec![
        vehicle_render_object(
            snapshot.tank_id,
            entry.hull,
            entry.material,
            hull_transform,
            variation.module_tint(surface, &[ModuleSlot::Engine, ModuleSlot::Suspension]),
        ),
        vehicle_render_object(
            snapshot.tank_id,
            entry.turret,
            entry.material,
            turret_transform,
            variation.module_tint(surface, &[ModuleSlot::Turret, ModuleSlot::AmmoRack]),
        ),
        vehicle_render_object(
            snapshot.tank_id,
            entry.gun,
            entry.material,
            gun_transform,
            variation.module_tint(surface, &[ModuleSlot::Gun]),
        ),
    ]
}

fn vehicle_mesh_asset_from_geometry(mesh: &GeometryMesh, pivot: Vec3) -> VehicleMeshAsset {
    let (mut vertices, indices) = vehicle_submesh_vertices(mesh);
    for vertex in &mut vertices {
        let shifted = Vec3::from_array(vertex.position) - pivot;
        vertex.position = shifted.to_array();
    }
    VehicleMeshAsset::new(vertices, indices)
}

fn vehicle_render_object(
    tank_id: TankId,
    mesh: MeshHandle,
    material: MaterialHandle,
    transform: Mat4,
    tint: [f32; 3],
) -> RenderObject {
    RenderObject {
        tank_id: Some(tank_id),
        mesh,
        material,
        transform: transform.to_cols_array_2d(),
        tint,
    }
}

fn submesh_label(submesh: SubmeshKind) -> &'static str {
    match submesh {
        SubmeshKind::Hull => "hull",
        SubmeshKind::Turret => "turret",
        SubmeshKind::Gun => "gun",
    }
}
