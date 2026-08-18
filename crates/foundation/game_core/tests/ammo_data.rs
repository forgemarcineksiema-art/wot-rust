//! Locks for the shell's physical data (Amunicja 3.0 B2): mass, bursting charge and penetrator
//! are AUTHORED per concrete round from `docs/ammunition.md`, and the relationships the physics
//! will lean on — tungsten cores are light solid shot, HE damage follows its filler through the
//! O-365K anchor law, the 12.8 cm slams hardest — hold across the whole catalog.

use game_core::{Penetrator, RoundId, ShellType, VehicleKind};

/// The sourced, high-confidence masses are pinned EXACTLY: a typo in a mass column is invisible
/// to every behavioral test until it has already warped drag, energy and spall.
#[test]
fn the_sourced_masses_are_authored_verbatim() {
    let expect = [
        (RoundId::Br412, 15.7),
        (RoundId::Of412, 15.8),
        (RoundId::Br365P, 4.95),
        (RoundId::Of471, 25.53),
        (RoundId::SprgrL45, 9.4),
        (RoundId::Pzgr39_43, 10.4),
        (RoundId::Pzgr43, 28.3),
        (RoundId::Pzgr40_42, 4.75),
    ];
    for (round, mass_kg) in expect {
        assert_eq!(
            round.spec().mass_kg,
            mass_kg,
            "{}: the dossier's mass column is the authored value",
            round.designation()
        );
    }
}

/// Every cataloged round carries a mass — `0.0` is the LEGACY marker for synthetic test shells,
/// and no fielded round may hide behind it.
#[test]
fn no_cataloged_round_is_massless() {
    for round in RoundId::ALL {
        assert!(
            round.spec().mass_kg > 0.0,
            "{}: a cataloged round authors its mass",
            round.designation()
        );
    }
}

/// The class facts the penetrator enum encodes, checked against the data rather than trusted:
/// a tungsten core is LIGHT solid shot (no burster, less mass than its gun's full-bore round),
/// and every blast case carries a real charge.
#[test]
fn tungsten_cores_are_light_solid_shot_and_blast_cases_carry_their_charge() {
    for round in RoundId::ALL {
        let spec = round.spec();
        match spec.penetrator {
            Penetrator::TungstenCore => {
                assert_eq!(spec.filler_kg, 0.0, "{}: a core has no burster", round.designation());
            }
            Penetrator::BlastCase => {
                assert!(
                    spec.filler_kg > 0.0,
                    "{}: an HE shell without a charge is a data hole",
                    round.designation()
                );
            }
            _ => {}
        }
    }
    // The stock-vs-special mass relation, per gun that fields a core.
    let mut cores_checked = 0;
    for kind in VehicleKind::ALL {
        for gun in kind.gun_options() {
            let Some(special) = gun.spec.special_shell else { continue };
            if special.penetrator == Penetrator::TungstenCore {
                assert!(
                    special.mass_kg < gun.spec.shell.mass_kg,
                    "{}: the tungsten round must be lighter than the full-bore round it rides \
                     beside ({} vs {} kg)",
                    gun.spec.name,
                    special.mass_kg,
                    gun.spec.shell.mass_kg
                );
                cores_checked += 1;
            }
        }
    }
    assert!(cores_checked >= 8, "every core-firing gun must be reached, saw {cores_checked}");
}

/// The Soviet APBC family is BLUNT — the terminal identity B5 will differentiate — and nobody
/// else is: the German APCBC/20-pdr/prototype full-bore rounds stay sharp.
#[test]
fn the_blunt_nose_belongs_to_the_soviet_apbc_family_alone() {
    for round in RoundId::ALL {
        let spec = round.spec();
        let blunt =
            matches!(round, RoundId::Br412 | RoundId::Br412D | RoundId::Br365K | RoundId::Br471B);
        if spec.shell_type == ShellType::ArmorPiercing {
            let expected =
                if blunt { Penetrator::FullBoreBlunt } else { Penetrator::FullBoreSharp };
            assert_eq!(
                spec.penetrator,
                expected,
                "{}: the nose form is part of the round's identity",
                round.designation()
            );
        }
    }
}

