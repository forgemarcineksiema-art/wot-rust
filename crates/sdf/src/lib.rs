//! Constructive signed-distance-field geometry kernel (renderer-free).
//!
//! Spike foundation for the geometry pivot ([[geometry-foundation-pivot]] in project notes): build
//! a vehicle shape as a CSG tree of analytic SDF primitives, mixing **hard** ops (crisp armour
//! plates whose slope is exact and recoverable) with **smooth** blends (cast turrets, mantlets).
//! Every node evaluates to a signed distance: negative inside, positive outside, zero on the
//! surface. Meshing (dual contouring / surface nets) and the parametric vehicle-description layer
//! build on top of this kernel; the kernel itself never touches the renderer.

mod node;
mod shape;

pub use node::Sdf;
