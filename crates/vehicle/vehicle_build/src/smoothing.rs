//! The smoothing groups the recipe fleet and the library's fittings share (one number, one
//! meaning, on both sides of the seam).

use vehicle_geometry::SmoothingGroup;

pub const SG_HARD: SmoothingGroup = SmoothingGroup::hard_edges();
pub const SG_CAST: SmoothingGroup = SmoothingGroup(2);
pub const SG_CUPOLA: SmoothingGroup = SmoothingGroup(3);
pub const SG_BARREL: SmoothingGroup = SmoothingGroup(4);
pub const SG_MANTLET: SmoothingGroup = SmoothingGroup(6);
pub const SG_RING: SmoothingGroup = SmoothingGroup(7);
