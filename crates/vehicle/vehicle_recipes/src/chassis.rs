//! Shared chassis helpers. Blueprint-born hulls live in [`super::chassis_blueprint`]; this
//! module keeps the lower-gear contact shade every recipe still applies.

use vehicle_geometry::GeometryMesh;

// Shared lower-gear contact shading: darken low/occluded running gear without touching upper
// armour. Pinned for the T-54 by `t54_surface_shading_darkens_lower_running_gear`.
const SHADE_FLOOR_Y: f32 = -0.05;
const SHADE_BRIGHT_Y: f32 = 1.05;
const SHADE_LOW: f32 = 0.70;

/// Apply the shared lower-gear contact shading to a finished hull mesh.
pub(crate) fn shade_hull(mesh: GeometryMesh) -> GeometryMesh {
    mesh.with_height_shade(SHADE_FLOOR_Y, SHADE_BRIGHT_Y, SHADE_LOW)
}
