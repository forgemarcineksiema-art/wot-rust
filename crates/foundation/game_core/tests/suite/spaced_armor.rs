//! Spaced armour must never subtract.
//!
//! A side skirt is the outermost layer of a stack — skirt, then belt, then the hull side plate.
//! Resolving the skirt as if it REPLACED the belt made bolting Schürzen onto a hull reduce its
//! flank armour: on the two vehicles in the fleet that carry them, the plate whose entire
//! purpose is standoff was worth −20 mm against AP and −40 mm against a HEAT jet. These tests
//! lock the direction of the effect, per vehicle and per shell, so no future layer can invert it
//! again.

use game_core::{
    ArmorProfile, ArmorZone, ShellSpec, VehicleBlueprint, VehicleKind,
    resolve_penetration_through_screens,
};

/// One representative of every family of round the armour model treats differently.
fn probe_shells() -> [(&'static str, ShellSpec); 3] {
    [
        ("AP", ShellSpec::armor_piercing(100.0, 900.0, 200.0, 320)),
        ("APCR", ShellSpec::apcr(100.0, 1_100.0, 250.0, 280)),
        ("HEAT", ShellSpec::heat(100.0, 900.0, 300.0, 320)),
    ]
}

fn effective_mm(shell: &ShellSpec, armor: &ArmorProfile, screens: &[ArmorZone]) -> f32 {
    resolve_penetration_through_screens(shell, armor, screens, 0.0, 0.0, 100.0).effective_armor_mm
}

/// The fleet-level promise, checked against the vehicles that actually carry the plate.
#[test]
fn a_skirt_never_makes_a_flank_easier_than_the_bare_belt_it_hangs_over() {
    let mut skirted = 0;
    for kind in VehicleKind::PLAYABLE {
        let Some(blueprint) = VehicleBlueprint::for_vehicle(kind) else {
            continue;
        };
        if blueprint.hull.skirt.is_none() {
            continue;
        }
        skirted += 1;
        let armor = kind.spec().hull;
        for (name, shell) in probe_shells() {
            let with_skirt =
                effective_mm(&shell, &armor, &[ArmorZone::Skirt, ArmorZone::RightTrack]);
            let bare_belt = effective_mm(&shell, &armor, &[ArmorZone::RightTrack]);
            assert!(
                with_skirt > bare_belt,
                "{kind:?} vs {name}: the skirt must ADD to the flank, not replace the belt — \
                 skirted {with_skirt:.1} mm vs bare {bare_belt:.1} mm"
            );
        }
    }
    assert!(
        skirted >= 2,
        "the fleet must still field the skirted vehicles this rule exists for (got {skirted})"
    );
}

/// The general rule the fleet case is one instance of: every layer standing off the hull can
/// only cost the shell more. Monotone in the stack, for every shell family.
#[test]
fn each_spaced_layer_only_adds_to_the_line_of_sight_steel() {
    let armor = VehicleKind::Centurion.spec().hull;
    for (name, shell) in probe_shells() {
        let bare = effective_mm(&shell, &armor, &[]);
        let belt = effective_mm(&shell, &armor, &[ArmorZone::RightTrack]);
        let belt_and_skirt =
            effective_mm(&shell, &armor, &[ArmorZone::Skirt, ArmorZone::RightTrack]);
        assert!(bare < belt, "{name}: a belt must screen the bare plate ({bare} -> {belt})");
        assert!(
            belt < belt_and_skirt,
            "{name}: a skirt must screen the belt ({belt} -> {belt_and_skirt})"
        );
    }
}

/// An empty stack is the honest description of a THROWN belt: the shot meets the bare side
/// plate, exactly as if the running gear were not there — because it is not.
#[test]
fn an_empty_stack_resolves_as_the_bare_side_plate() {
    let armor = VehicleKind::Centurion.spec().hull;
    let shell = ShellSpec::armor_piercing(100.0, 900.0, 200.0, 320);
    let empty = resolve_penetration_through_screens(&shell, &armor, &[], 0.0, 12.0, 100.0);
    let plain = game_core::resolve_penetration_at_distance_on_zone(
        &shell,
        &armor,
        ArmorZone::HullSide,
        12.0,
        100.0,
    );
    assert_eq!(empty, plain, "no screens is a plain hull-side hit");
}

/// HE fuzes on the first thing it touches: the outermost layer is the whole story, and the
/// stack behind it is irrelevant to a surface burst.
#[test]
fn high_explosive_bursts_on_the_outermost_layer_alone() {
    let armor = VehicleKind::Centurion.spec().hull;
    let he = ShellSpec::high_explosive(100.0, 600.0, 70.0, 450, 1.5);
    let one =
        resolve_penetration_through_screens(&he, &armor, &[ArmorZone::Skirt], 0.0, 0.0, 100.0);
    let stacked = resolve_penetration_through_screens(
        &he,
        &armor,
        &[ArmorZone::Skirt, ArmorZone::RightTrack],
        0.0,
        0.0,
        100.0,
    );
    assert_eq!(one, stacked, "a surface burst never sees past the plate it went off on");
    assert!(!one.penetrated, "an HE burst on running gear is never an interior hit");
}
