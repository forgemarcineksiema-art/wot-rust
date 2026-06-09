use glam::Vec2;

use crate::{MaterialRole, SmoothingGroup};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProfilePoint {
    pub radius: f32,
    pub offset: f32,
}

impl ProfilePoint {
    pub const fn new(radius: f32, offset: f32) -> Self {
        Self { radius, offset }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RevolveSpec {
    pub profile: Vec<ProfilePoint>,
    pub axis: Axis,
    pub segments: usize,
    pub material: MaterialRole,
    pub smoothing: SmoothingGroup,
}

/// Sweep a flat **convex** cross-section a fixed depth along an axis, producing a closed prism
/// with capped ends. This is the kernel's loft/wedge primitive: unlike [`super::MeshBuilder::chamfered_prism`]
/// (whose front and back faces are always flat caps) an extrusion lets the cross-section itself
/// carry the silhouette, so a side profile can describe a sloped glacis, a raked casemate front,
/// or a tapered turret in one shape.
///
/// `section` points live in the 2D plane perpendicular to `axis`, mapped as `(u, v)`:
/// - [`Axis::X`] → `(z, y)`: the vehicle **side** profile (forward, up), swept across the width.
/// - [`Axis::Y`] → `(x, z)`: the **plan** outline (right, forward), swept through the height.
/// - [`Axis::Z`] → `(x, y)`: the **front** profile (right, up), swept along the length.
///
/// The polygon may be wound either way — the builder reorients faces to point outward — but it
/// must be convex; caps are fanned from the section centroid.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtrudeSpec {
    pub section: Vec<Vec2>,
    pub axis: Axis,
    /// The section is swept from `-half_depth` to `+half_depth` along `axis`.
    pub half_depth: f32,
    pub material: MaterialRole,
    pub smoothing: SmoothingGroup,
}
