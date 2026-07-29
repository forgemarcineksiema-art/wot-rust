//! The T-54 cast turret as a **cast loft** shell — the controlled-surface replacement for the
//! metaball `sdf_mesh::t54_turret`. Every dimension is read from the blueprint's
//! [`TurretLoftVisual`] (the single source) and skinned by the [`cast_loft`] kernel; the cupola and
//! moving mantlet stay separate bedded parts, as before.

use std::f32::consts::FRAC_PI_2;

use cast_loft::{CastBump, CastCap, CastCaps, CastLoftSpec, CastSection, try_build_cast_loft};
use game_core::TurretLoftVisual;
use vehicle_geometry::{GeometryMesh, MaterialRole, SmoothingGroup};

/// Build the lofted T-54 turret casting: the blueprint stations skinned into one shell, with the
/// symmetric front cheeks and the gun embrasure carried as radial modulations of that one surface.
pub fn t54_turret_loft(t: &TurretLoftVisual) -> GeometryMesh {
    let sections: Vec<CastSection> = t
        .stations
        .iter()
        .map(|s| CastSection {
            y: s.y,
            half_width: s.half_width,
            half_len_front: s.half_len_front,
            half_len_rear: s.half_len_rear,
            z_center: s.z_center,
            exponent: t.exponent,
        })
        .collect();

    // The cheeks are a cast SWELL — the soft Gaussian is exactly right for them.
    let cheek = |azimuth: f32| {
        CastBump::gaussian(azimuth, t.cheek_az_width, t.cheek_y, t.cheek_y_width, t.cheek_amount)
    };
    let bumps = [
        cheek(FRAC_PI_2 - t.cheek_azimuth),
        cheek(FRAC_PI_2 + t.cheek_azimuth),
        // The front gun embrasure. NOT a cheek: this one is a pocket cut for the gun to come
        // through, so it takes the blueprint's own wall sharpness rather than the cast swell's
        // Gaussian. `2.0` was hard-coded here, which meant the aperture could only ever be a
        // dimple no matter what the blueprint asked for.
        CastBump::plateau(
            FRAC_PI_2,
            t.embrasure_az_width,
            t.embrasure_y,
            t.embrasure_y_width,
            t.embrasure_amount,
            t.embrasure_falloff,
        ),
    ];

    try_build_cast_loft(&CastLoftSpec {
        sections: &sections,
        bumps: &bumps,
        segments: t.segments,
        // Flat lids in the ring-seat and roof station planes: a watertight casting beneath the
        // separate cupola, with no artificial roof spike that reads as a pinched casting.
        caps: CastCaps { bottom: CastCap::Planar, top: CastCap::Planar },
        material: MaterialRole::CastArmor,
        smoothing: SmoothingGroup(2),
    })
    // The T-54 turret blueprint is static, validated authoring data, locked by the turret tests;
    // an error here means the blueprint regressed, not bad runtime input.
    .expect("the T-54 turret blueprint is a valid cast loft")
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{VehicleBlueprint, VehicleKind};

    fn turret_loft_visual() -> TurretLoftVisual {
        VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap().hybrid().unwrap().turret_loft
    }

    fn max_half_width_in_band(mesh: &GeometryMesh, lo: f32, hi: f32) -> f32 {
        mesh.vertices()
            .iter()
            .filter(|v| v.position.y >= lo && v.position.y <= hi)
            .map(|v| v.position.x.abs())
            .fold(0.0_f32, f32::max)
    }

    /// After the planar-cap migration the production turret is a watertight, consistently-wound
    /// casting with no boundary or non-manifold edges — the closed-shell contract beneath the
    /// separate cupola.
    #[test]
    fn the_planar_capped_turret_is_a_closed_smooth_manifold() {
        t54_turret_loft(&turret_loft_visual())
            .validate_quality(vehicle_geometry::CLOSED_SMOOTH_MESH)
            .expect("the planar-capped T-54 turret is a closed smooth manifold");
    }

    /// The lofted casting must bulge LOW (the ring overhang) and neck IN toward the flat roof — the
    /// flattened T-54 pancake, not a tall round pot. This is the silhouette lock that the metaball
    /// turret carried (`cast_dome_bulges_low_and_flattens_toward_the_roof`), now on the loft mesh.
    #[test]
    fn lofted_cast_dome_bulges_low_and_necks_to_a_flat_roof() {
        let t = turret_loft_visual();
        let mesh = t54_turret_loft(&t);
        let ring_y = t.stations.first().unwrap().y;
        let roof_y = t.stations.last().unwrap().y;
        let band = roof_y - ring_y;
        let w_lo = max_half_width_in_band(&mesh, ring_y, ring_y + 0.25 * band);
        let w_hi = max_half_width_in_band(&mesh, roof_y - 0.20 * band, roof_y);
        assert!(
            w_lo > w_hi + 0.10,
            "casting must bulge low and neck to the roof: w_lo {w_lo:.2} vs w_hi {w_hi:.2}"
        );
    }

    /// The front cheeks must read as the T-54's signature cast front mass: they add real width to
    /// the front shoulder over a cheekless shell, and they stop the casting from necking IN at the
    /// front (the old failure mode, where the front quarter was narrower than the sides).
    #[test]
    fn the_front_cheeks_carry_real_cast_mass_into_the_front_shoulder() {
        let v = turret_loft_visual();
        // Sample the cast shoulder in the band around the cheek centre (the gun-axis height).
        let band = (v.cheek_y - 0.06)..=(v.cheek_y + 0.06);
        let front_width = |mesh: &GeometryMesh| {
            mesh.vertices()
                .iter()
                .filter(|p| band.contains(&p.position.y))
                .filter(|p| (0.35..=0.90).contains(&p.position.z))
                .map(|p| p.position.x.abs())
                .fold(0.0_f32, f32::max)
        };
        let with_cheeks = t54_turret_loft(&v);
        let cheekless = t54_turret_loft(&TurretLoftVisual { cheek_amount: 0.0, ..v });

        let front = front_width(&with_cheeks);
        assert!(
            front > front_width(&cheekless) + 0.04,
            "the cheeks must add front-shoulder mass: {front:.3} vs cheekless {:.3}",
            front_width(&cheekless)
        );

        let side = with_cheeks
            .vertices()
            .iter()
            .filter(|p| band.contains(&p.position.y) && p.position.z.abs() <= 0.20)
            .map(|p| p.position.x.abs())
            .fold(0.0_f32, f32::max);
        assert!(
            front >= side - 0.01,
            "the front must not neck in: front {front:.3} vs side {side:.3}"
        );
    }

    /// The lofted turret — cheeks and all — stays inside the gameplay turret plan from the
    /// blueprint's `TurretShape`, so swapping it for the metaball turret cannot poke out of the
    /// hitbox volume.
    #[test]
    fn the_lofted_turret_fits_its_gameplay_plan() {
        let plan = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap().turret;
        let (px, pz) = (plan.plan_half_width + 0.05, plan.plan_half_length + 0.05);
        let b = t54_turret_loft(&turret_loft_visual()).bounds().expect("non-empty");
        assert!(
            b.min.x >= -px && b.max.x <= px,
            "within the ±{:.3} plan in X: {} {}",
            plan.plan_half_width,
            b.min.x,
            b.max.x
        );
        assert!(
            b.min.z >= -pz && b.max.z <= pz,
            "within the ±{:.3} plan in Z: {} {}",
            plan.plan_half_length,
            b.min.z,
            b.max.z
        );
    }
}
