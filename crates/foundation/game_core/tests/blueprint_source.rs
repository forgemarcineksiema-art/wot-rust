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
    for kind in VehicleKind::ALL {
        let Some(blueprint) = VehicleBlueprint::for_vehicle(kind) else {
            continue;
        };
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
}

/// The whole fleet passes the teaching lint with zero ERRORS (warnings are allowed — they are
/// studio-report material, not gate material).
#[test]
fn all_blueprints_pass_lint() {
    for kind in VehicleKind::ALL {
        let Some(blueprint) = VehicleBlueprint::for_vehicle(kind) else {
            continue;
        };
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
