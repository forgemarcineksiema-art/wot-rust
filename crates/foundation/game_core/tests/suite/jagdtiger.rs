use game_core::{
    ArmorFacing, ShellSpec, TankSpec, VehicleKind, known_tank_specs, resolve_penetration,
};

#[test]
fn jagdtiger_spec_models_fixed_casemate_tank_destroyer() {
    let tank = TankSpec::jagdtiger();

    assert_eq!(tank.name, "Panzerjager Tiger Ausf. B Jagdtiger");
    assert_eq!(tank.kind, VehicleKind::Jagdtiger);
    assert!((tank.mass_kg - 75_200.0).abs() < 1_000.0);
    assert!((tank.engine_power_kw - 441.0).abs() < 15.0);
    assert!((tank.max_forward_speed_mps - 9.61).abs() < 0.3);
    assert_eq!(tank.turret_rotation_rad_s, 0.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::HullFront), 150.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::HullSide), 80.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::HullRear), 80.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::TurretFront), 250.0);
    assert_eq!(tank.gun.name, "12.8 cm Pak 80 L/55");
    assert_eq!(tank.gun.shell.caliber_mm, 128.0);
    assert!((tank.gun.shell.muzzle_velocity_mps - 920.0).abs() < 25.0);
    assert_eq!(tank.gun.shell.penetration_mm_at_100m, 223.0);
}

#[test]
fn jagdtiger_is_available_in_known_specs() {
    let names: Vec<_> = known_tank_specs().into_iter().map(|spec| spec.name).collect();

    assert!(names.contains(&"Panzerjager Tiger Ausf. B Jagdtiger".to_string()));
}

#[test]
fn jagdtiger_pak80_penetrates_tiger_ii_turret_front_when_flat() {
    let jagdtiger = TankSpec::jagdtiger();
    let target = TankSpec::tiger_ii_ausf_b();
    let shell = ShellSpec::armor_piercing(
        jagdtiger.gun.shell.caliber_mm,
        jagdtiger.gun.shell.muzzle_velocity_mps,
        jagdtiger.gun.shell.penetration_mm_at_100m,
        jagdtiger.gun.shell.damage_hp,
    );

    // A flat (horizontal) shot meets the turret face at its slope — the true angle of incidence.
    let flat_shot = target.hull.facet(ArmorFacing::TurretFront).slope_degrees;
    let result = resolve_penetration(&shell, &target.hull, ArmorFacing::TurretFront, flat_shot);

    assert!(result.penetrated);
}
