//! T-54 turret and glacis as SDF compositions — the cast/organic arm of the hybrid kernel.
//!
//! The turret is the **cast** case (smooth blends of a flattened dome, ring, cupola and a recessed
//! mantlet socket); the glacis demo is the **sharp sloped plate** case (a block cut by an exact
//! plane whose normal is the armour slope). Every turret dimension is read from the blueprint's
//! [`TurretVisual`] — the single source — so the casting cannot drift from it.

use game_core::TurretVisual;
use glam::Vec3;
use sdf::Sdf;

/// The cast turret SDF, plus a tight world-space bounding box for meshing. Built entirely from the
/// blueprint's [`TurretVisual`]: the moving mantlet belongs to the gun submesh, so the fixed casting
/// contains only its recessed socket.
pub fn t54_turret(t: &TurretVisual) -> (Sdf, Vec3, Vec3) {
    // The cast dome is a flattened oval — longer front-to-back than tall — so two offset spheres
    // smooth-blended read truer than a single ball.
    let dome_front = Sdf::sphere(t.dome_radius).translate(t.dome_front);
    let dome_rear = Sdf::sphere(t.dome_radius).translate(t.dome_rear);
    let dome = dome_front.smooth_union(dome_rear, t.dome_blend);
    let ring = Sdf::cylinder(t.ring_radius, t.ring_half_height).translate(t.ring_center);
    let mut body = ring.smooth_union(dome, t.ring_blend);

    // Flat roof and a flat seat on the turret ring: crisp where the casting meets machined planes.
    body = body.intersect(Sdf::half_space(Vec3::Y, t.roof_plane_y));
    body = body.intersect(Sdf::half_space(-Vec3::Y, -t.ring_plane_y));

    // Commander's cupola standing proud as a drum; the recessed mantlet socket is subtracted.
    let cupola = Sdf::cylinder(t.cupola_radius, t.cupola_half_height).translate(t.cupola_center);
    body = body.smooth_union(cupola, t.cupola_blend);
    let socket = Sdf::sphere(t.socket_radius).translate(t.socket_center);
    body = body.smooth_subtract(socket, t.socket_blend);

    (body, t.bbox_min, t.bbox_max)
}

/// The hull front / glacis SDF demo, plus its meshing box. The plate normal sits `slope_deg` above
/// horizontal, matching the armour facet convention (`game_core::armor`) so the visible rake is the
/// angle the penetration model uses. A representative block — the SDF counterpart of the CAD glacis.
pub fn t54_glacis(slope_deg: f32) -> (Sdf, Vec3, Vec3) {
    let block = Sdf::cuboid(Vec3::new(1.45, 0.60, 1.40)).translate(Vec3::new(0.0, 0.60, 0.0));
    let slope = slope_deg.to_radians();
    let normal = Vec3::new(0.0, slope.sin(), slope.cos());
    let glacis = block.intersect(Sdf::half_space(normal, 1.30));
    (glacis, Vec3::new(-1.60, -0.10, -1.50), Vec3::new(1.60, 1.30, 1.50))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turret() -> TurretVisual {
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951)
            .unwrap()
            .hybrid()
            .unwrap()
            .turret
    }

    #[test]
    fn turret_bounds_contain_its_surface() {
        let (turret, min, max) = t54_turret(&turret());
        // The roof centre is solid, a point well above the roof is empty: the dome sits in its box.
        assert!(turret.eval(Vec3::new(0.0, 1.6, 0.0)) < 0.0);
        assert!(turret.eval(Vec3::new(0.0, max.y + 0.5, 0.0)) > 0.0);
        assert!(min.y < 1.30 && max.y > 2.05, "box spans ring..roof");
    }

    #[test]
    fn glacis_plate_angle_is_exact_and_recoverable() {
        // The plane normal encodes the slope in the armour convention (degrees above horizontal):
        // recovering atan2(n.y, n.z) returns the slope, and a point just past the plate is empty.
        let bp =
            game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951).unwrap();
        let slope_deg = bp.armor.hull_front.0;
        let slope = slope_deg.to_radians();
        let normal = Vec3::new(0.0, slope.sin(), slope.cos());
        assert!((normal.y.atan2(normal.z).to_degrees() - slope_deg).abs() < 1.0e-3);
        let (glacis, ..) = t54_glacis(slope_deg);
        let on_plate = normal * 1.30;
        assert!(glacis.eval(on_plate + normal * 0.05) > 0.0, "just past the plate is empty");
    }
}
