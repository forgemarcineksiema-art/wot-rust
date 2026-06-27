use game_core::{
    ArmorFacet, ArmorFacing, ArmorProfile, ArmorZone, ShellSpec, ShellType, resolve_penetration,
    resolve_penetration_at_distance, resolve_penetration_at_distance_on_zone,
};

#[test]
fn sloped_front_armor_increases_effective_thickness() {
    let armor = ArmorProfile::new(120.0, 80.0, 60.0, 200.0, 100.0, 80.0);

    let effective = armor.effective_thickness_mm(ArmorFacing::HullFront, 60.0);

    assert!((effective - 240.0).abs() < 0.5);
}

#[test]
fn shell_penetrates_when_penetration_exceeds_effective_armor() {
    let armor = ArmorProfile::new(120.0, 80.0, 60.0, 200.0, 100.0, 80.0);
    let shell = ShellSpec::armor_piercing(120.0, 900.0, 250.0, 390);

    let result = resolve_penetration(&shell, &armor, ArmorFacing::HullFront, 0.0);

    assert!(result.penetrated);
    assert_eq!(result.damage_hp, 390);
}

#[test]
fn each_facing_selects_its_own_plate() {
    let armor = ArmorProfile::new(120.0, 80.0, 60.0, 200.0, 100.0, 70.0);

    assert_eq!(armor.nominal_thickness_mm(ArmorFacing::HullFront), 120.0);
    assert_eq!(armor.nominal_thickness_mm(ArmorFacing::HullSide), 80.0);
    assert_eq!(armor.nominal_thickness_mm(ArmorFacing::HullRear), 60.0);
    assert_eq!(armor.nominal_thickness_mm(ArmorFacing::TurretFront), 200.0);
    assert_eq!(armor.nominal_thickness_mm(ArmorFacing::TurretSide), 100.0);
    assert_eq!(armor.nominal_thickness_mm(ArmorFacing::TurretRear), 70.0);
}

#[test]
fn armor_facets_can_describe_visual_slope_and_weakspots() {
    let armor = ArmorProfile::new_with_facets(
        ArmorFacet::new(100.0, 60.0, 0.65),
        ArmorFacet::new(80.0, 0.0, 1.0),
        ArmorFacet::new(45.0, 10.0, 1.0),
        ArmorFacet::new(200.0, 35.0, 0.75),
        ArmorFacet::new(90.0, 20.0, 1.0),
        ArmorFacet::new(65.0, 0.0, 1.0),
    );

    let front = armor.facet(ArmorFacing::HullFront);
    let turret = armor.facet(ArmorFacing::TurretFront);

    assert_eq!(front.slope_degrees, 60.0);
    assert_eq!(front.weakspot_multiplier, 0.65);
    assert_eq!(turret.slope_degrees, 35.0);
    assert!(
        armor.effective_thickness_mm(ArmorFacing::HullFront, 0.0)
            > armor.nominal_thickness_mm(ArmorFacing::HullFront)
    );
}

#[test]
fn armor_profile_exposes_named_plate_zones_for_mantlet_lower_roof_and_tracks() {
    let armor = ArmorProfile::new_with_facets(
        ArmorFacet::new(100.0, 60.0, 1.0),
        ArmorFacet::new(80.0, 0.0, 1.0),
        ArmorFacet::new(45.0, 0.0, 1.0),
        ArmorFacet::new(200.0, 25.0, 1.0),
        ArmorFacet::new(90.0, 10.0, 1.0),
        ArmorFacet::new(65.0, 0.0, 1.0),
    );

    assert_eq!(ArmorZone::UpperGlacis.facing(), ArmorFacing::HullFront);
    assert_eq!(ArmorZone::Mantlet.facing(), ArmorFacing::TurretFront);
    assert_eq!(ArmorZone::LeftTrack.facing(), ArmorFacing::HullSide);
    assert_eq!(ArmorZone::RightTrack.facing(), ArmorFacing::HullSide);
    assert!(
        armor.plate(ArmorZone::LowerPlate).nominal_thickness_mm
            < armor.plate(ArmorZone::UpperGlacis).nominal_thickness_mm
    );
    assert!(
        armor.plate(ArmorZone::Mantlet).nominal_thickness_mm
            > armor.plate(ArmorZone::TurretFront).nominal_thickness_mm
    );
    assert!(
        armor.plate(ArmorZone::Roof).nominal_thickness_mm
            < armor.plate(ArmorZone::HullSide).nominal_thickness_mm
    );
    assert_eq!(
        armor.plate(ArmorZone::LeftTrack).nominal_thickness_mm,
        armor.plate(ArmorZone::RightTrack).nominal_thickness_mm
    );
}

