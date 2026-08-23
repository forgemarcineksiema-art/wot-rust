//! Shared bake fixtures for the fleet-wide recipe tests.
#![allow(dead_code)]

use game_core::VehicleKind;
use vehicle_geometry::{BakedVehicle, MeshBounds, SubmeshKind};
use vehicle_recipes::bake_vehicle;

/// Every vehicle in the roster, baked, or the panic that names the one that would not.
pub fn bake_all() -> Vec<BakedVehicle> {
    VehicleKind::ALL
        .into_iter()
        .map(|kind| bake_vehicle(kind).unwrap_or_else(|e| panic!("{kind:?} should bake: {e}")))
        .collect()
}

/// The named submesh's bounds, or the panic that names what is missing.
pub fn submesh_bounds(vehicle: &BakedVehicle, kind: SubmeshKind) -> MeshBounds {
    vehicle
        .submesh(kind)
        .unwrap_or_else(|| panic!("{:?} missing {kind:?} submesh", vehicle.kind()))
        .mesh
        .bounds()
        .unwrap_or_else(|| panic!("{:?} {kind:?} submesh has no bounds", vehicle.kind()))
}
