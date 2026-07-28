//! Offline-only geometry experiments.
//!
//! This crate is the sanctioned place to try a CSG, SDF or CAD kernel against the fleet without
//! letting it near the runtime: stabilized output must be converted to
//! `vehicle_geometry::GeometryMesh` before anything reaches a runtime or client crate.
//!
//! **A backend lives here only while it is being tried.** Declaring an optional dependency that
//! does not build breaks `--all-features` for the whole workspace — third-party compile errors
//! in a standard invocation, on code nobody asked for. To open a trial, add the pair back:
//!
//! ```toml
//! [features]
//! sdf-fidget = ["dep:fidget"]
//! [dependencies]
//! fidget = { version = "...", optional = true }
//! ```
//!
//! …and take both out again when the trial ends, whichever way it went.

use vehicle_geometry::{GeometryMesh, MeshBuilder};

pub fn empty_geometry_mesh() -> GeometryMesh {
    MeshBuilder::new().build()
}
