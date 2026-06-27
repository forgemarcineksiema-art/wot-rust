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
    // smooth-blended read truer than a single ball. The rear sphere is smaller, tapering the casting
    // to a lower, narrower bustle behind the ring.
    let dome_front = Sdf::sphere(t.dome_radius).translate(t.dome_front);
    let dome_rear = Sdf::sphere(t.dome_rear_radius).translate(t.dome_rear);
    let dome = dome_front.smooth_union(dome_rear, t.dome_blend);

    // The signature front cheeks: a pair of bulges flanking the mantlet, blended into the dome so the
    // front mass reads heavier than the tapered rear (the T-54 turret's front-to-rear asymmetry).
    let cheek_right = Sdf::sphere(t.cheek_radius).translate(t.cheek_center);
    let cheek_left = Sdf::sphere(t.cheek_radius).translate(Vec3::new(
        -t.cheek_center.x,
        t.cheek_center.y,
        t.cheek_center.z,
    ));
    let cheeks = cheek_right.smooth_union(cheek_left, t.cheek_blend);
    let dome = dome.smooth_union(cheeks, t.cheek_blend);

    let ring = Sdf::cylinder(t.ring_radius, t.ring_half_height).translate(t.ring_center);
    let mut body = ring.smooth_union(dome, t.ring_blend);

    // Flat roof and a flat seat on the turret ring: crisp where the casting meets machined planes.
    body = body.intersect(Sdf::half_space(Vec3::Y, t.roof_plane_y));
    body = body.intersect(Sdf::half_space(-Vec3::Y, -t.ring_plane_y));

    // Commander's cupola standing proud as a drum, blended into the casting (a cast-only transition,
    // like the cheeks, socket and ring — every turret join here is a smooth blend). The recessed
    // mantlet socket is subtracted as a short vertical trough — the main socket plus a lower lobe —
    // so the cavity the gun mantlet beds into reads low and deep. Hard armour seams and the sloped
    // glacis are never SDF: those are exact CAD plates from the `solid` generator.
    let cupola = Sdf::cylinder(t.cupola_radius, t.cupola_half_height).translate(t.cupola_center);
    body = body.smooth_union(cupola, t.cupola_blend);
    // The cavity is carved to *match the wide oval mantlet*, not a round ball: two lateral lobes
    // widen it so the mantlet's broad flanks bed in with clearance, and a lower lobe gives the rear
    // room to swing down through full elevation. Sized so the moving mantlet never digs into the
    // casting across the gun's -5..+18 deg arc (locked by the clearance test in vehicle_build).
    let side = t.socket_radius * 0.78;
    // The widening lobes sit DEEPER (back along -Z) than the mouth: they give the gun mantlet swing
    // clearance inside the casting without enlarging the opening on the front face, so the moving
    // mask covers a tight mouth while the cavity behind it stays roomy across the elevation arc.
    let depth = Vec3::new(0.0, 0.0, -t.socket_radius * 0.5);
    let socket_low =
        Vec3::new(t.socket_center.x, t.socket_center.y - t.socket_radius * 0.55, t.socket_center.z)
            + depth;
    let lobe = |offset: Vec3, scale: f32| Sdf::sphere(t.socket_radius * scale).translate(offset);
    let socket = Sdf::sphere(t.socket_radius)
        .translate(t.socket_center)
        .smooth_union(lobe(t.socket_center + Vec3::X * side + depth, 0.92), t.socket_blend)
        .smooth_union(lobe(t.socket_center - Vec3::X * side + depth, 0.92), t.socket_blend)
        .smooth_union(lobe(socket_low, 0.85), t.socket_blend);
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

    /// March outward from the turret axis on the z=0 line at height `y`, returning the half-width
    /// (first surface crossing). Used to read the cast dome's profile.
    fn half_width_at(sdf: &sdf::Sdf, y: f32) -> f32 {
        let mut x = 0.0_f32;
        while x < 1.4 {
            if sdf.eval(Vec3::new(x, y, 0.0)) > 0.0 {
                return x;
            }
            x += 0.005;
        }
        x
    }

    #[test]
    fn cast_dome_bulges_low_and_flattens_toward_the_roof() {
        // The T-54 turret is a flattened "river-stone" casting: widest LOW (overhanging the ring),
        // necking in toward a flat roof — not a tall round pot whose widest point sits high. This
        // locks that profile so a future tweak can't silently restore the tall dome.
        let t = turret();
        let (sdf, _, _) = t54_turret(&t);
        let band = t.roof_plane_y - t.ring_plane_y;
        let mid = t.ring_plane_y + 0.5 * band;

        let w_lo = half_width_at(&sdf, t.ring_plane_y + 0.08);
        let w_hi = half_width_at(&sdf, t.roof_plane_y - 0.06);
        assert!(
            w_lo > w_hi + 0.05,
            "casting must bulge low and neck in toward the flat roof: w_lo {w_lo:.2} vs w_hi {w_hi:.2}"
        );

        // The widest sampled cross-section must live in the lower half of the ring..roof band.
        let widest_y = (0..=10)
            .map(|i| t.ring_plane_y + band * i as f32 / 10.0)
            .map(|y| (y, half_width_at(&sdf, y)))
            .fold((0.0_f32, 0.0_f32), |a, b| if b.1 > a.1 { b } else { a })
            .0;
        assert!(widest_y < mid, "widest cross-section must sit low: {widest_y:.2} vs mid {mid:.2}");
    }

    #[test]
    fn turret_bounds_contain_its_surface() {
        let t = turret();
        let (turret, min, max) = t54_turret(&t);
        // The roof centre is solid, a point well above the roof is empty: the dome sits in its box.
        assert!(turret.eval(Vec3::new(0.0, 1.6, 0.0)) < 0.0);
        assert!(turret.eval(Vec3::new(0.0, max.y + 0.5, 0.0)) > 0.0);
        // The box spans from below the machined ring seat to above the (lowered) flat roof plane —
        // tied to the blueprint single source, not a fixed height, so it tracks the pancake casting.
        assert!(min.y < t.ring_plane_y && max.y > t.roof_plane_y, "box spans ring..roof");
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
