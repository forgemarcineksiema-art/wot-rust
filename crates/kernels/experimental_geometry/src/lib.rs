//! Offline-only geometry experiments.
//!
//! This crate may try CSG, SDF, and CAD kernels, but stabilized output must be converted to
//! `vehicle_geometry::GeometryMesh` before anything reaches runtime or client crates.

use vehicle_geometry::{GeometryMesh, MeshBuilder};

pub fn empty_geometry_mesh() -> GeometryMesh {
    MeshBuilder::new().build()
}

#[cfg(feature = "csg-csgrs")]
pub mod csg {
    pub const BACKEND: &str = "csgrs";
}

#[cfg(feature = "sdf-fidget")]
pub mod sdf {
    pub const BACKEND: &str = "fidget";
}

#[cfg(feature = "cad-truck")]
pub mod cad {
    pub const BACKEND: &str = "truck-modeling";
}
