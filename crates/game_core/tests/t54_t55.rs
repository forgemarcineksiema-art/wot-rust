use game_core::{ArmorFacing, ShellSpec, TankSpec, known_tank_specs, resolve_penetration};

#[test]
fn t55a_spec_matches_core_historical_shape() {
    let tank = TankSpec::t55a();

    assert_eq!(tank.name, "T-55A");
    assert!((tank.mass_kg - 36_000.0).abs() < 500.0);
    assert!((tank.engine_power_kw - 433.0).abs() < 5.0);
    assert!((tank.max_forward_speed_mps - 15.28).abs() < 0.2);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::HullFront), 100.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::TurretFront), 200.0);
    assert_eq!(tank.gun.name, "100 mm D-10T2S");
    assert_eq!(tank.gun.shell.caliber_mm, 100.0);
}

#[test]
fn t54_1951_and_t55a_are_available_in_known_specs() {
    let names: Vec<_> = known_tank_specs().into_iter().map(|spec| spec.name).collect();

    assert!(names.contains(&"T-54-3 obr. 1951".to_string()));
    assert!(names.contains(&"T-55A".to_string()));
}

#[test]
fn t55a_d10t2s_can_penetrate_t54_front_when_flat() {
    let t55a = TankSpec::t55a();
    let target = TankSpec::t54_1951();
    let shell = ShellSpec::armor_piercing(
        t55a.gun.shell.caliber_mm,
        t55a.gun.shell.muzzle_velocity_mps,
        t55a.gun.shell.penetration_mm_at_100m,
        t55a.gun.shell.damage_hp,
    );

    let result = resolve_penetration(&shell, &target.hull, ArmorFacing::HullFront, 0.0);

    assert!(result.penetrated);
}
