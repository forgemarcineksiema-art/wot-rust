//! Deterministic procedural vehicle geometry.

mod bounds;
mod builder;
mod cavity;
mod contact;
mod damage_remesh;
mod lod;
mod mesh;
mod ops;
mod polygon;
mod quality;
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
pub use running_gear::{
    GEAR_DETAIL_SWITCH_M, GearDetail, GearPart, GearPlacement, RunningGearKinematics,
    gear_detail_for_distance,
};
pub use running_gear_arms::swing_arm_unit_mesh;
pub use running_gear_end_wheels::{end_wheel_unit_mesh, idler_unit_mesh, sprocket_unit_mesh};
pub use running_gear_geom::track_link_unit_mesh;
pub use running_gear_place::{
    GearDynamics, running_gear_placements, running_gear_placements_dynamic,
    running_gear_placements_dynamic_into, thrown_remnant_placements,
};
pub use running_gear_wheels::{return_roller_unit_mesh, road_wheel_unit_mesh};
pub use vehicle::{BakeError, BakedVehicle, Submesh, SubmeshKind};
