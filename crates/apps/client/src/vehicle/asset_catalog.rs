use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use game_core::VehicleKind;
use glam::Vec3;
use renderer_api::{
    MaterialHandle, MeshHandle, VehicleMaterialDescriptor, VehicleMaterialFamilies,
    VehicleMeshAsset,
};
use vehicle_forge::authoritative_baked_vehicle;
use vehicle_geometry::{
    GeometryMesh, MeshContactIndex, RunningGearKinematics, SubmeshKind, idler_unit_mesh,
    road_wheel_unit_mesh, sprocket_unit_mesh, swing_arm_unit_mesh, track_link_unit_mesh,
};

use super::pbr_mesh::vehicle_submesh_vertices;
use super::running_gear_objects::GearMeshHandles;

#[derive(Debug, Default)]
pub struct VehicleAssetCatalog {
    mesh_labels: BTreeMap<String, MeshHandle>,
    meshes: Vec<(String, VehicleMeshAsset)>,
    pub(crate) materials: Vec<VehicleMaterialDescriptor>,
    pub(crate) material_handles: HashMap<VehicleKind, MaterialHandle>,
    pub(crate) vehicles: HashMap<VehicleKind, VehicleAssetEntry>,
    /// Per-kind visual-mesh contact indices for seating impact marks flush on the armor. Built
    /// once alongside the render meshes and shared (the mesh is one per kind), so the ingest path
    /// borrows it cheaply; the `Arc` lets a hit resolve without holding the catalog borrow.
    contact_indices: HashMap<VehicleKind, Arc<VehicleContactIndex>>,
    pending_meshes: Vec<(MeshHandle, VehicleMeshAsset)>,
    pub(crate) pending_materials: Vec<(MaterialHandle, VehicleMaterialFamilies)>,
}

/// The hull and turret contact indices for one vehicle kind, each in its own decal-local frame
/// (hull about the ground origin, turret about the ring) so a query lines up with how the client
/// stores and poses the mark.
#[derive(Debug)]
pub struct VehicleContactIndex {
    pub hull: MeshContactIndex,
    pub turret: MeshContactIndex,
    pub gun: MeshContactIndex,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct VehicleAssetEntry {
    pub(crate) hull: MeshHandle,
    pub(crate) turret: MeshHandle,
    pub(crate) gun: MeshHandle,
    pub(crate) material: MaterialHandle,
    /// Cached unit meshes for the animatable running gear; `None` for legacy (non-blueprint)
    /// vehicles, which keep their static baked gear.
    pub(crate) running_gear: Option<GearMeshHandles>,
}

impl VehicleAssetCatalog {
    pub fn take_pending_vehicle_meshes(&mut self) -> Vec<(MeshHandle, VehicleMeshAsset)> {
        std::mem::take(&mut self.pending_meshes)
    }

    pub fn take_pending_vehicle_materials(
        &mut self,
    ) -> Vec<(MaterialHandle, VehicleMaterialFamilies)> {
        std::mem::take(&mut self.pending_materials)
    }

    pub fn vehicle_material(&self, handle: MaterialHandle) -> Option<&VehicleMaterialDescriptor> {
        self.materials.get(handle.0 as usize)
    }

    pub fn cached_vehicle_count(&self) -> usize {
        self.vehicles.len()
    }

    /// The visual-mesh contact index for a kind, if its assets have been built (they are, for
    /// every playable vehicle, by `prebake_playable_vehicle_assets` at startup). Cloned `Arc` so
    /// the caller drops the catalog borrow before posing the mark.
    pub(crate) fn contact_index(&self, kind: VehicleKind) -> Option<Arc<VehicleContactIndex>> {
        self.contact_indices.get(&kind).cloned()
    }

    pub fn material_count(&self) -> usize {
        self.material_handles.len()
    }

    pub(crate) fn vehicle_entry(&mut self, kind: VehicleKind) -> Option<VehicleAssetEntry> {
        if let Some(entry) = self.vehicles.get(&kind) {
            return Some(*entry);
        }
        let vehicle = authoritative_baked_vehicle(kind).ok()?;
        let turret_ring = vehicle.mounts().turret_ring.translation;
        let trunnion = vehicle.mounts().gun_trunnion.translation;
        let hull = vehicle.submesh(SubmeshKind::Hull)?;
        let turret = vehicle.submesh(SubmeshKind::Turret)?;
        let gun = vehicle.submesh(SubmeshKind::Gun)?;
        // Build the visual-mesh contact indices in the same pivots the GPU submeshes use, so an
        // impact-mark query in the hull/turret frame lands on the surface the player sees.
        self.contact_indices.entry(kind).or_insert_with(|| {
            Arc::new(VehicleContactIndex {
                hull: MeshContactIndex::from_mesh(&hull.mesh, Vec3::ZERO),
                turret: MeshContactIndex::from_mesh(&turret.mesh, turret_ring),
                gun: MeshContactIndex::from_mesh(&gun.mesh, trunnion),
            })
        });
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
            running_gear: self.register_running_gear(kind),
        };
        self.vehicles.insert(kind, entry);
        Some(entry)
    }

