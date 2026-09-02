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
    let modules = VehicleKind::T54_1951.default_loadout();
    let spec = modules.assemble(VehicleKind::T54_1951);
    assert_eq!(spec.mass_kg, modules.total_mass_kg());
    assert!((spec.mass_kg - 36_000.0).abs() < 1.0);
}

#[test]
fn swapping_the_gun_changes_assembled_stats_and_weight() {
    let mut modules = VehicleKind::T54_1951.default_loadout();
    let stock = modules.assemble(VehicleKind::T54_1951);
    assert_eq!(stock.gun.name, "100 mm D-10T");

    // The D-10T2S is an offered, compatible upgrade (caliber 100 <= turret max 105).
    let alt = VehicleKind::T54_1951
        .gun_options()
        .into_iter()
        .find(|gun| gun.spec.name == "100 mm D-10T2S")
        .expect("D-10T2S is offered for the T-54");
    modules.try_install_gun(alt).expect("D-10T2S fits the T-54 turret");
    let swapped = modules.assemble(VehicleKind::T54_1951);

    assert_eq!(swapped.gun.name, "100 mm D-10T2S");
    assert_ne!(swapped.gun.reload_seconds, stock.gun.reload_seconds);
}

#[test]
fn oversized_gun_is_rejected_by_the_turret() {
    let mut modules = VehicleKind::T54_1951.default_loadout();
    let big_gun =
        VehicleKind::Jagdtiger.gun_options().into_iter().next().expect("Jagdtiger has a gun"); // 128 mm

    let result = modules.try_install_gun(big_gun);

    assert!(matches!(result, Err(ModuleError::GunTooLargeForTurret { .. })));
    assert_eq!(modules.gun.spec.name, "100 mm D-10T", "loadout unchanged after a rejected swap");
}

#[test]
fn jagdtiger_turret_is_a_fixed_casemate() {
    let modules = VehicleKind::Jagdtiger.default_loadout();
    assert!(modules.turret.traverse.is_fixed());
    assert_eq!(modules.assemble(VehicleKind::Jagdtiger).turret_rotation_rad_s, 0.0);
}

/// Inny Poziom A11: the fleet's turrets traverse at genre-class rates. They shipped at half of
/// them (16–25 deg/s, a 1.57× spread) and every rotating turret was out-turned by its own hull,
/// so steering stripped the world bearing faster than the gunner recovered it. Three floors, so
/// a re-tune cannot slide back: no rotating turret under 22 deg/s, every turret at least three
/// quarters of its hull's turn rate, and a fleet spread of at least 2× (traverse is a stat that
/// tells vehicles apart, not a constant with noise on it).
#[test]
fn every_rotating_turret_is_genre_fast_and_keeps_up_with_its_hull() {
    let mut slowest = f32::MAX;
    let mut fastest = 0.0_f32;
    let mut walked = 0usize;
    for kind in VehicleKind::ALL {
        let spec = kind.default_loadout().assemble(kind);
        if spec.has_fixed_casemate() {
            continue;
        }
        let turret = spec.turret_rotation_rad_s.to_degrees();
        let hull = spec.turn_rate_rad_s.to_degrees();
        walked += 1;
        assert!(turret >= 22.0, "{kind:?}: turret {turret:.1} deg/s is under the 22 deg/s floor");
        assert!(
            turret >= 0.75 * hull,
            "{kind:?}: turret {turret:.1} deg/s cannot keep up with a hull turning {hull:.1} deg/s"
        );
        slowest = slowest.min(turret);
        fastest = fastest.max(turret);
    }
    assert!(
        walked >= 7,
        "the fleet walk checked {walked} turrets of {} vehicles",
        VehicleKind::ALL.len()
    );
    assert!(fastest / slowest >= 2.0, "fleet spread {fastest:.1}/{slowest:.1} tells nobody apart");
}
