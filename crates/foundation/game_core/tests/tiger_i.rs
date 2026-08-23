use game_core::{ArmorFacing, ShellSpec, TankSpec, known_tank_specs, resolve_penetration};

#[test]
fn tiger_i_front_armor_carries_slope_and_weakspot_facets() {
    let tank = TankSpec::tiger_i_ausf_e();

    // Regression: the Tiger I used to fall back to flat 0°/no-weakspot defaults instead of an
    // explicit facet profile like the rest of the roster.
    let hull_front = tank.hull.facet(ArmorFacing::HullFront);
    assert!(hull_front.slope_degrees > 0.0, "hull front should be sloped, not a flat default");
    // Retired 2026-08-03 (#428): the facet-wide smear is gone — a front's weakness is its
    // PATCHES (mantlet, cupola, bow ports), each aimable where the eye sees it. The plate
    // itself resolves at full strength, and this lock now guards AGAINST the smear returning.
    assert_eq!(hull_front.weakspot_multiplier, 1.0, "the front's weakness lives in its patches");

    let turret_front = tank.hull.facet(ArmorFacing::TurretFront);
    assert!(turret_front.slope_degrees > 0.0, "turret front should be sloped");
    assert_eq!(
        turret_front.weakspot_multiplier, 1.0,
        "the mantlet weakness is the mantlet PATCH, not a facet-wide discount"
    );

    // Slab sides and rear stay vertical, matching the Tiger I's reputation.
    assert_eq!(tank.hull.facet(ArmorFacing::HullSide).slope_degrees, 0.0);
    assert_eq!(tank.hull.facet(ArmorFacing::TurretSide).slope_degrees, 0.0);
}

#[test]
fn tiger_i_spec_matches_core_historical_shape() {
    let tank = TankSpec::tiger_i_ausf_e();

    assert_eq!(tank.name, "Panzerkampfwagen VI Tiger Ausf. E");
    assert!((tank.mass_kg - 57_000.0).abs() < 800.0);
    assert!((tank.engine_power_kw - 515.0).abs() < 15.0);
    assert!((tank.max_forward_speed_mps - 10.56).abs() < 0.2);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::HullFront), 100.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::HullSide), 80.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::TurretFront), 100.0);
    assert_eq!(tank.gun.name, "8.8 cm KwK 36 L/56");
    assert_eq!(tank.gun.shell.caliber_mm, 88.0);
}

#[test]
fn tiger_i_is_available_in_known_specs() {
    let names: Vec<_> = known_tank_specs().into_iter().map(|spec| spec.name).collect();

    assert!(names.contains(&"Panzerkampfwagen VI Tiger Ausf. E".to_string()));
}

/// The aim-dependent duel the weakspot work exists to create (#428). The KwK 36 could not beat
/// a T-54's honest 60-degree glacis — the retired smear was what let it — but the bow port's
/// flat 100 mm is exactly what an 8.8 cm round eats. When the older gun meets the T-54, it
/// wins by aiming, not by a discount.
#[test]
fn tiger_kwk36_beats_the_t54_port_but_not_its_honest_glacis() {
    let tiger = TankSpec::tiger_i_ausf_e();
    let target = TankSpec::t54_1951();
    let shell = ShellSpec::armor_piercing(
        tiger.gun.shell.caliber_mm,
        tiger.gun.shell.muzzle_velocity_mps,
        tiger.gun.shell.penetration_mm_at_100m,
        tiger.gun.shell.damage_hp,
    );

    // A flat (horizontal) shot meets the glacis at its slope — the true angle of incidence.
    let flat_shot = target.hull.facet(ArmorFacing::HullFront).slope_degrees;
    let glacis = resolve_penetration(&shell, &target.hull, ArmorFacing::HullFront, flat_shot);
    assert!(!glacis.penetrated, "an honest 60-degree glacis defeats the KwK 36");

    // The same shot into the course-MG port: the plate's own steel presenting FLAT.
    let port = game_core::resolve_penetration_at_distance_on_zone(
        &shell,
        &target.hull,
        game_core::ArmorZone::GlacisPort,
        0.0,
        100.0,
    );
    assert!(port.penetrated, "the bow port is what the older gun aims for");
}
