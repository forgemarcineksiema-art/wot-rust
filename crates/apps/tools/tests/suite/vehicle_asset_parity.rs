//! One truth about steel: the committed `assets/vehicles/*.vehicle.json` snapshots must equal
//! the module catalog they were generated from. Gameplay reads the catalog (`kind.spec()`),
//! humans and external tools read the JSON — this gate stops the two hand-maintained copies of
//! the armor numbers (and everything else in the spec) from silently drifting apart.
//!
//! When a catalog change is intentional, regenerate the snapshot:
//! `cargo run -p tools -- generate-vehicle --vehicle <slug> --output assets/vehicles/<slug>.vehicle.json`

use game_core::VehicleKind;

fn asset_path(slug: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/vehicles")
        .join(format!("{slug}.vehicle.json"))
}

#[test]
fn every_playable_vehicle_asset_matches_its_catalog_spec() {
    for kind in VehicleKind::PLAYABLE {
        let path = asset_path(kind.slug());
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!("{kind:?}: asset {} unreadable: {error}", path.display())
        });
        let on_disk: serde_json::Value = serde_json::from_str(&raw)
            .unwrap_or_else(|error| panic!("{kind:?}: bad JSON: {error}"));
        // Through the string writer and back, so f32 fields take the same shortest-decimal
        // form the generator writes (a direct `to_value` widens f32→f64 and false-alarms).
        let fresh: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&kind.spec()).expect("spec serializes"))
                .expect("round-trip");

        // Armor thickness first, with a readable message — this is the honesty-critical block.
        for section in [
            "hull_front_mm",
            "hull_side_mm",
            "hull_rear_mm",
            "turret_front_mm",
            "turret_side_mm",
            "turret_rear_mm",
        ] {
            assert_eq!(
                on_disk["hull"][section],
                fresh["hull"][section],
                "{kind:?}: {section} drifted between assets/vehicles/{}.vehicle.json and the \
                 module catalog — regenerate the snapshot or fix the catalog",
                kind.slug()
            );
        }

        // Then the whole spec: the snapshot is generated, so it must be byte-equal in value
        // space (field order and formatting are free to differ).
        assert_eq!(
            on_disk,
            fresh,
            "{kind:?}: assets/vehicles/{}.vehicle.json is stale relative to the module catalog \
             — regenerate it with `cargo run -p tools -- generate-vehicle`",
            kind.slug()
        );
    }
}
