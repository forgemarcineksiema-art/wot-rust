use std::collections::HashMap;

use game_core::VehicleKind;
use glam::Vec3;
use renderer_api::{
    MaterialDescriptor, MaterialHandle, MeshAsset, MeshHandle, MeshRegistry,
    RenderMaterialRegistry, SceneVertex,
};
use vehicle_forge::authoritative_baked_vehicle;
use vehicle_geometry::{
    GeometryMesh, MaterialRole, RunningGearKinematics, SubmeshKind, idler_unit_mesh,
    road_wheel_unit_mesh, sprocket_unit_mesh, track_link_unit_mesh,
};

use super::running_gear_objects::GearMeshHandles;
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
pub(crate) struct VehicleRenderEntry {
    pub(crate) hull: MeshHandle,
    pub(crate) turret: MeshHandle,
    pub(crate) gun: MeshHandle,
    pub(crate) material: MaterialHandle,
    pub(crate) running_gear: Option<GearMeshHandles>,
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

    pub(crate) fn vehicle_entry(&mut self, kind: VehicleKind) -> Option<VehicleRenderEntry> {
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
            running_gear: self.register_running_gear(kind),
        };
        self.vehicles.insert(kind, entry);
        Some(entry)
    }

    fn register_running_gear(&mut self, kind: VehicleKind) -> Option<GearMeshHandles> {
        let kin = RunningGearKinematics::for_vehicle(kind)?;
        Some(GearMeshHandles {
            road_wheel: self.register_gear_mesh(kind, "road_wheel", &road_wheel_unit_mesh(&kin)),
            swing_arm: self.register_gear_mesh(kind, "swing_arm", &vehicle_geometry::swing_arm_unit_mesh(&kin)),
            idler: self.register_gear_mesh(kind, "idler", &idler_unit_mesh(&kin)),
            sprocket: self.register_gear_mesh(kind, "sprocket", &sprocket_unit_mesh(&kin)),
            link: self.register_gear_mesh(kind, "track_link", &track_link_unit_mesh(&kin)),
        })
    }

    fn register_gear_mesh(
        &mut self,
        kind: VehicleKind,
        part: &str,
        mesh: &GeometryMesh,
    ) -> MeshHandle {
        let asset = mesh_asset_from_geometry(mesh, Vec3::ZERO);
        let handle = self.meshes.register(format!("{}_{}", kind.slug(), part), asset);
        let mesh = self.meshes.mesh(handle).expect("registered mesh asset").clone();
        self.pending_meshes.push((handle, mesh));
        handle
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
