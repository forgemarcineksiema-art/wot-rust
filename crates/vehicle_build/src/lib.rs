//! The parametric vehicle-description layer: the spine that ties the hybrid geometry together.
//!
//! A vehicle is a list of [`VehiclePart`]s, each routed to the generator its nature wants — flat
//! armour plates to exact CAD ([`solid`]), cast castings to the SDF ([`sdf_mesh`]) — then merged by
//! submesh kind into one `BakedVehicle` for the Forge. The same parametric dimensions drive both the
//! visible geometry and the armour facets, so gameplay stays coherent with what is rendered. See
//! `[[geometry-foundation-pivot]]` in project notes. Round parts (barrel, road wheels via a Revolve
//! generator) and the track belt slot in here next.

mod description;
mod part;
mod t54;

pub use description::VehicleDescription;
pub use part::{PartLod, PartShape, VehiclePart};
pub use t54::{MEDIUM_LOD0_TRI_BUDGET, t54_description, t54_from_modules};
