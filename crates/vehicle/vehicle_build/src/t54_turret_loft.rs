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

    // The FACE PLATE: one central plateau swell across the whole window band. On the real
    // casting (REF 22/23) the forward-most structure IS the face around the window — the
    // window's walls top out at the front, and the rebate is cut through that fullness. Two
    // outboard gaussian "cheeks" put the mass on the egg's curl instead, and the window's wall
    // ring sank below the bare nose; a pair centred on the window filled the rebate itself.
    // A plateau raises floor, shelf and walls TOGETHER, so the window keeps its depth.
    let bumps = [
        CastBump::plateau(
            FRAC_PI_2,
            t.cheek_az_width,
            t.cheek_y,
            t.cheek_y_width,
            t.cheek_amount,
            // Falloff 6: flat until ~0.8 of the width (the wall band included), gone by ~1.3 —
            // the plate reads as the casting's full face, not as a soft boss on the nose.
            6.0,
        ),
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
        // The outer WINDOW the canvas is fastened over: the wide, shallow rectangular seat a
        // T-54 carries between its cheeks. The embrasure above is cut through this seat's floor.
        CastBump::plateau(
            FRAC_PI_2,
            t.window_az_width,
            t.embrasure_y,
            t.window_y_width,
            t.window_amount,
            t.window_falloff,
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

    /// The dome tapers CONTINUOUSLY from its widest band — the pilot's photo verdict (REF 46,
    /// 2026-07-29), which overturned the S1 drawing's "vertical flank to 2.00": the drawing was
    /// the sheet with a ±4-7% internal disagreement, and the vertical band it dictated read as
    /// the player's "pancake". What is locked now: the documented 2.25 m lives at the widest
    /// band (1.66-1.76), every station above it is strictly narrower, and the taper is smooth
    /// (no plateau followed by a cliff). The face fullness is a central PLATEAU bump (the face
    /// plate the window is cut through), so cheek_amount is positive and CENTRED.
    #[test]
    fn the_dome_tapers_continuously_from_its_widest_band() {
        let v = turret_loft_visual();
        assert!(
            v.cheek_amount > 0.0 && v.cheek_azimuth == 0.0,
            "the face plate is one centred plateau, not offset lobes"
        );
        let mesh = t54_turret_loft(&v);
        let width_at = |y: f32| {
            let (lo, hi) = mesh
                .vertices()
                .iter()
                .filter(|p| (p.position.y - y).abs() < 1.0e-4)
                .map(|p| p.position.x)
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), x| (lo.min(x), hi.max(x)));
            hi - lo
        };
        let widest = width_at(1.66).max(width_at(1.76));
        assert!(
            (widest - 2.25).abs() <= 0.012,
            "the documented 2.25 m lives at the widest band, got {widest:.3}"
        );
        let ladder =
            [width_at(1.76), width_at(1.88), width_at(2.00), width_at(2.12), width_at(2.22)];
        for pair in ladder.windows(2) {
            assert!(
                pair[1] < pair[0] - 0.005,
                "the taper is continuous above the widest band: {:.3} then {:.3}",
                pair[0],
                pair[1]
            );
        }
        let steps: Vec<f32> = ladder.windows(2).map(|p| p[0] - p[1]).collect();
        for pair in steps.windows(2) {
            assert!(
                pair[1] < pair[0] * 3.0 + 0.06,
                "no plateau-then-cliff: taper steps {:.3} then {:.3}",
                pair[0],
                pair[1]
            );
        }
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
