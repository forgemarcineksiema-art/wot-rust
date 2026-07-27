//! The KV-1 mod. 1942's spec contract: Era II's anvil carries its historical shape into
//! gameplay — thick everywhere and slow everywhere, with a gun that is honest about what it
//! cannot do. Dossier: docs/vehicles/kv-1.md.

use game_core::{
    ArmorFacing, Era, Nation, ShellSpec, TankSpec, VehicleKind, known_tank_specs,
    resolve_penetration,
};

fn kv1() -> TankSpec {
    VehicleKind::KV1_1942.spec()
}

#[test]
fn kv1_spec_matches_core_historical_shape() {
    let tank = kv1();

    assert_eq!(tank.name, "KV-1 obr. 1942");
    assert_eq!(tank.kind, VehicleKind::KV1_1942);
    assert!((tank.mass_kg - 47_000.0).abs() < 1_000.0, "a ~47 t heavy: {}", tank.mass_kg);
    assert!((tank.engine_power_kw - 441.0).abs() < 15.0, "the V-2K's 600 hp");
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::HullFront), 90.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::HullSide), 75.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::TurretFront), 100.0);
    assert_eq!(tank.gun.name, "76 mm ZiS-5");
    assert_eq!(tank.gun.shell.caliber_mm, 76.2);
    assert_eq!(tank.ammo_capacity, 114, "the deepest rack in the game");
}

#[test]
fn kv1_is_the_soviet_era_ii_heavy() {
    assert_eq!(VehicleKind::KV1_1942.nation(), Nation::Ussr);
    // A 1942 vehicle in the 1943-45 bracket, deliberately: Era I is empty and a one-vehicle era
    // is forbidden, and cast-turret KV-1s fought into 1943 against the Tigers in this bracket.
    assert_eq!(VehicleKind::KV1_1942.era(), Era::LateWar);
    assert!(VehicleKind::PLAYABLE.contains(&VehicleKind::KV1_1942));
    let names: Vec<_> = known_tank_specs().into_iter().map(|spec| spec.name).collect();
    assert!(names.contains(&"KV-1 obr. 1942".to_string()));
}

/// The armour identity: this tank has no cheap angle. Its hull side is the thickest in Era II
/// bar the Tigers', and its TURRET side is the thickest in the bracket outright — there is no
/// thin cheek to hunt for the way there is on a Panther or a T-34-85.
#[test]
fn the_kv_has_no_cheap_flank() {
    let kv = kv1();
    let kv_turret_side = kv.hull.nominal_thickness_mm(ArmorFacing::TurretSide);

    for other in [VehicleKind::T34_85, VehicleKind::PantherII, VehicleKind::TigerI] {
        let spec = other.spec();
        assert!(
            kv_turret_side > spec.hull.nominal_thickness_mm(ArmorFacing::TurretSide),
            "the KV's 100 mm turret flank should beat {other:?}'s"
        );
    }
    // And the hull side is within a whisker of the bow: angling this hull changes little.
    let front = kv.hull.nominal_thickness_mm(ArmorFacing::HullFront);
    let side = kv.hull.nominal_thickness_mm(ArmorFacing::HullSide);
    assert!(side >= front * 0.8, "the KV's side ({side}) is nearly its bow ({front})");
}

/// The price of that armour: the slowest vehicle in the game outright, and the worst-steering
/// TURRETED one. Only the Jagdtiger turns tighter-fisted, and it has to — a fixed casemate steers
/// its whole hull to aim, so its sluggishness is a gun-handling stat, not a mobility one.
#[test]
fn the_kv_is_the_slowest_thing_in_the_game() {
    let kv = kv1();
    for other in VehicleKind::PLAYABLE.iter().filter(|k| **k != VehicleKind::KV1_1942) {
        let spec = other.spec();
        assert!(
            kv.max_forward_speed_mps < spec.max_forward_speed_mps,
            "{other:?} should out-run the KV ({} vs {})",
            spec.max_forward_speed_mps,
            kv.max_forward_speed_mps
        );
        if *other != VehicleKind::Jagdtiger {
            assert!(
                kv.turn_rate_rad_s <= spec.turn_rate_rad_s,
                "{other:?} should out-turn the KV ({} vs {})",
                spec.turn_rate_rad_s,
                kv.turn_rate_rad_s
            );
        }
    }
    assert!(
        VehicleKind::Jagdtiger.spec().turn_rate_rad_s < kv.turn_rate_rad_s,
        "the casemate TD is the one thing that turns worse"
    );
}

/// The honest limit, stated as a test so nobody quietly buffs it away: the ZiS-5's AP shell
/// cannot open a Tiger I from the front — not at point blank, not at any range. The KV's answer
/// to a Tiger is the flank, the tracks, or the scarce arrowhead round; it is not the bow.
#[test]
fn the_zis5_cannot_open_a_tiger_from_the_front() {
    let kv = kv1();
    let target = TankSpec::tiger_i_ausf_e();
    let ap = ShellSpec::armor_piercing(
        kv.gun.shell.caliber_mm,
        kv.gun.shell.muzzle_velocity_mps,
        kv.gun.shell.penetration_mm_at_100m,
        kv.gun.shell.damage_hp,
    );

    for facing in [ArmorFacing::HullFront, ArmorFacing::TurretFront] {
        let flat_shot = target.hull.facet(facing).slope_degrees;
        let result = resolve_penetration(&ap, &target.hull, facing, flat_shot);
        assert!(!result.penetrated, "86 mm of AP must not beat the Tiger's {facing:?}");
    }
}

/// ...but the BR-350P arrowhead does, and only just — the round that makes the matchup playable
/// without making it easy. If this ever flips, the KV's whole role has quietly changed.
#[test]
fn the_arrowhead_round_only_just_does() {
    let kv = kv1();
    let target = TankSpec::tiger_i_ausf_e();
    let apcr = kv.gun.ammo_options()[1];
    assert!(apcr.penetration_mm_at_100m > kv.gun.shell.penetration_mm_at_100m, "APCR digs deeper");

    let flat_shot = target.hull.facet(ArmorFacing::HullFront).slope_degrees;
    let result = resolve_penetration(&apcr, &target.hull, ArmorFacing::HullFront, flat_shot);
    assert!(result.penetrated, "the arrowhead round opens the Tiger's bow up close");

    // Sub-caliber: it bleeds hard with range, so the window is a knife-fight one.
    assert!(
        apcr.penetration_mm_at_distance(1_000.0) < apcr.penetration_mm_at_100m * 0.85,
        "APCR penetration falls off with range"
    );
}
