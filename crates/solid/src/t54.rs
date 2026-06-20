//! The T-54 hull and its appendage plates as convex solids — the CAD/B-rep arm of the hybrid kernel.
//!
//! Every dimension is read from the vehicle blueprint's [`HullVisual`] / [`BoxVisual`] /
//! [`FenderVisual`]; the plate *slopes* are passed in from the blueprint's armour facets, so the
//! visible rake of each facet is the same angle the penetration model uses — what you see is what
//! you shoot.

use game_core::{BoxVisual, FenderVisual, HullVisual};
use glam::Vec3;

use crate::{ConvexSolid, Plane};

/// The full hull as a convex solid: a block whose glacis, sloped sides and sloped rear carry the
/// blueprint armour angles (degrees of the plate normal above horizontal), plus a lower nose bevel.
pub fn t54_hull_solid(hull: &HullVisual, front_deg: f32, side_deg: f32, rear_deg: f32) -> ConvexSolid {
    let (front, side, rear) =
        (front_deg.to_radians(), side_deg.to_radians(), rear_deg.to_radians());
    let (hx, belly, roof, hz) = (hull.half_width, hull.belly_y, hull.roof_y, hull.half_len);
    let side_off = hx * side.cos() + belly * side.sin();
    ConvexSolid::new(vec![
        Plane::new(Vec3::new(0.0, -1.0, 0.0), -belly),
        Plane::new(Vec3::new(0.0, 1.0, 0.0), roof),
        Plane::new(Vec3::new(side.cos(), side.sin(), 0.0), side_off),
        Plane::new(Vec3::new(-side.cos(), side.sin(), 0.0), side_off),
        Plane::new(Vec3::new(0.0, rear.sin(), -rear.cos()), belly * rear.sin() + hz * rear.cos()),
        Plane::new(Vec3::new(0.0, front.sin(), front.cos()), hull.glacis_offset),
        Plane::new(hull.nose_normal, hull.nose_offset),
    ])
}

/// The hull front / glacis as a two-plate solid: the hull block clipped by the upper glacis plate
/// (normal `slope` degrees above horizontal) and the lower nose bevel. The CAD counterpart of the
/// SDF glacis in `sdf_mesh`, built from the same dimensions for a like-for-like sharpness comparison.
pub fn t54_glacis_solid(hull: &HullVisual, slope_deg: f32) -> ConvexSolid {
    let slope = slope_deg.to_radians();
    let center = Vec3::new(0.0, 0.5 * (hull.belly_y + hull.roof_y), 0.0);
    let half = Vec3::new(hull.half_width, 0.5 * (hull.roof_y - hull.belly_y), hull.half_len);
    let glacis = Plane::new(Vec3::new(0.0, slope.sin(), slope.cos()), hull.glacis_offset);
    let nose = Plane::new(hull.nose_normal, hull.nose_offset);
    ConvexSolid::box_at(center, half).clipped_by(glacis).clipped_by(nose)
}

/// A thin fender (mudguard) plate riding above one track run, at world `side_x`.
pub fn t54_fender(side_x: f32, fender: &FenderVisual) -> ConvexSolid {
    ConvexSolid::box_at(Vec3::new(side_x, fender.center_y, 0.0), fender.half)
}

/// The raised rear engine deck panel behind the turret.
pub fn t54_engine_deck(deck: &BoxVisual) -> ConvexSolid {
    ConvexSolid::box_at(deck.center, deck.half)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vehicle_geometry::{MaterialRole, SmoothingGroup};

    fn hull() -> HullVisual {
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951)
            .unwrap()
            .hybrid()
            .unwrap()
            .hull
    }

    #[test]
    fn the_glacis_solid_meshes_to_a_handful_of_crisp_triangles() {
        let bp =
            game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951).unwrap();
        let mesh = t54_glacis_solid(&hull(), bp.armor.hull_front.0)
            .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges());
        assert!(mesh.triangle_count() > 0, "glacis solid is empty");
        assert!(
            mesh.triangle_count() < 40,
            "exact convex glacis stays tiny: {}",
            mesh.triangle_count()
        );
    }

    #[test]
    fn the_hull_solid_carries_the_armour_glacis_angle() {
        let bp =
            game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951).unwrap();
        let mesh =
            t54_hull_solid(&hull(), bp.armor.hull_front.0, bp.armor.hull_side.0, bp.armor.hull_rear.0)
                .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges());
        let on_slope = mesh.vertices().iter().any(|v| {
            let n = v.normal;
            n.y > 0.2
                && n.z > 0.2
                && (n.y.atan2(n.z).to_degrees() - bp.armor.hull_front.0).abs() < 2.0
        });
        assert!(on_slope, "a glacis face normal carries the blueprint armour slope");
    }
}
