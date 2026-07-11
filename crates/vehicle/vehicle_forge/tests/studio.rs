//! Contract tests for the Forge Studio bundle: deterministic bytes (the golden-compare
//! foundation), every review camera present as a tile, and a report that quotes the numbers
//! the gates enforce.

use vehicle_forge::bake_studio_bundle;

#[test]
fn studio_bundle_is_deterministic() {
    let kind = game_core::VehicleKind::IS3;
    let first = bake_studio_bundle(kind).expect("first bake");
    let second = bake_studio_bundle(kind).expect("second bake");

    assert_eq!(first.report_md(), second.report_md(), "report must be deterministic");
    assert_eq!(
        first.contact_sheet_png(),
        second.contact_sheet_png(),
        "contact sheet must be byte-deterministic"
    );
    assert_eq!(first.views().len(), second.views().len());
    for (a, b) in first.views().iter().zip(second.views()) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.png, b.png, "view {} must be byte-deterministic", a.name);
    }
}

#[test]
fn the_report_quotes_every_gate_the_fleet_is_held_to() {
    let bundle = bake_studio_bundle(game_core::VehicleKind::Centurion).expect("bakes");
    let report = bundle.report_md();
    for section in [
        "## Dimensions",
        "## Reference ratios",
        "## Budgets",
        "## Mesh quality",
        "## Determinism",
        "## Blueprint lint",
        "## The loop",
    ] {
        assert!(report.contains(section), "report.md is missing the {section} section");
    }
    // The golden-hash verdict is stated in plain words the author can act on.
    assert!(
        report.contains("MATCHES the recorded golden") || report.contains("DIFFERS from golden"),
        "the determinism section must state the golden verdict"
    );
    // The loop names the RON file the author edits.
    assert!(report.contains("centurion_mk3.blueprint.ron"));

    // Every review camera produced a tile.
    assert!(!bundle.views().is_empty());
    assert!(bundle.views().iter().all(|view| !view.png.is_empty()));
}
