use game_core::{
    ArmorFacet, ArmorFacing, ArmorProfile, ArmorZone, ShellSpec, ShellType, resolve_penetration,
    resolve_penetration_at_distance, resolve_penetration_at_distance_on_zone,
    resolve_penetration_through_track,
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
    // Slope lives in the plate NORMAL (see `plate_normal`), never in an angle sum: the impact
    // angle passed here is already the true angle of incidence. A horizontal shot meets this
    // 60° glacis at 60° — and only then does the plate present more than its nominal.
    assert!(
        armor.effective_thickness_mm(ArmorFacing::HullFront, front.slope_degrees)
            > armor.nominal_thickness_mm(ArmorFacing::HullFront)
    );
    assert!(
        armor.effective_thickness_mm(ArmorFacing::HullFront, 0.0)
            < armor.nominal_thickness_mm(ArmorFacing::HullFront),
        "square-on the plate presents its nominal (times the weakspot multiplier), nothing more"
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
fn the_track_screen_shields_the_side_plate_behind_it() {
    // 80 mm side; the track band derives to 28 mm of spaced screen in front of it.
    let armor = ArmorProfile::new(100.0, 80.0, 45.0, 180.0, 90.0, 65.0);

    // 100 mm of penetration beats the bare side plate but dies in the track + side stack.
    let shell = ShellSpec::armor_piercing(85.0, 900.0, 100.0, 300);
    let direct = resolve_penetration_at_distance(&shell, &armor, ArmorFacing::HullSide, 0.0, 100.0);
    let screened =
        resolve_penetration_through_track(&shell, &armor, ArmorZone::LeftTrack, 0.0, 0.0, 100.0);
    assert!(direct.penetrated, "the bare side plate falls to 100 mm of penetration");
    assert!(!screened.penetrated, "the track screen absorbs what the side alone could not");
    assert_eq!(screened.damage_hp, 0, "a screened AP shell does no hull damage");
    assert!(
        screened.effective_armor_mm > direct.effective_armor_mm,
        "the reported armor is the whole stack: {} vs {}",
        screened.effective_armor_mm,
        direct.effective_armor_mm
    );

    // Enough penetration for the whole stack goes through and does full damage.
    let heavy = ShellSpec::armor_piercing(122.0, 800.0, 150.0, 420);
    let through =
        resolve_penetration_through_track(&heavy, &armor, ArmorZone::LeftTrack, 0.0, 0.0, 100.0);
    assert!(through.penetrated);
    assert_eq!(through.damage_hp, 420);
}

#[test]
fn the_screen_detonates_heat_early_and_fuzes_he_on_the_track() {
    let armor = ArmorProfile::new(100.0, 80.0, 45.0, 180.0, 90.0, 65.0);

    // 130 mm HEAT beats 28 + 80 on paper, but the screen costs a shaped charge double.
    let heat = ShellSpec::heat(100.0, 760.0, 130.0, 300);
    let screened =
        resolve_penetration_through_track(&heat, &armor, ArmorZone::LeftTrack, 0.0, 0.0, 100.0);
    assert!(!screened.penetrated, "the spaced track kills the jet's standoff");

    let hot = ShellSpec::heat(100.0, 760.0, 145.0, 300);
    let through =
        resolve_penetration_through_track(&hot, &armor, ArmorZone::LeftTrack, 0.0, 0.0, 100.0);
    assert!(through.penetrated, "a hot enough charge still burns through the stack");

    // HE fuzes on the first surface: it bursts on the track for chip damage, never interior.
    let he = ShellSpec::high_explosive(122.0, 515.0, 300.0, 410, 4.0);
    let burst =
        resolve_penetration_through_track(&he, &armor, ArmorZone::LeftTrack, 0.0, 0.0, 100.0);
    assert!(!burst.penetrated, "HE cannot carry through the screen no matter the numbers");
    assert!(burst.damage_hp > 0, "the burst still chips the running gear");
    assert!(burst.damage_hp < 410 / 2, "chip damage, not a penetration's worth");
}

/// S23: the glance band is a NUMBER the sight can read. A kinetic round between 60° and the
/// 70° bounce loses up to 30 % of its bite linearly — 15 % at 65° — and the effective
/// penetration the reticle prints (`effective_armor_mm + remaining_penetration_mm`, the sum
/// the client's hint reads) is the post-bite figure, not the nominal one.
#[test]
fn the_glance_band_costs_fifteen_percent_at_sixty_five_degrees_and_the_hint_reads_it() {
    let br412 = game_core::RoundId::Br412.spec();
    let armor = ArmorProfile::new(100.0, 80.0, 45.0, 200.0, 160.0, 65.0);
    let nominal = br412.penetration_mm_at_distance(300.0);
    let at = |angle: f32| {
        resolve_penetration_at_distance(&br412, &armor, ArmorFacing::HullFront, angle, 300.0)
    };
    let square = at(30.0);
    assert_eq!(square.glance_loss, 0.0);
    assert!(((square.effective_armor_mm + square.remaining_penetration_mm) - nominal).abs() < 0.01);
    // The band runs from 60° to the ROUND's own bounce: the blunt BR-412 digs in until 73°, so
    // at 65° it has lost 0.3 · 5/13 = 11.5 %; a sharp-nosed AP that bounces at 70° has lost 15 %.
    let glancing = at(65.0);
    let bounce = br412.ricochet_angle_deg().expect("a kinetic round bounces");
    let expected_loss = 0.3 * (65.0 - 60.0) / (bounce - 60.0);
    assert!(
        (glancing.glance_loss - expected_loss).abs() < 1.0e-4,
        "{} lost at 65° against a {bounce}° bounce",
        glancing.glance_loss
    );
    let effective_pen = glancing.effective_armor_mm + glancing.remaining_penetration_mm;
    assert!(
        (effective_pen - nominal * (1.0 - expected_loss)).abs() < 0.01,
        "{effective_pen} vs {nominal} less the glance"
    );
    assert!(effective_pen < nominal, "the sight never prints the nominal figure on a glance");

    let sharp = ShellSpec::armor_piercing(100.0, 895.0, 185.0, 320);
    assert_eq!(sharp.ricochet_angle_deg(), Some(70.0), "a plain AP bounces at 70°");
    let sharp_glance =
        resolve_penetration_at_distance(&sharp, &armor, ArmorFacing::HullFront, 65.0, 300.0);
    assert!((sharp_glance.glance_loss - 0.15).abs() < 1.0e-4, "15 % at 65° for a 70° bounce");
}

/// S15 (GDD §3.1): a surface burst hurts by the steel under it. The D-10's OF-412 (430 HP)
/// on a T-34-85's 45 mm side, a Tiger II's 150 mm glacis and a 166 mm plate, square on.
#[test]
fn he_surface_damage_is_a_function_of_the_steel_at_the_point() {
    let of412 = game_core::RoundId::Of412.spec();
    let square = |front_mm: f32| {
        let armor = ArmorProfile::new(front_mm, 45.0, 45.0, 90.0, 75.0, 52.0);
        resolve_penetration_at_distance(&of412, &armor, ArmorFacing::HullFront, 0.0, 300.0)
    };
    let side = square(45.0);
    let glacis = square(150.0);
    let thick = square(166.0);
    assert!(!side.penetrated && !glacis.penetrated && !thick.penetrated);
    assert_eq!(side.damage_hp, 157, "0.5 * 430 - 1.3 * 45");
    assert_eq!(glacis.damage_hp, 20, "0.5 * 430 - 1.3 * 150");
    assert!(side.damage_hp > glacis.damage_hp, "a thin side hurts more than a thick glacis");
    assert_eq!(thick.damage_hp, 0, "from 166 mm a surface burst does nothing");
    assert_eq!(game_core::he_surface_damage_hp(430, 165.0), 1, "...and 165 mm is the last HP");
    // Obliquity counts as steel: the same 45 mm plate met at 60 degrees is 90 mm through.
    let armor = ArmorProfile::new(45.0, 45.0, 45.0, 90.0, 75.0, 52.0);
    let oblique =
        resolve_penetration_at_distance(&of412, &armor, ArmorFacing::HullFront, 60.0, 300.0);
    assert_eq!(oblique.damage_hp, 98, "0.5 * 430 - 1.3 * 90");
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
