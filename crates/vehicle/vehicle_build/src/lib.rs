//! The parametric vehicle-description layer: the spine that ties the hybrid geometry together.
//!
//! A vehicle is a list of [`VehiclePart`]s, each routed to the generator its nature wants — flat
//! armour plates to exact CAD ([`solid`]), the cast turret to a station loft ([`cast_loft`]), the
//! fenders to a folded pressing ([`panel`]), round parts to [`revolve`], covers and kit lines to
//! [`sweep`] — then merged by submesh kind into one `BakedVehicle` for the Forge. The
//! [`PartShape::Cast`] arm still meshes through [`sdf_mesh`], but no part uses it (K4 decides
//! whether it earns a call site or goes). The same parametric dimensions drive both the visible
//! geometry and the armour facets, so gameplay stays coherent with what is rendered. Only the T-54
//! is described here today; the other seven vehicles bake through `vehicle_recipes` (K3).

mod attachment;
mod description;
mod inventory;
mod manifest;
mod part;
mod parts_fittings;
mod parts_hull;
mod parts_plates;
mod surface_bake;
mod t54;
mod t54_chassis;
mod t54_details;
mod t54_dshk;
mod t54_fender;
mod t54_gun_cover;
mod t54_interior;
mod t54_interior_detail;
mod t54_kit;
mod t54_kit_lines;
mod t54_turret_loft;

pub use attachment::{SurfaceAttachment, t54_attachments};
pub use description::{
    Fidelity, LodStrategy, PostMerge, VehicleDescription, description_for, description_for_modules,
    description_from_blueprint,
};
pub use inventory::{
    CarriedInventory, DossierPartList, ExpectedPart, InventoryReport, InventorySpec, PartClass,
    inventory_for,
};
pub use manifest::{
    GameplayRole, PartManifestEntry, PartManifestError, part_manifest, validate_manifest,
};
pub use part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart, VisualTolerance};
pub use parts_fittings::{
    exhaust_housing, fender_brackets, flap_ribs, periscope, periscope_guards, periscope_prism,
};
pub use parts_hull::{
    deck_grille, engine_deck_panels, hull_solid, lower_tub_solid, upper_hull_solid,
};
pub use parts_plates::{hull_plate_seams, transmission_covers};
pub use surface_bake::{NamedCavity, SurfaceBake, t54_surface_bake};
pub use t54::{
    MEDIUM_LOD0_TRI_BUDGET, MEDIUM_LOD0_VERT_BUDGET, t54_description,
    t54_description_from_blueprint, t54_from_modules, t54_from_modules_with_blueprint,
};
pub use t54_details::fitting_parts_for_blueprint;
pub use t54_fender::fender_parts_for_blueprint;
pub use t54_turret_loft::t54_turret_loft;