#[test]
fn penetration_can_resolve_against_named_armor_zone_instead_of_whole_facing() {
    let armor = ArmorProfile::new(100.0, 80.0, 45.0, 180.0, 90.0, 65.0);
    let shell = ShellSpec::armor_piercing(75.0, 900.0, 40.0, 240);

    let side = resolve_penetration_at_distance(&shell, &armor, ArmorFacing::HullSide, 0.0, 100.0);
    let track =
        resolve_penetration_at_distance_on_zone(&shell, &armor, ArmorZone::LeftTrack, 0.0, 100.0);

    assert!(!side.penetrated, "40 mm should not penetrate the whole 80 mm side facing");
    assert!(track.penetrated, "40 mm should penetrate the thinner track armor zone");
}

#[test]
fn apcr_loses_more_penetration_over_distance_than_ap() {
    let ap = ShellSpec::armor_piercing(100.0, 895.0, 200.0, 320);
    let apcr = ShellSpec::apcr(100.0, 1_150.0, 230.0, 300);

    assert_eq!(ap.shell_type, ShellType::ArmorPiercing);
    assert_eq!(apcr.shell_type, ShellType::Apcr);
    assert!(
        apcr.penetration_mm_at_distance(900.0) < ap.penetration_mm_at_distance(900.0) * 1.05,
        "APCR should buy close-range penetration at the cost of harsher long-range falloff"
    );
}

#[test]
fn heat_keeps_penetration_over_range_but_ricochets_at_extreme_angle() {
    let armor = ArmorProfile::new(100.0, 80.0, 60.0, 200.0, 90.0, 65.0);
    let heat = ShellSpec::heat(100.0, 760.0, 260.0, 300);

    assert_eq!(heat.penetration_mm_at_distance(100.0), heat.penetration_mm_at_distance(900.0));

    let result = resolve_penetration(&heat, &armor, ArmorFacing::HullSide, 86.0);

    assert!(!result.penetrated);
    assert!(result.ricocheted);
    assert_eq!(result.damage_hp, 0);
}

#[test]
fn large_ap_shell_overmatches_thin_plate_instead_of_ricocheting() {
    let armor = ArmorProfile::new(30.0, 25.0, 20.0, 45.0, 30.0, 20.0);
    let shell = ShellSpec::armor_piercing(100.0, 900.0, 140.0, 320);

    let result = resolve_penetration(&shell, &armor, ArmorFacing::HullSide, 78.0);

    assert!(!result.ricocheted);
}

#[test]
fn he_non_penetration_still_reports_surface_damage() {
    let armor = ArmorProfile::new(150.0, 80.0, 60.0, 200.0, 90.0, 65.0);
    let he = ShellSpec::high_explosive(122.0, 515.0, 38.0, 410, 4.0);

    let result = resolve_penetration_at_distance(&he, &armor, ArmorFacing::HullFront, 0.0, 500.0);

    assert!(!result.penetrated);
    assert!(!result.ricocheted);
    assert!(result.damage_hp > 0, "HE should chip external armor even without penetration");
    assert!(result.module_damage_hp > 0, "HE should be able to crit exposed modules");
}
