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

/// One convex cross-section of a [`LoftSpec`], positioned at `offset` along the loft axis. The
/// `section` points live in the plane perpendicular to the axis, mapped `(u, v)` exactly as
/// [`ExtrudeSpec`] documents.
#[derive(Debug, Clone, PartialEq)]
pub struct LoftSection {
    pub offset: f32,
    pub section: Vec<Vec2>,
}

impl LoftSection {
    pub fn new(offset: f32, section: Vec<Vec2>) -> Self {
        Self { offset, section }
    }
}

/// Loft a surface through a sequence of convex cross-sections placed along an axis. Unlike
/// [`ExtrudeSpec`] — a single section swept at a constant size — a loft lets the section *change*
/// from station to station, so one shape can describe a hull that tapers toward the bow, a cast
/// turret that swells at the shoulder and narrows to the roof, or a casemate that rakes forward.
///
/// Every section must carry the same point count (rings connect 1:1) and be convex; the builder
/// reorients each ring to CCW and asserts convexity, then connects consecutive rings with one quad
/// per edge and optionally fans flat caps over the first and last sections.
#[derive(Debug, Clone, PartialEq)]
pub struct LoftSpec {
    pub sections: Vec<LoftSection>,
    pub axis: Axis,
    pub material: MaterialRole,
    pub smoothing: SmoothingGroup,
    /// Fan flat caps over the first and last sections (a closed solid) when `true`.
    pub cap_ends: bool,
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

/// Sweep a flat polygon, including concave outlines and optional holes, a fixed depth along an
/// axis. The points are a single Earcut-compatible list: first the outer ring, followed by each
/// hole ring; `hole_indices` stores the starting index of every hole in that list.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtrudePolygonSpec {
    pub points: Vec<Vec2>,
    pub hole_indices: Vec<usize>,
    pub axis: Axis,
    /// The section is swept from `-half_depth` to `+half_depth` along `axis`.
    pub half_depth: f32,
    pub material: MaterialRole,
    pub smoothing: SmoothingGroup,
}