    /// Register a per-instance DENTED hull for a wreck: the base hull deformed at its penetration
    /// points (hull-local). Uploaded through the same pending path as every other mesh; a distinct
    /// label per tank means a distinct handle, so the wreck draws its own dented copy without
    /// disturbing the shared base mesh other tanks of the kind still use. `None` when there is
    /// nothing to dent. See `wreck_deform` for the collision-safety contract.
    pub(crate) fn wreck_hull_mesh(
        &mut self,
        kind: VehicleKind,
        tank_id: game_core::TankId,
        penetrations_local: &[Vec3],
    ) -> Option<MeshHandle> {
        let baked = authoritative_baked_vehicle(kind).ok()?;
        let hull = baked.submesh(SubmeshKind::Hull)?;
        let dented =
            crate::vehicle::wreck_deform::dented_hull(&hull.mesh, penetrations_local, tank_id.0)?;
        let label = format!("{}_wreck{}_hull", kind.slug(), tank_id.0);
        Some(self.register_named_mesh(label, &dented, Vec3::ZERO))
    }

    /// Register (or re-upload) a mesh under an arbitrary label, returning its stable handle.
    fn register_named_mesh(
        &mut self,
        label: String,
        mesh: &GeometryMesh,
        pivot: Vec3,
    ) -> MeshHandle {
        let asset = vehicle_mesh_asset_from_geometry(mesh, pivot);
        if let Some(handle) = self.mesh_labels.get(&label).copied() {
            if let Some((_, stored)) = self.meshes.get_mut(handle.0 as usize) {
                *stored = asset.clone();
            }
            self.pending_meshes.push((handle, asset));
            return handle;
        }
        let handle = MeshHandle(self.meshes.len() as u32);
        self.mesh_labels.insert(label.clone(), handle);
        self.meshes.push((label, asset.clone()));
        self.pending_meshes.push((handle, asset));
        handle
    }

    /// Cache the three running-gear unit meshes for `kind`, or `None` if it has no blueprint gear.
    pub(crate) fn register_running_gear(&mut self, kind: VehicleKind) -> Option<GearMeshHandles> {
        let kin = RunningGearKinematics::for_vehicle(kind)?;
        Some(GearMeshHandles {
            road_wheel: self.register_gear_mesh(kind, "road_wheel", &road_wheel_unit_mesh(&kin)),
            swing_arm: self.register_gear_mesh(kind, "swing_arm", &swing_arm_unit_mesh(&kin)),
            idler: self.register_gear_mesh(kind, "idler", &idler_unit_mesh(&kin)),
            sprocket: self.register_gear_mesh(kind, "sprocket", &sprocket_unit_mesh(&kin)),
            link: self.register_gear_mesh(kind, "track_link", &track_link_unit_mesh(&kin)),
            return_roller: self.register_gear_mesh(
                kind,
                "return_roller",
                &vehicle_geometry::return_roller_unit_mesh(&kin),
            ),
        })
    }

    fn register_gear_mesh(
        &mut self,
        kind: VehicleKind,
        part: &str,
        mesh: &GeometryMesh,
    ) -> MeshHandle {
        let label = format!("{}_{}_vehicle", kind.slug(), part);
        let asset = vehicle_mesh_asset_from_geometry(mesh, Vec3::ZERO);
        if let Some(handle) = self.mesh_labels.get(&label).copied() {
            if let Some((_, stored)) = self.meshes.get_mut(handle.0 as usize) {
                *stored = asset.clone();
            }
            self.pending_meshes.push((handle, asset));
            return handle;
        }
        let handle = MeshHandle(self.meshes.len() as u32);
        self.mesh_labels.insert(label.clone(), handle);
        self.meshes.push((label, asset.clone()));
        self.pending_meshes.push((handle, asset));
        handle
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

fn vehicle_mesh_asset_from_geometry(mesh: &GeometryMesh, pivot: Vec3) -> VehicleMeshAsset {
    let (mut vertices, indices) = vehicle_submesh_vertices(mesh);
    for vertex in &mut vertices {
        let shifted = Vec3::from_array(vertex.position) - pivot;
        vertex.position = shifted.to_array();
    }
    VehicleMeshAsset::new(vertices, indices)
}

fn submesh_label(submesh: SubmeshKind) -> &'static str {
    match submesh {
        SubmeshKind::Hull => "hull",
        SubmeshKind::Turret => "turret",
        SubmeshKind::Gun => "gun",
    }
}
