//! Deterministic procedural vehicle geometry.

mod bounds;
mod builder;
mod cavity;
mod lod;
mod mesh;
mod ops;
mod polygon;
mod quality;
mod recipes;
mod running_gear;
mod running_gear_geom;
mod vehicle;
mod weld;

pub use bounds::MeshBounds;
pub use builder::MeshBuilder;
pub use cavity::CavityBand;
pub use game_core::{MountFrame, MountFrames};
pub use lod::{
    LodAuditError, LodLevel, PartImportance, audit_reduction, reduce_mesh, reduce_vehicle,
};
pub use mesh::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup, SurfaceMapping};
pub use ops::{Axis, ExtrudeSpec, LoftSection, LoftSpec, ProfilePoint, RevolveSpec};
pub use polygon::{is_convex, signed_area};
pub use quality::{
    CLOSED_SMOOTH_MESH, MeshQualityError, MeshQualityReport, MeshQualitySpec, OPEN_OR_CLOSED_MESH,
    TopologyExpectation,
};
pub use recipes::bake_vehicle;
pub use running_gear::{GearPart, GearPlacement, RunningGearKinematics, running_gear_placements};
pub use running_gear_geom::{end_wheel_unit_mesh, road_wheel_unit_mesh, track_link_unit_mesh};
pub use vehicle::{BakeError, BakedVehicle, Submesh, SubmeshKind};

/// Bake `kind` and reduce it to the requested LOD in one call.
pub fn bake_vehicle_lod(
    kind: game_core::VehicleKind,
    level: LodLevel,
) -> Result<BakedVehicle, BakeError> {
    Ok(reduce_vehicle(&bake_vehicle(kind)?, level))
}
