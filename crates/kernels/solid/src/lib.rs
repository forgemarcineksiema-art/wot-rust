//! Constructive convex-solid geometry (renderer-free): the CAD/B-rep arm of the geometry spike.
//!
//! Where `sdf` represents shapes as a sampled distance field meshed by Surface Nets (smooth, soft
//! edges), this crate represents a convex solid as exact half-spaces triangulated to a crisp
//! boundary — perfect flat plates and exact armour angles, at a handful of triangles. The spike
//! compares the two on the glacis. See `[[geometry-foundation-pivot]]` in project notes.

mod convex;
mod t54;
mod t54_fittings;
mod t54_plates;
mod validate;

pub use convex::{ConvexSolid, ConvexSolidError, Plane};
pub use t54::{
    t54_deck_grille, t54_engine_deck_panels, t54_hull_solid, t54_lower_tub, t54_upper_hull,
};
pub use t54_fittings::{
    chamfered_box, t54_exhaust_housing, t54_fender_brackets, t54_fender_slope, t54_periscope,
    t54_periscope_guards, t54_periscope_prism,
};
pub use t54_plates::{t54_hull_plate_seams, t54_transmission_covers};
