//! The fast loop must not lie. Two promises are locked here:
//!
//! 1. A live override of the PRISTINE on-disk RON produces exactly the bundle the embedded bake
//!    produces — same mesh source, same tiles, same numbers. Before this, the T-54's fast loop
//!    rendered its dead legacy recipe while the game shipped the hybrid.
//! 2. An edited track moves BOTH halves of the loop — the rendered gear and the measured
//!    anchors — so an author tuning `wheel_radius` sees and measures what they just typed.

use std::path::PathBuf;

use game_core::VehicleKind;
use vehicle_forge::{
    DimensionKind, ReferencePack, authoritative_baked_vehicle, bake_studio_bundle,
    bake_studio_bundle_from_blueprint,
};

fn blueprint_path(kind: VehicleKind) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../foundation/game_core/blueprints")
        .join(format!("{}.blueprint.ron", kind.slug()))
}

fn pristine(kind: VehicleKind) -> game_core::VehicleBlueprint {
    let text = std::fs::read_to_string(blueprint_path(kind)).expect("blueprint file on disk");
    game_core::parse_blueprint(kind, &text).expect("on-disk blueprint parses")
}

#[test]
fn a_live_override_of_the_pristine_ron_reproduces_the_embedded_bundle() {
    // Both migrated (hybrid) and un-migrated (procedural) vehicles must round-trip, so the
    // routing itself is under test, not just the T-54.
    for kind in [VehicleKind::T54_1951, VehicleKind::IS3] {
        let embedded = bake_studio_bundle(kind).expect("embedded bake");
        let live = bake_studio_bundle_from_blueprint(&pristine(kind)).expect("live bake");

        assert_eq!(
            embedded.report_md(),
            live.report_md(),
            "{kind:?}: the live loop must report the same numbers as the embedded bake"
        );
        assert_eq!(
            embedded.contact_sheet_png(),
            live.contact_sheet_png(),
            "{kind:?}: the live loop must draw the same mesh as the embedded bake"
        );
        for (a, b) in embedded.views().iter().zip(live.views()) {
            assert_eq!(a.png, b.png, "{kind:?}: view {} drifted between the two paths", a.name);
        }
    }
}

#[test]
fn the_t54_fast_loop_bakes_the_hybrid_the_game_ships() {
    // The bundle's own report names the source; the hybrid contract lines only appear for the
    // hybrid path. If the live loop ever falls back to the procedural recipe, this fails.
    let live = bake_studio_bundle_from_blueprint(&pristine(VehicleKind::T54_1951))
        .expect("live T-54 bake");
    let report = live.report_md();
    assert!(report.contains("hybrid production mesh"), "the live T-54 must bake the hybrid");
    assert!(
        report.contains("Fast-loop caveat (hybrid source)"),
        "the report must warn that the Rust-side hybrid tree does not live in the RON"
    );
}

#[test]
fn editing_the_track_moves_both_the_render_and_the_measurement() {
    let kind = VehicleKind::T54_1951;
    let mut edited = pristine(kind);
    // A wheel a quarter bigger: far past any tolerance, impossible to miss visually.
    edited.track.wheel_radius = 0.505;

    let baseline = bake_studio_bundle_from_blueprint(&pristine(kind)).expect("baseline");
    let live = bake_studio_bundle_from_blueprint(&edited).expect("edited");

    // 1. The picture moved.
    assert_ne!(
        baseline.contact_sheet_png(),
        live.contact_sheet_png(),
        "a bigger road wheel must change the rendered gear"
    );

    // 2. The numbers moved, and moved to the EDITED value — not the embedded one.
    let pack = ReferencePack::for_vehicle(kind).expect("pack");
    let baked = authoritative_baked_vehicle(kind).expect("bake");
    let report = pack.measure_dimensions_live(&baked, &edited).expect("live dimensions");
    let wheel = report.measurement(DimensionKind::RoadWheelDiameter).expect("wheel anchor");
    assert!(
        (wheel.measured_m() - 1.01).abs() < 0.02,
        "the wheel anchor must measure the EDITED radius (got {:.3} m)",
        wheel.measured_m()
    );
    assert!(!wheel.passed(), "a 25% oversized wheel must fail its locked anchor");

    // 3. And the embedded path is untouched by the edit.
    let embedded = pack.measure_dimensions(&baked).expect("embedded dimensions");
    let embedded_wheel =
        embedded.measurement(DimensionKind::RoadWheelDiameter).expect("wheel anchor");
    assert!(embedded_wheel.passed(), "the embedded blueprint still measures its own 810 mm wheel");
}
