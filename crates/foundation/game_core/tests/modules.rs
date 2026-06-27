use game_core::{ModuleError, VehicleKind};

#[test]
fn every_stock_gun_declares_aim_time_and_bloom() {
    for spec in game_core::known_tank_specs() {
        let gun = &spec.gun;
        assert!(gun.aim_time_seconds > 0.0, "{} needs aim time", gun.name);
        assert!(gun.shot_bloom_mrad > 0.0, "{} needs shot bloom", gun.name);
        assert!(gun.movement_bloom_mrad > 0.0, "{} needs movement bloom", gun.name);
        assert!(
            gun.max_dispersion_mrad > gun.dispersion_mrad,
            "{} needs room above base dispersion",
            gun.name
        );
    }
}

#[test]
fn every_stock_loadout_is_self_consistent() {
    for kind in VehicleKind::ALL {
        let modules = kind.default_loadout();
        let spec = modules.assemble(kind);
        assert_eq!(spec.name, kind.display_name(), "{kind:?} name");
        assert_eq!(spec.kind, kind, "{kind:?} kind");
        assert!(modules.can_mount_gun(&modules.gun), "{kind:?} stock gun must fit its turret");
        assert!(modules.total_mass_kg() > 0.0, "{kind:?} mass");
    }
}

#[test]
fn assembled_mass_is_the_sum_of_modules() {
    let modules = VehicleKind::T55A.default_loadout();
    let spec = modules.assemble(VehicleKind::T55A);
    assert_eq!(spec.mass_kg, modules.total_mass_kg());
    assert!((spec.mass_kg - 36_000.0).abs() < 1.0);
}

#[test]
fn swapping_the_gun_changes_assembled_stats_and_weight() {
    let mut modules = VehicleKind::T55A.default_loadout();
    let stock = modules.assemble(VehicleKind::T55A);
    assert_eq!(stock.gun.name, "100 mm D-10T2S");

    // The D-10T is an offered, compatible alternative (caliber 100 <= turret max 105).
    let alt = VehicleKind::T55A
        .gun_options()
        .into_iter()
        .find(|gun| gun.spec.name == "100 mm D-10T")
        .expect("D-10T is offered for the T-55A");
    modules.try_install_gun(alt).expect("D-10T fits the T-55 turret");
    let swapped = modules.assemble(VehicleKind::T55A);

    assert_eq!(swapped.gun.name, "100 mm D-10T");
    assert_ne!(swapped.gun.reload_seconds, stock.gun.reload_seconds);
}

#[test]
fn oversized_gun_is_rejected_by_the_turret() {
    let mut modules = VehicleKind::T55A.default_loadout();
    let big_gun =
        VehicleKind::Jagdtiger.gun_options().into_iter().next().expect("Jagdtiger has a gun"); // 128 mm

    let result = modules.try_install_gun(big_gun);

    assert!(matches!(result, Err(ModuleError::GunTooLargeForTurret { .. })));
    assert_eq!(modules.gun.spec.name, "100 mm D-10T2S", "loadout unchanged after a rejected swap");
}

#[test]
fn jagdtiger_turret_is_a_fixed_casemate() {
    let modules = VehicleKind::Jagdtiger.default_loadout();
    assert!(modules.turret.traverse.is_fixed());
    assert_eq!(modules.assemble(VehicleKind::Jagdtiger).turret_rotation_rad_s, 0.0);
}
