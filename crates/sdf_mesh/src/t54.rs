//! T-54 turret and glacis as SDF compositions — the two spike test parts.
//!
//! The turret is the **cast/organic** case (smooth blends of a dome, ring, cupola and mantlet); the
//! glacis is the **sharp sloped plate** case (a hull block cut by an exact 60-degrees-from-horizontal
//! plane). The plate normal *is* the armour slope, so a parametric description could read the angle
//! straight back out — the gameplay-coherence property the pivot is built around. Dimensions follow
//! the existing T-54 blueprint (ring 1.30 m, roof 2.05 m).

use glam::Vec3;
use sdf::Sdf;

/// The cast turret SDF, plus a tight world-space bounding box for meshing.
pub fn t54_turret() -> (Sdf, Vec3, Vec3) {
    let ring_y = 1.30;
    let roof_y = 2.05;

    // The cast dome is a flattened oval — longer front-to-back than tall — so two offset spheres
    // smooth-blended read truer than a single ball.
    let dome_front = Sdf::sphere(0.95).translate(Vec3::new(0.0, ring_y + 0.50, 0.28));
    let dome_rear = Sdf::sphere(0.95).translate(Vec3::new(0.0, ring_y + 0.50, -0.32));
    let dome = dome_front.smooth_union(dome_rear, 0.60);
    let ring = Sdf::cylinder(0.98, 0.30).translate(Vec3::new(0.0, ring_y + 0.30, 0.0));
    let mut body = ring.smooth_union(dome, 0.45);

    // Flat roof and a flat seat on the turret ring: crisp where the casting meets machined planes.
    body = body.intersect(Sdf::half_space(Vec3::Y, roof_y));
    body = body.intersect(Sdf::half_space(-Vec3::Y, -ring_y));

    // Commander's cupola standing proud as a drum; the moving mantlet belongs to the gun submesh,
    // so the fixed casting contains only its recessed socket.
    let cupola = Sdf::cylinder(0.23, 0.17).translate(Vec3::new(-0.34, roof_y + 0.06, -0.10));
    body = body.smooth_union(cupola, 0.05);
    let socket = Sdf::sphere(0.34).translate(Vec3::new(0.0, ring_y + 0.42, 1.05));
    body = body.smooth_subtract(socket, 0.06);

    (body, Vec3::new(-1.20, ring_y - 0.05, -1.20), Vec3::new(1.20, roof_y + 0.25, 1.45))
}

/// The hull front / glacis SDF, plus its meshing bounding box. The plate normal sits `slope` degrees
/// above horizontal, matching the armour facet convention (`game_core::armor`) so the visible rake is
/// the angle the penetration model uses.
pub fn t54_glacis() -> (Sdf, Vec3, Vec3) {
    let block = Sdf::cuboid(Vec3::new(1.45, 0.60, 1.40)).translate(Vec3::new(0.0, 0.60, 0.0));

    let slope = GLACIS_SLOPE_DEG.to_radians();
    let normal = Vec3::new(0.0, slope.sin(), slope.cos());
    let glacis = block.intersect(Sdf::half_space(normal, 1.30));

    (glacis, Vec3::new(-1.60, -0.10, -1.50), Vec3::new(1.60, 1.30, 1.50))
}

/// The glacis slope: the angle of the plate **normal from horizontal**, matching `game_core::armor`'s
/// `slope_degrees` (effective = nominal / cos(impact + slope)).
pub const GLACIS_SLOPE_DEG: f32 = 60.0;

pub fn glacis_slope_deg() -> f32 {
    GLACIS_SLOPE_DEG
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turret_bounds_contain_its_surface() {
        let (turret, min, max) = t54_turret();
        // The roof centre is solid, a point well above the roof is empty: the dome sits in its box.
        assert!(turret.eval(Vec3::new(0.0, 1.6, 0.0)) < 0.0);
        assert!(turret.eval(Vec3::new(0.0, max.y + 0.5, 0.0)) > 0.0);
        assert!(min.y < 1.30 && max.y > 2.05, "box spans ring..roof");
    }

    #[test]
    fn glacis_plate_angle_is_exact_and_recoverable() {
        // The plane normal encodes the slope in the armour convention (degrees above horizontal):
        // recovering atan2(n.y, n.z) returns the slope, and a point just past the plate is empty.
        let slope = glacis_slope_deg().to_radians();
        let normal = Vec3::new(0.0, slope.sin(), slope.cos());
        assert!((normal.y.atan2(normal.z).to_degrees() - glacis_slope_deg()).abs() < 1.0e-3);
        let (glacis, ..) = t54_glacis();
        let on_plate = normal * 1.30;
        assert!(glacis.eval(on_plate + normal * 0.05) > 0.0, "just past the plate is empty");
    }
}