/// The HE pricing decision, executable: damage follows the FILLER through the O-365K anchor
/// (0.741 kg → 300 HP, cube-root blast scaling). Sourced fillers land within 6% (the 88's
/// near-identical shell is deliberately priced AT the anchor); back-derived fillers land exact.
#[test]
fn high_explosive_damage_follows_its_filler_through_the_anchor_law() {
    let mut priced = 0;
    for round in RoundId::ALL {
        let spec = round.spec();
        if spec.penetrator != Penetrator::BlastCase {
            continue;
        }
        let predicted = 300.0 * (spec.filler_kg / 0.741_f32).powf(1.0 / 3.0);
        let relative = (spec.damage_hp as f32 - predicted).abs() / spec.damage_hp as f32;
        assert!(
            relative < 0.06,
            "{}: {} HP against the anchor law's {:.0} — the filler column and the damage \
             column must not disagree",
            round.designation(),
            spec.damage_hp,
            predicted
        );
        priced += 1;
    }
    assert_eq!(priced, 8, "every HE round in the catalog must be priced by the law");
}

/// Drag comes from the round's own body (B3): heavier same-class shells carry their speed,
/// light tungsten cores shed it, every round stays inside its class band (readability), and a
/// legacy massless spec keeps the old class constants EXACTLY — synthetic shells do not move.
#[test]
fn drag_follows_the_rounds_own_body_inside_its_class_band() {
    let drag = |round: RoundId| round.spec().drag_per_s();
    // The heavy 12.8 cm carries; the light 7.5 cm bleeds — same full-bore class.
    assert!(
        drag(RoundId::Pzgr43) < drag(RoundId::Pzgr39_42),
        "a 28.3 kg shell must hold its speed better than a 6.8 kg one: {} vs {}",
        drag(RoundId::Pzgr43),
        drag(RoundId::Pzgr39_42)
    );
    // A core sheds speed faster than the full-bore round beside it in the same rack.
    assert!(
        drag(RoundId::Pzgr40_43) > drag(RoundId::Pzgr39_43),
        "the tungsten core bleeds harder than the gun's own APCBC"
    );
    // Class bands: a concrete shell may fly flatter or bleed faster, it may not impersonate
    // another class.
    let mut banded = 0;
    for round in RoundId::ALL {
        let spec = round.spec();
        let band = match spec.penetrator {
            Penetrator::FullBoreSharp | Penetrator::FullBoreBlunt => 0.07..=0.12,
            Penetrator::TungstenCore => 0.17..=0.24,
            Penetrator::ShapedCharge | Penetrator::BlastCase => 0.05..=0.05,
        };
        assert!(
            band.contains(&spec.drag_per_s()),
            "{}: drag {} left its class band {band:?}",
            round.designation(),
            spec.drag_per_s()
        );
        banded += 1;
    }
    assert_eq!(banded, 25, "every cataloged round must sit in a band");
    // The legacy fallback: a massless synthetic spec keeps the pre-B3 constants verbatim.
    assert_eq!(game_core::ShellSpec::armor_piercing(100.0, 900.0, 200.0, 320).drag_per_s(), 0.09);
    assert_eq!(game_core::ShellSpec::apcr(100.0, 1_000.0, 250.0, 300).drag_per_s(), 0.21);
    assert_eq!(game_core::ShellSpec::heat(100.0, 900.0, 280.0, 320).drag_per_s(), 0.05);
}

/// The muzzle-energy order the fleet's feel is built on: the 12.8 cm Pzgr 43 slams hardest of
/// every kinetic round — with real masses, that is now a computable fact instead of lore.
#[test]
fn the_pak_80_round_carries_the_fleets_top_muzzle_energy() {
    let top = RoundId::Pzgr43.spec();
    let top_energy = top.impact_energy_kj(top.muzzle_velocity_mps);
    let mut contenders = 0;
    for round in RoundId::ALL {
        let spec = round.spec();
        if round == RoundId::Pzgr43 || spec.shell_type == ShellType::HighExplosive {
            continue;
        }
        let energy = spec.impact_energy_kj(spec.muzzle_velocity_mps);
        assert!(
            energy < top_energy,
            "{}: {:.0} kJ must stay under the Pzgr 43's {:.0} kJ",
            round.designation(),
            energy,
            top_energy
        );
        contenders += 1;
    }
    assert_eq!(contenders, 16, "every non-HE round in the catalog must contest the crown");
}
