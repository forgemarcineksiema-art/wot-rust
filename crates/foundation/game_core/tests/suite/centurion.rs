//! The Centurion Mk 3's spec contract: the British line's first vehicle carries its historical
//! shape into gameplay — the gunnery trade (best accuracy and optics for modest alpha and
//! pace), the skirted flank, and the honest 65-round rack.

use game_core::{
    ArmorFacing, Nation, ShellSpec, TankSpec, VehicleClass, VehicleKind, known_tank_specs,
    resolve_penetration,
};

#[test]
fn centurion_spec_matches_core_historical_shape() {
    let tank = TankSpec::centurion_mk3();

    assert_eq!(tank.name, "Centurion Mk 3");
    assert_eq!(tank.kind, VehicleKind::Centurion);
    assert!((tank.mass_kg - 49_000.0).abs() < 2_000.0, "a ~49 t medium-heavy: {}", tank.mass_kg);
    assert!((tank.engine_power_kw - 480.0).abs() < 15.0, "the Meteor");
    assert!((tank.max_forward_speed_mps - 9.6).abs() < 0.2, "34.6 km/h, not a sprinter");
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::HullFront), 76.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::TurretFront), 152.0);
    assert_eq!(tank.gun.name, "84 mm 20-pounder Type A");
    assert_eq!(tank.gun.shell.caliber_mm, 84.0);
    assert_eq!(tank.ammo_capacity, 65, "the deep 20-pounder stowage");
}

#[test]
fn centurion_is_the_british_tier_eight_medium() {
    assert_eq!(VehicleKind::Centurion.nation(), Nation::Britain);
    assert_eq!(VehicleKind::Centurion.tier(), 8);
    assert_eq!(VehicleKind::Centurion.class(), VehicleClass::Medium);
    assert!(VehicleKind::PLAYABLE.contains(&VehicleKind::Centurion));
    let names: Vec<_> = known_tank_specs().into_iter().map(|spec| spec.name).collect();
    assert!(names.contains(&"Centurion Mk 3".to_string()));
}

/// The gunnery trade in numbers: the 20-pounder shoots tighter than the D-10 family it meets
/// at tier VIII/IX, for less per-shot alpha — a sidegrade role, not a straight upgrade.
#[test]
fn the_20pdr_out_shoots_the_d10_for_less_alpha() {
    let cent = TankSpec::centurion_mk3();
    let t54 = TankSpec::t54_1951();
    assert!(cent.gun.dispersion_mrad < t54.gun.dispersion_mrad, "tighter than the D-10");
    assert!(cent.gun.aim_time_seconds < t54.gun.aim_time_seconds, "settles faster");
    assert!(cent.gun.shell.damage_hp < t54.gun.shell.damage_hp, "...for less alpha");
    // And the second slot is APDS: the fastest shell in the game, penetration bleeding hard
    // with range like the sub-caliber round it is.
    let apds = cent.gun.ammo_options()[1];
    assert!(apds.muzzle_velocity_mps > 1_400.0, "APDS flies flat");
    assert!(
        apds.penetration_mm_at_distance(1_000.0) < apds.penetration_mm_at_100m * 0.85,
        "sub-caliber penetration falls off with range"
    );
}

#[test]
fn centurion_20pdr_penetrates_t54_front_when_flat() {
    let cent = TankSpec::centurion_mk3();
    let target = TankSpec::t54_1951();
    let shell = ShellSpec::armor_piercing(
        cent.gun.shell.caliber_mm,
        cent.gun.shell.muzzle_velocity_mps,
        cent.gun.shell.penetration_mm_at_100m,
        cent.gun.shell.damage_hp,
    );

    // A flat (horizontal) shot meets the glacis at its slope — the true angle of incidence.
    let flat_shot = target.hull.facet(ArmorFacing::HullFront).slope_degrees;
    let result = resolve_penetration(&shell, &target.hull, ArmorFacing::HullFront, flat_shot);

    assert!(result.penetrated, "230 mm at 100 m beats the T-54 glacis when flat");
}
