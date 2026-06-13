//! Deterministic procedural vehicle geometry.

mod bounds;
mod builder;
mod mesh;
mod ops;
mod recipes;
mod vehicle;

pub use bounds::MeshBounds;
pub use builder::MeshBuilder;
pub use game_core::{MountFrame, MountFrames};
pub use mesh::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};
pub use ops::{Axis, ExtrudeSpec, LoftSection, LoftSpec, ProfilePoint, RevolveSpec};
pub use recipes::bake_vehicle;
pub use vehicle::{BakeError, BakedVehicle, Submesh, SubmeshKind};
