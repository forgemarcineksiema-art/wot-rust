//! A cast turret is not a box, and the armour model now knows it.
//!
//! The T-54's documented wall is 200 mm at the face, 160 mm behind the cheeks and 65 mm at the
//! rear quarter. Quoting ONE side number for that casting — the 90 mm the module used to carry —
//! is wrong in both directions at once: it made the cheeks paper (they are nearly twice that)
//! and the rear over-armoured (it is a third less). No slope maths recovers the difference,
//! because the difference is metal, not geometry.
//!
//! The armour volume already sweeps the dome as per-azimuth sectors, so each sector carries its
//! share of the wall and a shell resolves against the steel actually at the spot it struck.

use game_core::{
    ArmorZone, ShellSpec, VehicleKind, resolve_penetration_at_distance_on_zone,
    resolve_penetration_at_distance_on_zone_scaled, vehicle_armor_volumes,
};

fn t54_armor() -> game_core::ArmorProfile {
    VehicleKind::T54_1951.spec().hull
}

/// A round that is decisive against the thin rear of the casting and hopeless against its cheek.
fn probe_shell() -> ShellSpec {
    ShellSpec::armor_piercing(100.0, 895.0, 120.0, 320)
}

#[test]
fn the_documented_wall_reaches_the_armour_profile() {
    let armor = t54_armor();
    assert_eq!(
        armor.turret_side_mm, 160.0,
        "the side wall's THICKEST point is the documented 160 mm, not an average"
    );
    assert_eq!(armor.turret_front_mm, 200.0);
    assert_eq!(armor.turret_rear_mm, 65.0);
    assert_eq!(
        armor.plate(ArmorZone::Roof).nominal_thickness_mm,
        30.0,
        "the turret roof is AUTHORED at its documented 30 mm — the fleet formula gave 24"
    );
    // And the hull deck is its own plate now, not the turret roof's twin.
    let deck = armor.plate(ArmorZone::HullDeck).nominal_thickness_mm;
    assert!(
        deck < armor.plate(ArmorZone::Roof).nominal_thickness_mm,
        "the engine deck ({deck} mm) is thinner than the turret roof — it always was"
    );
}

#[test]
fn the_cast_wall_thins_toward_the_rear_of_the_turret() {
    let volumes = vehicle_armor_volumes(VehicleKind::T54_1951).expect("baked volumes");
    let mut side_scales: Vec<(f32, f32)> = Vec::new();
    for plane in &volumes.turret.planes {
        if plane.zone == ArmorZone::TurretSide
            && let Some(scale) = plane.thickness_scale
        {
            // How far aft this sector faces: -z is the rear.
            side_scales.push((plane.normal.z, scale));
        }
    }
    assert!(
        side_scales.len() >= 6,
        "the swept casting must carry a per-sector wall, got {} tapered sectors",
        side_scales.len()
    );

    // Forward-facing side sectors keep the full wall; rearward ones have thinned toward 0.41.
    let front_most = side_scales.iter().max_by(|a, b| a.0.total_cmp(&b.0)).expect("front sector");
    let rear_most = side_scales.iter().min_by(|a, b| a.0.total_cmp(&b.0)).expect("rear sector");
    assert!(
        front_most.1 > rear_most.1 + 0.3,
        "the wall must visibly thin running aft: {:.2} at the cheeks vs {:.2} at the rear",
        front_most.1,
        rear_most.1
    );
    assert!(
        (0.9..=1.0).contains(&front_most.1),
        "behind the cheeks the wall is still the full 160 mm (scale {:.2})",
        front_most.1
    );
    assert!(
        (0.35..=0.55).contains(&rear_most.1),
        "at the rear quarter it has thinned to about the 65 mm rear (scale {:.2})",
        rear_most.1
    );
}

#[test]
fn where_a_shell_lands_on_the_flank_decides_whether_it_gets_in() {
    let armor = t54_armor();
    let shell = probe_shell();
    // Square-on, point blank: the only difference is WHERE on the flank the round struck.
    let cheek = resolve_penetration_at_distance_on_zone_scaled(
        &shell,
        &armor,
        ArmorZone::TurretSide,
        0.0,
        100.0,
        1.0,
    );
    let rear_quarter = resolve_penetration_at_distance_on_zone_scaled(
        &shell,
        &armor,
        ArmorZone::TurretSide,
        0.0,
        100.0,
        0.41,
    );
    assert!(
        !cheek.penetrated,
        "120 mm of penetration cannot open the 160 mm cheek wall (effective {:.0} mm)",
        cheek.effective_armor_mm
    );
    assert!(
        rear_quarter.penetrated,
        "the same round opens the ~65 mm rear quarter of the same casting (effective {:.0} mm)",
        rear_quarter.effective_armor_mm
    );
    assert!(
        rear_quarter.effective_armor_mm < cheek.effective_armor_mm * 0.6,
        "the rear quarter must be substantially thinner: {:.0} vs {:.0} mm",
        rear_quarter.effective_armor_mm,
        cheek.effective_armor_mm
    );
}

/// The un-tapered path must be untouched: a scale of 1.0 is exactly the old resolution, and a
/// vehicle with no documented taper resolves as it always did.
#[test]
fn a_plate_without_a_taper_resolves_exactly_as_before() {
    let armor = t54_armor();
    let shell = probe_shell();
    for zone in [ArmorZone::UpperGlacis, ArmorZone::HullSide, ArmorZone::TurretFront] {
        let plain = resolve_penetration_at_distance_on_zone(&shell, &armor, zone, 25.0, 250.0);
        let scaled =
            resolve_penetration_at_distance_on_zone_scaled(&shell, &armor, zone, 25.0, 250.0, 1.0);
        assert_eq!(plain, scaled, "{zone:?}: scale 1.0 must be the identity");
    }

    let mut castings_checked = 0;
    for kind in VehicleKind::PLAYABLE {
        let Some(volumes) = vehicle_armor_volumes(kind) else { continue };
        castings_checked += 1;
        let tapered =
            volumes.turret.planes.iter().filter(|plane| plane.thickness_scale.is_some()).count();
        if kind == VehicleKind::T54_1951 {
            assert!(tapered > 0, "the T-54's casting is documented as tapering");
        } else {
            assert_eq!(
                tapered, 0,
                "{kind:?} has no documented taper — it must keep a constant wall until its \
                 dossier says otherwise"
            );
        }
    }
    assert_eq!(
        castings_checked,
        VehicleKind::PLAYABLE.len(),
        "the taper claim covers every playable turret; a missing armour volume must fail here \
         rather than quietly excuse a casting from the question"
    );
}
