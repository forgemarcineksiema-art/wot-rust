//! Spike bridge: mesh an [`sdf::Sdf`] into a [`vehicle_geometry::GeometryMesh`] (Surface Nets) and
//! preview it without a renderer. Hosts the two T-54 spike parts (cast turret, sharp glacis) used to
//! compare the SDF path against CAD on the gameplay-critical question: do sharp armour angles survive
//! meshing at budget. See `[[geometry-foundation-pivot]]` in project notes.

mod preview;
mod quality;
mod surface_nets;
mod t54;

pub use preview::{render_by_material, render_contact_sheet, render_png};
pub use quality::{SdfMeshingResult, SdfMeshingSpec, mesh_to_spec};
pub use surface_nets::{Grid, mesh_sdf, mesh_within_budget};
pub use t54::{t54_glacis, t54_turret};
