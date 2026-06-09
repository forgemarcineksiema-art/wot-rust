use game_core::{ArmorFacing, ShellSpec, TankSpec, known_tank_specs, resolve_penetration};

#[test]
fn panther_ii_spec_models_up_armored_panther_prototype() {
    let tank = TankSpec::panther_ii();

    assert_eq!(tank.name, "Panzerkampfwagen V Panther II");
    assert!((tank.mass_kg - 53_000.0).abs() < 1_000.0);
    assert!((tank.engine_power_kw - 441.0).abs() < 15.0);
    assert!((tank.max_forward_speed_mps - 12.78).abs() < 0.3);
    assert!(tank.max_forward_speed_mps > TankSpec::tiger_ii_ausf_b().max_forward_speed_mps);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::HullFront), 100.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::HullSide), 60.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::HullRear), 40.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::TurretFront), 120.0);
    assert_eq!(tank.gun.name, "7.5 cm KwK 42 L/70");
    assert_eq!(tank.gun.shell.caliber_mm, 75.0);
    assert!((tank.gun.shell.muzzle_velocity_mps - 935.0).abs() < 20.0);
    assert_eq!(tank.gun.shell.penetration_mm_at_100m, 138.0);
}

#[test]
fn panther_ii_is_available_in_known_specs() {
    let names: Vec<_> = known_tank_specs().into_iter().map(|spec| spec.name).collect();

    assert!(names.contains(&"Panzerkampfwagen V Panther II".to_string()));
}

#[test]
fn panther_ii_kwk42_penetrates_tiger_i_front_when_flat() {
    let panther_ii = TankSpec::panther_ii();
    let target = TankSpec::tiger_i_ausf_e();
    let shell = ShellSpec::armor_piercing(
        panther_ii.gun.shell.caliber_mm,
        panther_ii.gun.shell.muzzle_velocity_mps,
        panther_ii.gun.shell.penetration_mm_at_100m,
        panther_ii.gun.shell.damage_hp,
    );

    let result = resolve_penetration(&shell, &target.hull, ArmorFacing::HullFront, 0.0);

    assert!(result.penetrated);
}
