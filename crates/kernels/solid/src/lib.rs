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

/// THE BEVEL LAW: chamfer widths by how the edge was made.
///
/// No manufactured edge is perfectly sharp, and the arris is where light lives — it is the only
/// surface on a hard edge facing the sky, so it is the bright line a viewer reads as "this steel
/// has thickness". These are not taste; they are what the process leaves.
///
/// The OPERATOR that applies them is [`crate::chamfered_box`], which this crate has had all
/// along. A general "chamfer any convex solid" pass was built against this table and withdrawn:
/// see the note on that function.
pub mod chamfer {
    /// A machined face edge — broken by a tool pass, barely there.
    pub const MACHINED: f32 = 0.001;
    /// Rolled plate, sheared or saw-cut.
    pub const ROLLED_PLATE: f32 = 0.003;
    /// Flame-cut plate: the torch leaves a wider, rougher arris.
    pub const FLAME_CUT: f32 = 0.006;
    /// A casting's edge — the mould's own draft, an order above the others, which is why a cast
    /// turret has no sharp edge on it anywhere.
    pub const CAST: f32 = 0.020;
}

pub use convex::{ConvexSolid, ConvexSolidError, Plane};
pub use t54::{
    t54_deck_grille, t54_engine_deck_panels, t54_hull_solid, t54_lower_tub, t54_upper_hull,
};
pub use t54_fittings::{
    chamfered_box, t54_exhaust_housing, t54_fender_brackets, t54_fender_slope, t54_periscope,
    t54_periscope_guards, t54_periscope_prism,
};
pub use t54_plates::{t54_hull_plate_seams, t54_transmission_covers};
