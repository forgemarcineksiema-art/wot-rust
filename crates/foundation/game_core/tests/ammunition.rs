//! Every round a gun fires is AUTHORED — nothing computed from another shell.
//!
//! `ammo_options()` used to derive two of the three: an APCR at `x 1.20` velocity / `x 1.25`
//! penetration / `x 0.85` damage of the stock round, and an HE at `x 0.70` / `x 0.35` / `x 1.4`.
//! Both are gone. The research behind the replacements is `docs/ammunition.md`, with a source and
//! a confidence per row and six holes recorded as GAP rather than filled with a guess.

use game_core::{ShellType, VehicleKind};

/// Every gun in the catalog authors its high-explosive round.
///
/// The field is `Option` for wire compatibility only; a gun with no HE would silently ship a
/// two-slot rack whose second slot is its special round, and nobody would notice until a player
/// looked for HE and found none.
#[test]
fn every_gun_authors_its_high_explosive_round() {
    let mut checked = 0;
    for kind in VehicleKind::ALL {
        for gun in kind.gun_options() {
            let options = gun.spec.ammo_options();
            assert_eq!(
                options.last().map(|shell| shell.shell_type),
                Some(ShellType::HighExplosive),
                "{}: the last slot must be the authored HE round",
                gun.spec.name
            );
            checked += 1;
        }
    }
    assert!(checked >= 12, "every gun in the fleet must be examined, saw {checked}");
}

/// A gun carries the rounds it fielded — the slot count is a property of the weapon.
///
/// The 12.8 cm Pak 80 and the 122 mm D-25T fielded no tungsten round. They used to be handed a
/// fabricated APCR (279 mm for the Pak 80) because the fallback existed; now they carry two slots,
/// and that is the gun's identity rather than a gap in the data.
#[test]
fn a_gun_that_fielded_no_second_round_carries_two_slots() {
    // The fictional prototype is NOT here: everything about an invented vehicle is invented, and
    // it stays three-slot because it is the fleet's general test tank. What the fallback did wrong
    // was handing REAL guns rounds they never fired.
    let two_slot = ["12.8 cm Pak 80 L/55", "122 mm D-25T"];
    let mut seen = 0;
    for kind in VehicleKind::ALL {
        for gun in kind.gun_options() {
            let options = gun.spec.ammo_options();
            if two_slot.contains(&gun.spec.name.as_str()) {
                assert_eq!(options.len(), 2, "{} never fielded a special round", gun.spec.name);
                seen += 1;
            } else {
                assert_eq!(options.len(), 3, "{} fields a second round", gun.spec.name);
            }
        }
    }
    assert_eq!(seen, 2, "both two-slot guns must have been reached");
}

/// High explosive is priced by the SHELL, not by the gun's armour-piercing round.
///
/// Two consequences of the old multipliers, both fixed here and both locked so they cannot come
/// back: an 84 mm gun cannot out-penetrate a 122 mm with high explosive, and two guns of different
/// caliber cannot fire identical HE just because their AP alpha happens to match.
#[test]
fn high_explosive_ranks_by_caliber_not_by_the_armour_piercing_round() {
    let he = |name: &str| {
        VehicleKind::ALL
            .iter()
            .flat_map(|kind| kind.gun_options())
            .find(|gun| gun.spec.name == name)
            .and_then(|gun| gun.spec.ammo_options().last().copied())
            .unwrap_or_else(|| panic!("{name} is in the catalog"))
    };
    let light = he("84 mm 20-pounder Type A");
    let heavy = he("122 mm D-25T");
    assert!(
        heavy.penetration_mm_at_100m > light.penetration_mm_at_100m,
        "a 122 mm HE shell must out-penetrate an 84 mm one: {} vs {}",
        heavy.penetration_mm_at_100m,
        light.penetration_mm_at_100m
    );
    assert!(
        heavy.damage_hp > light.damage_hp,
        "and hit harder: {} vs {}",
        heavy.damage_hp,
        light.damage_hp
    );
    // The 88s and the 85 fire near-identical shells (9.4 kg / 0.870 kg against 9.54 / 0.741), so
    // their HE lands on the same number. The old derivation gave the 88 nearly twice the 85's HE
    // damage purely because its ARMOUR-PIERCING round hits harder.
    assert_eq!(he("8.8 cm KwK 43 L/71").damage_hp, he("85 mm ZiS-S-53").damage_hp);
}

/// Soviet tank HE was a full-charge round, and the fleet must fly it that way.
#[test]
fn the_d10s_high_explosive_is_not_slower_than_its_armour_piercing_round() {
    let gun = VehicleKind::T54_1951
        .gun_options()
        .into_iter()
        .find(|gun| gun.spec.name == "100 mm D-10T")
        .expect("the T-54 mounts the D-10T");
    let options = gun.spec.ammo_options();
    let he = options.last().expect("HE authored");
    assert!(
        he.muzzle_velocity_mps >= gun.spec.shell.muzzle_velocity_mps,
        "OF-412 leaves the muzzle at 900 m/s against BR-412's 895 — the derivation flew it at \
         626, and flight time is what a player leads with"
    );
}
