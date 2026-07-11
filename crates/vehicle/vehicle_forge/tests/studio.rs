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

/// Program C's un-T-54'd compiler: a NON-benchmark vehicle compiles with a swapped gun module,
/// and the module genuinely reshapes the geometry — the Jagdtiger's alternate 8.8 cm reaches a
/// different muzzle than the stock 12.8 cm, in the baked mesh and the mount frames alike.
#[test]
fn a_swapped_gun_module_reshapes_a_non_t54_vehicle() {
    use vehicle_forge::{TankCompileRequest, compile_tank};

    let kind = game_core::VehicleKind::Jagdtiger;
    let guns = kind.gun_options();
    assert!(guns.len() >= 2, "the Jagdtiger fields two guns");

    let compiled: Vec<_> = guns
        .into_iter()
        .map(|gun| {
            let mut modules = kind.default_loadout();
            modules.gun = gun;
            compile_tank(TankCompileRequest {
                vehicle: kind,
                modules,
                profile: vehicle_forge::BakeProfile::Lod0,
            })
            .expect("non-T-54 compiles")
        })
        .collect();

    let stock_muzzle = compiled[0].mounts.muzzle.translation.z;
    let alt_muzzle = compiled[1].mounts.muzzle.translation.z;
    assert!(
        (stock_muzzle - alt_muzzle).abs() > 0.2,
        "the swapped gun must move the muzzle: stock {stock_muzzle:.2} vs alt {alt_muzzle:.2}"
    );
    assert_ne!(
        compiled[0].source_hash, compiled[1].source_hash,
        "a different gun is a different baked silhouette"
    );
}
