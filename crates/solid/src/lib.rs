//! Constructive convex-solid geometry (renderer-free): the CAD/B-rep arm of the geometry spike.
//!
//! Where `sdf` represents shapes as a sampled distance field meshed by Surface Nets (smooth, soft
//! edges), this crate represents a convex solid as exact half-spaces triangulated to a crisp
//! boundary — perfect flat plates and exact armour angles, at a handful of triangles. The spike
//! compares the two on the glacis. See `[[geometry-foundation-pivot]]` in project notes.

mod convex;
mod t54;

pub use convex::{ConvexSolid, Plane};
pub use t54::{t54_engine_deck, t54_fender, t54_glacis_solid, t54_hull_solid};
