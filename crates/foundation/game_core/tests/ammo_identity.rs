//! Locks for the concrete-round identity (`RoundId`, Amunicja 3.0 B1): every slot of every gun
//! names WHICH round it chambers, the catalog is the ONE authoring point for shell data, and a
//! round two guns really shared is one identity, not two coincidentally equal numbers.

use std::collections::{HashMap, HashSet};

use game_core::{RoundId, ShellSpec, VehicleKind};

fn every_fielded_shell() -> Vec<(String, ShellSpec)> {
    let mut shells = Vec::new();
    for kind in VehicleKind::ALL {
        for gun in kind.gun_options() {
            for shell in gun.spec.ammo_options() {
                shells.push((gun.spec.name.clone(), shell));
            }
        }
    }
    shells
}

/// The single-source-of-truth lock: a gun stating shell numbers of its own — a slot whose spec
/// differs from its round's catalog entry, or a slot with no identity at all — is the drift
/// `ammo_catalog.rs` exists to end.
#[test]
fn every_slot_names_its_round_and_matches_the_catalog() {
    let shells = every_fielded_shell();
    assert!(shells.len() >= 26, "the whole fleet must be examined, saw {}", shells.len());
    for (gun, shell) in shells {
        let round = shell
            .round
            .unwrap_or_else(|| panic!("{gun}: a fielded slot must name its concrete round"));
        assert_eq!(
            shell,
            round.spec(),
            "{gun}: the slot's spec must BE {}'s catalog entry, not a private copy",
            round.designation()
        );
    }
}

/// The sharing lock: rounds that were one physical shell are ONE `RoundId`. The facts a per-gun
/// anonymous spec could never state: both 88s (and the Pak 43/3) fire the same Sprgr L/4.5, and
/// the whole D-10 family loads the same BK-5 and OF-412.
#[test]
fn shared_rounds_are_the_same_identity_not_equal_numbers() {
    let by_gun: HashMap<String, Vec<RoundId>> =
        every_fielded_shell().into_iter().fold(HashMap::new(), |mut map, (gun, shell)| {
            map.entry(gun).or_default().extend(shell.round);
            map
        });
    let fires =
        |gun: &str, round: RoundId| by_gun.get(gun).is_some_and(|rounds| rounds.contains(&round));
    for gun in ["8.8 cm KwK 36 L/56", "8.8 cm KwK 43 L/71", "8.8 cm Pak 43/3 L/71"] {
        assert!(fires(gun, RoundId::SprgrL45), "{gun} fires the shared Sprgr L/4.5");
    }
    for gun in ["100 mm D-10T", "100 mm D-10T2S"] {
        assert!(fires(gun, RoundId::Bk5), "{gun} loads the family's BK-5");
        assert!(fires(gun, RoundId::Of412), "{gun} loads the family's OF-412");
    }
    for gun in ["84 mm 20-pounder Type A", "84 mm 20-pounder Type B"] {
        for round in [RoundId::TwentyPdrApcbc, RoundId::TwentyPdrApds, RoundId::TwentyPdrHe] {
            assert!(fires(gun, round), "{gun} fires the same three rounds as its sibling");
        }
    }
    // And the family's stock rounds genuinely DIFFER — the D-10T2S's BR-412D is a sidegrade,
    // not a renamed BR-412.
    assert!(fires("100 mm D-10T", RoundId::Br412));
    assert!(fires("100 mm D-10T2S", RoundId::Br412D));
    assert_ne!(RoundId::Br412.spec(), RoundId::Br412D.spec());
}

/// Designations are the HUD's words: unique, non-empty, one per identity.
#[test]
fn every_designation_is_unique_and_non_empty() {
    let mut seen = HashSet::new();
    for round in RoundId::ALL {
        let name = round.designation();
        assert!(!name.is_empty(), "{round:?} has no designation");
        assert!(seen.insert(name), "designation {name:?} is used by two rounds");
    }
}

/// No orphan identities: every cataloged round is fielded by at least one gun. An identity
/// nothing chambers is either a typo or a round that should not have been added yet — appending
/// on demand is the doctrine working.
#[test]
fn every_round_in_the_catalog_is_fielded_by_a_gun() {
    let fielded: HashSet<RoundId> =
        every_fielded_shell().into_iter().filter_map(|(_, shell)| shell.round).collect();
    for round in RoundId::ALL {
        assert!(
            fielded.contains(&round),
            "{} ({round:?}) is cataloged but no gun chambers it",
            round.designation()
        );
    }
}
