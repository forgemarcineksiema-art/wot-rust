//! Contract tests for the RON blueprint source: the file set and the `VehicleKind` roster are
//! a bijection (no orphan data files, no fileless kinds), every file re-parses with digit
//! fidelity, and the whole fleet passes the teaching lint clean.

use std::path::PathBuf;

use game_core::{BlueprintFile, VehicleBlueprint, VehicleKind, lint};

fn blueprints_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("blueprints")
}

/// Every `*.blueprint.ron` maps to a `VehicleKind` and every blueprint-backed kind has its
/// file — the data-file version of the old "missing match arm = compile error" guarantee.
/// (The compile-time half is the exhaustive `include_str!` match in `source.rs`.)
#[test]
fn blueprint_dir_and_vehicle_roster_are_a_bijection() {
    let mut file_slugs: Vec<String> = std::fs::read_dir(blueprints_dir())
        .expect("blueprints dir exists")
        .map(|entry| entry.expect("dir entry").file_name().to_string_lossy().into_owned())
        .filter_map(|name| name.strip_suffix(".blueprint.ron").map(str::to_owned))
        .collect();
    file_slugs.sort();

    let mut kind_slugs: Vec<String> = VehicleKind::ALL
        .into_iter()
        .filter(|&kind| VehicleBlueprint::for_vehicle(kind).is_some())
        .map(|kind| kind.slug().to_owned())
        .collect();
    kind_slugs.sort();

    assert_eq!(
        file_slugs, kind_slugs,
        "blueprints/ and the blueprint-backed roster must match one-to-one"
    );
}

/// Every file's `kind` tag matches the registry arm it is included under: re-serializing the
/// parsed value and parsing it again must reproduce the same blueprint (digit fidelity — f32
/// round-trips at shortest-precision), WITHOUT requiring textual equality, so hand-written
/// comments in the files survive.
#[test]
fn blueprint_files_round_trip_with_digit_fidelity() {
    let mut round_tripped = 0;
    for kind in VehicleKind::ALL {
        let Some(blueprint) = VehicleBlueprint::for_vehicle(kind) else {
            continue;
        };
        round_tripped += 1;
        let file = BlueprintFile::from_blueprint(&blueprint);
        assert_eq!(file.kind, kind, "{kind:?}: file kind tag must match its registry arm");

        let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::new())
            .expect("blueprint serializes");
        let reparsed = game_core::parse_blueprint(kind, &text)
            .unwrap_or_else(|error| panic!("{kind:?} round-trip failed: {error}"));
        assert_eq!(
            reparsed, blueprint,
            "{kind:?}: serialize->parse must reproduce the identical blueprint"
        );
    }
    assert_eq!(
        round_tripped,
        VehicleKind::PLAYABLE.len(),
        "every playable vehicle is blueprint-born, so every one of them must round-trip — a          `continue` here used to mean this test could check nothing and still pass"
    );
}

/// The whole fleet passes the teaching lint with zero ERRORS (warnings are allowed — they are
/// studio-report material, not gate material).
#[test]
fn all_blueprints_pass_lint() {
    let mut linted = 0;
    for kind in VehicleKind::ALL {
        let Some(blueprint) = VehicleBlueprint::for_vehicle(kind) else {
            continue;
        };
        linted += 1;
        let errors: Vec<_> = lint::validate_blueprint(&blueprint)
            .into_iter()
            .filter(|issue| issue.severity == lint::Severity::Error)
            .collect();
        assert!(
            errors.is_empty(),
            "{kind:?} blueprint fails lint:\n{}",
            errors
                .iter()
                .map(|issue| format!("  - {}: {}", issue.field, issue.message))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    assert_eq!(
        linted,
        VehicleKind::PLAYABLE.len(),
        "the lint must have been pointed at every blueprint-born vehicle, not at whatever the \
         registry happened to answer for"
    );
}

/// The lint TEACHES: a physically impossible dome (ring wider than the casting) is refused
/// with a message that names both values and the historical constraint.
#[test]
fn lint_teaches_the_dome_overhang_rule() {
    let mut blueprint = VehicleBlueprint::for_vehicle(VehicleKind::IS3).expect("IS-3 loads");
    blueprint.turret.ring_radius = blueprint.turret.base_radius + 0.1;
    let issues = lint::validate_blueprint(&blueprint);
    let dome = issues
        .iter()
        .find(|issue| issue.field == "turret.ring_radius")
        .expect("the impossible dome is flagged");
    assert_eq!(dome.severity, lint::Severity::Error);
    assert!(
        dome.message.contains("overhang") || dome.message.contains("OVERHANG"),
        "the message explains the WHY, got: {}",
        dome.message
    );
}

/// The honesty rule with teeth: a plate may not LOOK one way and RESOLVE another.
///
/// Five angles are authored twice — once on the shape (the mesh) and once on the armour (the
/// shell). Three had already drifted before this lint existed: the T-54's rear plate (8 vs 5
/// deg) and the Panther II's turret face and rear (11 vs 20, 25 vs 20). Nothing compared them,
/// so a shell could ricochet off an angle no player could see.
#[test]
fn lint_refuses_a_plate_that_looks_one_way_and_resolves_another() {
    for (field, perturb) in [
        ("hull.glacis_slope_deg", 0usize),
        ("hull.rear_slope_deg", 1),
        ("turret.front_slope_deg", 2),
        ("turret.side_slope_deg", 3),
        ("turret.rear_slope_deg", 4),
    ] {
        let mut blueprint =
            VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 loads");
        match perturb {
            0 => blueprint.hull.glacis_slope_deg += 7.0,
            1 => blueprint.hull.rear_slope_deg += 7.0,
            2 => blueprint.turret.front_slope_deg += 7.0,
            3 => blueprint.turret.side_slope_deg += 7.0,
            _ => blueprint.turret.rear_slope_deg += 7.0,
        }
        let issues = lint::validate_blueprint(&blueprint);
        let drift = issues
            .iter()
            .find(|issue| issue.field == field)
            .unwrap_or_else(|| panic!("{field} drift must be flagged"));
        assert_eq!(drift.severity, lint::Severity::Error, "{field}: drift blocks the load");
        assert!(
            drift.message.contains("RESOLVES") && drift.message.contains("dossier"),
            "the message must teach the rule and name the tiebreaker, got: {}",
            drift.message
        );
    }
}

/// And the fleet as shipped satisfies it — the three reconciliations (T-54 rear to 5 deg,
/// Panther II turret face and rear to 20 deg) are locked here, not just in the RON.
#[test]
fn every_shipped_blueprint_states_each_slope_exactly_once() {
    let mut reconciled = 0;
    for kind in VehicleKind::PLAYABLE {
        let Some(bp) = VehicleBlueprint::for_vehicle(kind) else { continue };
        reconciled += 1;
        for (name, shape, armor) in [
            ("glacis", bp.hull.glacis_slope_deg, bp.armor.hull_front.0),
            ("hull rear", bp.hull.rear_slope_deg, bp.armor.hull_rear.0),
            ("turret front", bp.turret.front_slope_deg, bp.armor.turret_front.0),
            ("turret side", bp.turret.side_slope_deg, bp.armor.turret_side.0),
            ("turret rear", bp.turret.rear_slope_deg, bp.armor.turret_rear.0),
        ] {
            assert!(
                (shape - armor).abs() < 1.0e-6,
                "{kind:?} {name}: shape {shape} vs armour {armor}"
            );
        }
    }
    assert_eq!(
        reconciled,
        VehicleKind::PLAYABLE.len(),
        "the slope reconciliation covers the whole shipped fleet or it covers nothing provable"
    );
}
