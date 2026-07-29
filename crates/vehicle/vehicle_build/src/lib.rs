//! The parametric vehicle-description layer: the spine that ties the hybrid geometry together.
//!
//! A vehicle is a list of [`VehiclePart`]s, each routed to the generator its nature wants — flat
//! armour plates to exact CAD ([`solid`]), cast castings to the SDF ([`sdf_mesh`]) — then merged by
//! submesh kind into one `BakedVehicle` for the Forge. The same parametric dimensions drive both the
//! visible geometry and the armour facets, so gameplay stays coherent with what is rendered. See
//! `[[geometry-foundation-pivot]]` in project notes. Round parts (barrel, road wheels via a Revolve
//! generator) and the track belt slot in here next.

mod attachment;
mod description;
mod manifest;
mod part;
mod surface_bake;
mod t54;
mod t54_chassis;
mod t54_details;
mod t54_dshk;
mod t54_gun_cover;
mod t54_interior;
mod t54_interior_detail;
mod t54_kit;
mod t54_kit_lines;
mod t54_turret_loft;

pub use attachment::{SurfaceAttachment, t54_attachments};
pub use description::VehicleDescription;
pub use manifest::{
    GameplayRole, PartManifestEntry, PartManifestError, part_manifest, validate_manifest,
};
pub use part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart, VisualTolerance};
pub use surface_bake::{NamedCavity, SurfaceBake, t54_surface_bake};
pub use t54::{
    MEDIUM_LOD0_TRI_BUDGET, t54_description, t54_description_from_blueprint, t54_from_modules,
    t54_from_modules_with_blueprint,
};
pub use t54_turret_loft::t54_turret_loft;
