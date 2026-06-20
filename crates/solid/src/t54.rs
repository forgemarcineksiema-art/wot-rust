//! The T-54 glacis as a convex solid — the CAD counterpart to the SDF glacis in `sdf_mesh`, built
//! from the same dimensions and the same 60-degrees-from-horizontal plate, for a like-for-like
//! sharpness comparison.

use glam::Vec3;

use crate::{ConvexSolid, Plane};

/// The glacis slope in degrees, matching the armour facet convention in `game_core::armor`:
/// `slope_degrees` is the angle of the plate **normal from horizontal** (the shell direction), so
/// effective armour = nominal / cos(impact + slope). 60° → a steeply raked, ~2x-effective glacis.
pub const GLACIS_SLOPE_DEG: f32 = 60.0;

/// The full-length hull block (1.45 x 0.55 x 2.90 half-extents at y = 0.65) clipped by the glacis
/// plate. The plate normal is `slope` degrees above horizontal, so the visible rake matches the
/// angle the armour model uses — what you see is what you shoot.
pub fn t54_glacis_solid() -> ConvexSolid {
    let slope = GLACIS_SLOPE_DEG.to_radians();
    let glacis = Plane::new(Vec3::new(0.0, slope.sin(), slope.cos()), 1.85);
    // A lower nose plate bevels the bottom-front edge — the T-54's two-plate front.
    let nose = Plane::new(Vec3::new(0.0, -0.5, 1.0), 2.55);
    ConvexSolid::box_at(Vec3::new(0.0, 0.65, 0.0), Vec3::new(1.45, 0.55, 2.90))
        .clipped_by(glacis)
        .clipped_by(nose)
}

/// A thin fender (mudguard) plate riding above one track run.
pub fn t54_fender(side_x: f32) -> ConvexSolid {
    ConvexSolid::box_at(Vec3::new(side_x, 0.90, 0.0), Vec3::new(0.28, 0.03, 2.55))
}

/// The raised rear engine deck panel behind the turret.
pub fn t54_engine_deck() -> ConvexSolid {
    ConvexSolid::box_at(Vec3::new(0.0, 1.25, -1.75), Vec3::new(1.30, 0.06, 0.95))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vehicle_geometry::{MaterialRole, SmoothingGroup};

    #[test]
    fn the_glacis_solid_meshes_to_a_handful_of_crisp_triangles() {
        let mesh =
            t54_glacis_solid().to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges());
        assert!(mesh.triangle_count() > 0, "glacis solid is empty");
        assert!(
            mesh.triangle_count() < 40,
            "exact convex glacis stays tiny: {}",
            mesh.triangle_count()
        );
    }
}
