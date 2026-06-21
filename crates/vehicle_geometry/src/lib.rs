//! Deterministic procedural vehicle geometry.

mod bounds;
mod builder;
mod lod;
mod mesh;
mod ops;
mod quality;
mod recipes;
mod vehicle;

pub use bounds::MeshBounds;
pub use builder::MeshBuilder;
pub use game_core::{MountFrame, MountFrames};
pub use lod::{LodLevel, reduce_vehicle};
pub use mesh::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};
pub use ops::{Axis, ExtrudeSpec, LoftSection, LoftSpec, ProfilePoint, RevolveSpec};
pub use quality::{
    CLOSED_SMOOTH_MESH, MeshQualityError, MeshQualityReport, MeshQualitySpec, OPEN_OR_CLOSED_MESH,
    TopologyExpectation,
};
pub use recipes::bake_vehicle;
pub use vehicle::{BakeError, BakedVehicle, Submesh, SubmeshKind};

/// Bake `kind` and reduce it to the requested LOD in one call.
pub fn bake_vehicle_lod(
    kind: game_core::VehicleKind,
    level: LodLevel,
) -> Result<BakedVehicle, BakeError> {
    Ok(reduce_vehicle(&bake_vehicle(kind)?, level))
}
