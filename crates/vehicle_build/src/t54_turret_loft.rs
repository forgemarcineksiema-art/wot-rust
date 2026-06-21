//! The T-54 cast turret as a **cast loft** shell — the controlled-surface replacement for the
//! metaball `sdf_mesh::t54_turret`. Every dimension is read from the blueprint's
//! [`TurretLoftVisual`] (the single source) and skinned by the [`cast_loft`] kernel; the cupola and
//! moving mantlet stay separate bedded parts, as before.

use std::f32::consts::FRAC_PI_2;

use cast_loft::{CastBump, CastCaps, CastLoftSpec, CastSection, build_cast_loft};
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

    let cheek = |azimuth: f32| CastBump {
        azimuth,
        az_width: t.cheek_az_width,
        y: t.cheek_y,
        y_width: t.cheek_y_width,
        amount: t.cheek_amount,
    };
    let bumps = [
        cheek(FRAC_PI_2 - t.cheek_azimuth),
        cheek(FRAC_PI_2 + t.cheek_azimuth),
        // The front gun embrasure: an inward recess the moving mantlet beds into.
        CastBump {
            azimuth: FRAC_PI_2,
            az_width: t.embrasure_az_width,
            y: t.embrasure_y,
            y_width: t.embrasure_y_width,
            amount: t.embrasure_amount,
        },
    ];

    build_cast_loft(&CastLoftSpec {
        sections: &sections,
        bumps: &bumps,
        segments: t.segments,
        caps: CastCaps { bottom: Some(t.floor_apex), top: Some(t.roof_apex) },
        material: MaterialRole::CastArmor,
        smoothing: SmoothingGroup(2),
    })
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

    /// The lofted turret — cheeks and all — stays inside the gameplay turret plan (±1.0 wide, ±1.04
    /// long), so swapping it for the metaball turret cannot poke out of the hitbox volume.
    #[test]
    fn the_lofted_turret_fits_its_gameplay_plan() {
        let b = t54_turret_loft(&turret_loft_visual()).bounds().expect("non-empty");
        assert!(
            b.min.x >= -1.05 && b.max.x <= 1.05,
            "within ±1.0 plan in X: {} {}",
            b.min.x,
            b.max.x
        );
        assert!(
            b.min.z >= -1.09 && b.max.z <= 1.09,
            "within ±1.04 plan in Z: {} {}",
            b.min.z,
            b.max.z
        );
    }
}
