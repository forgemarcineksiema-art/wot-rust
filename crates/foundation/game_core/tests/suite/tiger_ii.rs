use game_core::{ArmorFacing, ShellSpec, TankSpec, known_tank_specs, resolve_penetration};

#[test]
fn tiger_ii_spec_matches_core_historical_shape() {
    let tank = TankSpec::tiger_ii_ausf_b();

    assert_eq!(tank.name, "Panzerkampfwagen VI B Tiger II");
    assert!((tank.mass_kg - 69_800.0).abs() < 1_000.0);
    assert!((tank.engine_power_kw - 515.0).abs() < 15.0);
    assert!((tank.max_forward_speed_mps - 10.56).abs() < 0.3);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::HullFront), 150.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::HullSide), 80.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::HullRear), 80.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::TurretFront), 180.0);
    assert_eq!(tank.gun.name, "8.8 cm KwK 43 L/71");
    assert_eq!(tank.gun.shell.caliber_mm, 88.0);
    assert!((tank.gun.shell.muzzle_velocity_mps - 1_000.0).abs() < 50.0);
}

#[test]
fn tiger_ii_is_available_in_known_specs() {
    let names: Vec<_> = known_tank_specs().into_iter().map(|spec| spec.name).collect();

    assert!(names.contains(&"Panzerkampfwagen VI B Tiger II".to_string()));
}

#[test]
fn tiger_ii_kwk43_penetrates_tiger_i_front_when_flat() {
    let tiger_ii = TankSpec::tiger_ii_ausf_b();
    let target = TankSpec::tiger_i_ausf_e();
    let shell = ShellSpec::armor_piercing(
        tiger_ii.gun.shell.caliber_mm,
        tiger_ii.gun.shell.muzzle_velocity_mps,
        tiger_ii.gun.shell.penetration_mm_at_100m,
        tiger_ii.gun.shell.damage_hp,
    );

    // A flat (horizontal) shot meets the glacis at its slope — the true angle of incidence.
    let flat_shot = target.hull.facet(ArmorFacing::HullFront).slope_degrees;
    let result = resolve_penetration(&shell, &target.hull, ArmorFacing::HullFront, flat_shot);

    assert!(result.penetrated);
}
