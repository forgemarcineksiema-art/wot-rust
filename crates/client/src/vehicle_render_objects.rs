use std::collections::HashMap;

use game_core::math::rotate_around;
use game_core::{ModuleSlot, TankId, VehicleKind};
use glam::{Mat3, Mat4, Vec3};
use net::TankSnapshot;
use renderer_api::{
    MaterialDescriptor, MaterialHandle, MeshAsset, MeshHandle, MeshRegistry,
    RenderMaterialRegistry, RenderObject, SceneVertex,
};
use vehicle_geometry::{GeometryMesh, MaterialRole, SubmeshKind, bake_vehicle};

use crate::color::{BARREL_STEEL, RUBBER, TRACK_METAL, shade_color};

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
    turret_ring: Vec3,
    trunnion: Vec3,
    turret_basis: Mat3,
    gun_basis: Mat3,
    fixed_casemate: bool,
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
        let vehicle = bake_vehicle(kind).ok()?;
        let turret_ring = vehicle.mounts().turret_ring.translation;
        let trunnion = vehicle.mounts().gun_trunnion.translation;
        let turret_basis = vehicle.mounts().turret_ring.basis;
        let gun_basis = vehicle.mounts().gun_trunnion.basis;
        let hull = vehicle.submesh(SubmeshKind::Hull)?;
        let turret = vehicle.submesh(SubmeshKind::Turret)?;
        let gun = vehicle.submesh(SubmeshKind::Gun)?;
        let entry = VehicleRenderEntry {
            hull: self.register_mesh(kind, SubmeshKind::Hull, &hull.mesh, Vec3::ZERO),
            turret: self.register_mesh(kind, SubmeshKind::Turret, &turret.mesh, turret_ring),
            gun: self.register_mesh(kind, SubmeshKind::Gun, &gun.mesh, trunnion),
            material: self.material(kind),
            turret_ring,
            trunnion,
            turret_basis,
            gun_basis,
            fixed_casemate: kind.has_fixed_casemate(),
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
    let ground = Vec3::from_array(snapshot.position);
    let hull_rotation = Mat3::from_rotation_y(snapshot.yaw_rad);
    let turret_yaw = if entry.fixed_casemate { 0.0 } else { snapshot.turret_yaw_rad };
    let turret_rotation = Mat3::from_rotation_y(turret_yaw);
    let gun_pitch = Mat3::from_rotation_x(-snapshot.gun_pitch_rad);

    let hull_transform = Mat4::from_translation(ground) * Mat4::from_mat3(hull_rotation);
    let turret_position = ground + hull_rotation * entry.turret_ring;
    let turret_transform = Mat4::from_translation(turret_position)
        * Mat4::from_mat3(hull_rotation * turret_rotation * entry.turret_basis);
    let gun_position =
        ground + hull_rotation * rotate_around(entry.trunnion, entry.turret_ring, turret_rotation);
    let gun_transform = Mat4::from_translation(gun_position)
        * Mat4::from_mat3(hull_rotation * turret_rotation * entry.gun_basis * gun_pitch);

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
