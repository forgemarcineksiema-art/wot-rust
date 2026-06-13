use game_core::VehicleKind;
use vehicle_forge::{RatioKind, ReferencePack, t54_t55_reference_pack};
use vehicle_geometry::bake_vehicle;

#[test]
fn t54_t55_reference_pack_locks_the_forge_benchmark_intent() {
    let pack = t54_t55_reference_pack();

    assert_eq!(pack.family_slug(), "t54_t55");
    assert!(pack.summary().contains("Armored Vehicle Forge benchmark"));
    assert_eq!(pack.road_wheel_count_per_side(), 5);

    let source_urls: Vec<&str> = pack.sources().iter().map(|source| source.url()).collect();
    assert!(
        source_urls.iter().any(|url| url.contains("commons.wikimedia.org/wiki/T-54/T-55")),
        "T-54/T-55 pack must cite a photo reference gallery"
    );
    assert!(
        source_urls.iter().any(|url| url.contains("tank-afv.com")),
        "T-54/T-55 pack must cite a technical/reference article"
    );

    let hull_ratio = pack.ratio(RatioKind::HullLengthToWidth).expect("hull ratio target");
    assert_eq!(hull_ratio.kind(), RatioKind::HullLengthToWidth);
    assert!(hull_ratio.target() > 1.6 && hull_ratio.target() < 2.0);
    assert!(hull_ratio.tolerance() <= 0.20);
}

#[test]
fn t54_reference_pack_is_discoverable_by_vehicle_kind() {
    let t54 = ReferencePack::for_vehicle(VehicleKind::T54_1951).expect("T-54 pack");
    let t55a = ReferencePack::for_vehicle(VehicleKind::T55A).expect("T-55A pack");

    assert_eq!(t54.family_slug(), "t54_t55");
    assert_eq!(t55a.family_slug(), "t54_t55");
    assert_eq!(t54.road_wheel_count_per_side(), 5);
    assert_eq!(t55a.road_wheel_count_per_side(), 5);
    assert!(ReferencePack::for_vehicle(VehicleKind::TigerI).is_none());
}

#[test]
fn t54_baked_geometry_produces_a_ratio_report_against_reference_targets() {
    let pack = ReferencePack::for_vehicle(VehicleKind::T54_1951).expect("T-54 pack");
    let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 should bake");
    let report = pack.measure_baked_vehicle(&vehicle).expect("ratio report");

    assert_eq!(report.vehicle(), VehicleKind::T54_1951);
    assert!(report.passes(RatioKind::HullLengthToWidth).expect("hull ratio measured"));
    assert!(report.passes(RatioKind::TurretWidthToHullWidth).expect("turret ratio measured"));
    assert!(report.passes(RatioKind::GunProtrusionToHullLength).expect("gun ratio measured"));

    let lines = report.markdown_summary();
    assert!(lines.contains("T-54/T-55 Forge reference report"));
    assert!(lines.contains("HullLengthToWidth"));
    assert!(lines.contains("GunProtrusionToHullLength"));
}

#[test]
fn t55a_baked_geometry_passes_the_family_reference_ratios() {
    let pack = ReferencePack::for_vehicle(VehicleKind::T55A).expect("T-55A pack");
    let vehicle = bake_vehicle(VehicleKind::T55A).expect("T-55A should bake");
    let report = pack.measure_baked_vehicle(&vehicle).expect("ratio report");

    assert_eq!(report.vehicle(), VehicleKind::T55A);
    assert!(report.all_pass(), "T-55A is now blueprint-backed and must read in family proportion");
}
