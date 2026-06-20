use std::collections::HashMap;

use game_core::{ModuleSlot, TankId, VehicleKind};
use glam::{Mat4, Vec3};
use net::TankSnapshot;
use renderer_api::{
    MaterialDescriptor, MaterialHandle, MeshAsset, MeshHandle, MeshRegistry,
    RenderMaterialRegistry, RenderObject, SceneVertex,
};
use vehicle_forge::authoritative_baked_vehicle;
use vehicle_geometry::{GeometryMesh, MaterialRole, SubmeshKind};

use crate::color::{BARREL_STEEL, RUBBER, TRACK_METAL, shade_color};
use crate::vehicle_pose::VehiclePose;

#[derive(Debug, Default)]
pub struct VehicleMeshCatalog {
    meshes: MeshRegistry,
    materials: RenderMaterialRegistry,
    vehicles: HashMap<VehicleKind, VehicleRenderEntry>,
    material_handles: HashMap<VehicleKind, MaterialHandle>,
    pending_meshes: Vec<(MeshHandle, MeshAsset)>,
}

#[derive(Debug, Clone, Copy)]
struct VehicleRenderEntry {
    hull: MeshHandle,
    turret: MeshHandle,
    gun: MeshHandle,
    material: MaterialHandle,
}

impl VehicleMeshCatalog {
    pub fn take_pending_meshes(&mut self) -> Vec<(MeshHandle, MeshAsset)> {
        std::mem::take(&mut self.pending_meshes)
    }

    pub fn cached_vehicle_count(&self) -> usize {
        self.vehicles.len()
    }

    pub fn material_count(&self) -> usize {
        self.material_handles.len()
    }

    fn vehicle_entry(&mut self, kind: VehicleKind) -> Option<VehicleRenderEntry> {
        if let Some(entry) = self.vehicles.get(&kind) {
            return Some(*entry);
        }
        let vehicle = authoritative_baked_vehicle(kind).ok()?;
        // Meshes are registered relative to their pivots; `VehiclePose` reads the same mount
        // frames at draw time, so registration and posing cannot disagree on the pivot points.
        let turret_ring = vehicle.mounts().turret_ring.translation;
        let trunnion = vehicle.mounts().gun_trunnion.translation;
        let hull = vehicle.submesh(SubmeshKind::Hull)?;
        let turret = vehicle.submesh(SubmeshKind::Turret)?;
        let gun = vehicle.submesh(SubmeshKind::Gun)?;
        let entry = VehicleRenderEntry {
            hull: self.register_mesh(kind, SubmeshKind::Hull, &hull.mesh, Vec3::ZERO),
            turret: self.register_mesh(kind, SubmeshKind::Turret, &turret.mesh, turret_ring),
            gun: self.register_mesh(kind, SubmeshKind::Gun, &gun.mesh, trunnion),
            material: self.material(kind),
        };
        self.vehicles.insert(kind, entry);
        Some(entry)
    }

    fn register_mesh(
        &mut self,
        kind: VehicleKind,
        submesh: SubmeshKind,
        mesh: &GeometryMesh,
        pivot: Vec3,
    ) -> MeshHandle {
        let asset = mesh_asset_from_geometry(mesh, pivot);
        let handle = self.meshes.register(format!("{}_{}", kind.slug(), submesh.label()), asset);
        let mesh = self.meshes.mesh(handle).expect("registered mesh asset").clone();
        self.pending_meshes.push((handle, mesh));
        handle
    }

    fn material(&mut self, kind: VehicleKind) -> MaterialHandle {
        // Colour rides on the per-object tint, so the material is team-neutral and shared.
        *self.material_handles.entry(kind).or_insert_with(|| {
            self.materials.register(MaterialDescriptor::new(kind.slug(), [1.0, 1.0, 1.0]))
        })
    }
}

pub fn tank_render_objects(
    catalog: &mut VehicleMeshCatalog,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
) -> Vec<RenderObject> {
    let entry = catalog.vehicle_entry(snapshot.vehicle).expect("vehicle must have baked geometry");
    let pose = VehiclePose::from_snapshot(snapshot);

    let hull_transform =
        Mat4::from_translation(pose.hull_translation()) * Mat4::from_mat3(pose.hull_basis());
    let turret_transform =
        Mat4::from_translation(pose.turret_translation()) * Mat4::from_mat3(pose.turret_basis());
    let gun_transform =
        Mat4::from_translation(pose.gun_translation()) * Mat4::from_mat3(pose.gun_basis());

    let hull_tint = damage_tint(
        hull_color,
        snapshot.destroyed_modules_mask,
        &[ModuleSlot::Engine, ModuleSlot::Suspension],
    );
    let turret_tint = damage_tint(
        hull_color,
        snapshot.destroyed_modules_mask,
        &[ModuleSlot::Turret, ModuleSlot::AmmoRack],
    );
    let gun_tint = damage_tint(hull_color, snapshot.destroyed_modules_mask, &[ModuleSlot::Gun]);

    vec![
        object(snapshot.tank_id, entry.hull, entry.material, hull_transform, hull_tint),
        object(snapshot.tank_id, entry.turret, entry.material, turret_transform, turret_tint),
        object(snapshot.tank_id, entry.gun, entry.material, gun_transform, gun_tint),
    ]
}

fn object(
    tank_id: TankId,
    mesh: MeshHandle,
    material: renderer_api::MaterialHandle,
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

fn mesh_asset_from_geometry(mesh: &GeometryMesh, pivot: Vec3) -> MeshAsset {
    MeshAsset::new(
        mesh.vertices()
            .iter()
            .map(|vertex| {
                let (base, tint_weight) = material_appearance(vertex.material);
                let color = shade_color(base, vertex.surface_shade);
                SceneVertex::tinted(
                    (vertex.position - pivot).to_array(),
                    vertex.normal.to_array(),
                    color,
                    tint_weight,
                )
            })
            .collect(),
        mesh.indices().to_vec(),
    )
}

fn damage_tint(base: [f32; 3], mask: u8, slots: &[ModuleSlot]) -> [f32; 3] {
    if slots.iter().any(|slot| mask & slot.destroyed_mask_bit() != 0) {
        [base[0] * 0.42, base[1] * 0.40, base[2] * 0.36]
    } else {
        base
    }
}

/// Team-neutral base colour and tint weight per material role. Armour is white and fully tinted so
/// the per-object team colour shows through unchanged; detail materials are absolute and untinted.
fn material_appearance(material: MaterialRole) -> ([f32; 3], f32) {
    match material {
        MaterialRole::RolledArmor | MaterialRole::CastArmor => ([1.0, 1.0, 1.0], 1.0),
        MaterialRole::BarrelSteel => (BARREL_STEEL, 0.0),
        MaterialRole::TrackMetal => (TRACK_METAL, 0.0),
        MaterialRole::Rubber => (RUBBER, 0.0),
    }
}

trait SubmeshKindLabel {
    fn label(self) -> &'static str;
}

impl SubmeshKindLabel for SubmeshKind {
    fn label(self) -> &'static str {
        match self {
            SubmeshKind::Hull => "hull",
            SubmeshKind::Turret => "turret",
            SubmeshKind::Gun => "gun",
        }
    }
}
