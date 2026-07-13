//! Deterministic procedural vehicle geometry.

mod bounds;
mod budgets;
mod builder;
mod cavity;
mod contact;
mod damage_remesh;
mod lod;
mod mesh;
mod ops;
mod polygon;
mod quality;
mod recipes;
mod running_gear;
mod running_gear_arms;
mod running_gear_belt;
mod running_gear_end_wheels;
mod running_gear_geom;
mod running_gear_place;
mod running_gear_wheels;
mod vehicle;
mod weld;

pub use bounds::MeshBounds;
pub use budgets::{GOLDEN_BAKE_HASHES, VEHICLE_BUDGETS, VehicleBudgets, golden_bake_hash};
pub use builder::MeshBuilder;
pub use cavity::CavityBand;
pub use contact::{DecalPatch, MeshContactIndex, SurfaceContact};
pub use damage_remesh::{DamageRemeshError, remesh_aperture};
pub use game_core::{MountFrame, MountFrames};
pub use lod::{
    LodAuditError, LodLevel, PartImportance, audit_reduction, reduce_mesh, reduce_vehicle,
};
pub use mesh::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup, SurfaceMapping};
pub use ops::{
    Axis, ExtrudePolygonSpec, ExtrudeSpec, LoftSection, LoftSpec, ProfilePoint, RevolveSpec,
};
pub use polygon::{is_convex, signed_area};
pub use quality::{
    CLOSED_SMOOTH_MESH, MeshQualityError, MeshQualityReport, MeshQualitySpec, OPEN_OR_CLOSED_MESH,
    TopologyExpectation,
};
pub use recipes::{bake_vehicle, bake_vehicle_from_blueprint};
pub use running_gear::{GearPart, GearPlacement, RunningGearKinematics};
pub use running_gear_arms::swing_arm_unit_mesh;
pub use running_gear_end_wheels::{end_wheel_unit_mesh, idler_unit_mesh, sprocket_unit_mesh};
pub use running_gear_geom::track_link_unit_mesh;
pub use running_gear_place::{
    GearDynamics, running_gear_placements, running_gear_placements_dynamic,
};
pub use running_gear_wheels::{return_roller_unit_mesh, road_wheel_unit_mesh};
pub use vehicle::{BakeError, BakedVehicle, Submesh, SubmeshKind};

/// Bake `kind` and reduce it to the requested LOD in one call.
pub fn bake_vehicle_lod(
    kind: game_core::VehicleKind,
    level: LodLevel,
) -> Result<BakedVehicle, BakeError> {
    Ok(reduce_vehicle(&bake_vehicle(kind)?, level))
}
