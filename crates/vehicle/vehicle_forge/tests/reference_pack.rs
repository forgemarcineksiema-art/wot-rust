use game_core::VehicleKind;
use vehicle_forge::{
    BakeProfile, RatioKind, ReferencePack, bake_production_vehicle, t54_reference_pack,
};

#[test]
fn t54_reference_pack_locks_the_forge_benchmark_intent() {
    let pack = t54_reference_pack();

    assert_eq!(pack.family_slug(), "t54");
    assert_eq!(pack.display_name(), "T-54");
    assert_eq!(pack.vehicles(), &[VehicleKind::T54_1951]);
    assert!(pack.summary().contains("Armored Vehicle Forge benchmark"));
    assert!(pack.summary().contains("without bore evacuator"));
    assert_eq!(pack.road_wheel_count_per_side(), 5);

    let source_urls: Vec<&str> = pack.sources().iter().map(|source| source.url()).collect();
    assert!(
        source_urls.iter().any(|url| url.contains("commons.wikimedia.org/wiki/T-54/T-55")),
        "T-54 pack must cite a photo reference gallery"
    );
    assert!(
        source_urls.iter().any(|url| url.contains("tanks-encyclopedia.com")),
        "T-54 pack must cite a technical/reference article"
    );

    let hull_ratio = pack.ratio(RatioKind::HullLengthToWidth).expect("hull ratio target");
    assert_eq!(hull_ratio.kind(), RatioKind::HullLengthToWidth);
    assert!(hull_ratio.target() > 1.6 && hull_ratio.target() < 2.0);
    assert!(hull_ratio.tolerance() <= 0.20);
}

#[test]
fn t54_reference_pack_is_discoverable_by_vehicle_kind() {
    let t54 = ReferencePack::for_vehicle(VehicleKind::T54_1951).expect("T-54 pack");

    assert_eq!(t54.family_slug(), "t54");
    assert_eq!(t54.road_wheel_count_per_side(), 5);
    // The German heavies are migrated too, each with their own family pack.
    assert_eq!(ReferencePack::for_vehicle(VehicleKind::TigerI).unwrap().family_slug(), "tiger_i");
    assert_eq!(
        ReferencePack::for_vehicle(VehicleKind::Jagdtiger).unwrap().family_slug(),
        "jagdtiger"
    );
}

#[test]
fn every_migrated_family_pack_passes_its_own_silhouette_ratios() {
    for kind in [
        VehicleKind::T54_1951,
        VehicleKind::TigerI,
        VehicleKind::TigerII,
        VehicleKind::Jagdtiger,
        VehicleKind::PantherII,
    ] {
        let pack = ReferencePack::for_vehicle(kind).expect("migrated vehicle has a pack");
        let vehicle = bake_production_vehicle(kind, BakeProfile::Lod0).expect("vehicle bakes");
        let report = pack.measure_baked_vehicle(&vehicle).expect("ratio report");
        assert!(
            report.all_pass(),
            "{kind:?} fails its own reference ratios:\n{}",
            report.markdown_summary()
        );
    }
}

#[test]
fn t54_baked_geometry_produces_a_ratio_report_against_reference_targets() {
    let pack = ReferencePack::for_vehicle(VehicleKind::T54_1951).expect("T-54 pack");
    let vehicle = bake_production_vehicle(VehicleKind::T54_1951, BakeProfile::Lod0)
        .expect("T-54 should bake");
    let report = pack.measure_baked_vehicle(&vehicle).expect("ratio report");

    assert_eq!(report.vehicle(), VehicleKind::T54_1951);
    assert!(report.passes(RatioKind::HullLengthToWidth).expect("hull ratio measured"));
    assert!(report.passes(RatioKind::TurretWidthToHullWidth).expect("turret ratio measured"));
    assert!(report.passes(RatioKind::GunProtrusionToHullLength).expect("gun ratio measured"));

    // The enriched ratio set covers hull plan + height, turret width + height, and gun reach.
    assert!(report.passes(RatioKind::HullHeightToLength).expect("hull height ratio measured"));
    assert!(report.passes(RatioKind::TurretHeightToHullHeight).expect("turret height measured"));

    let lines = report.markdown_summary();
    assert!(lines.contains("T-54 Forge reference report"));
    assert!(lines.contains("HullLengthToWidth"));
    assert!(lines.contains("HullHeightToLength"));
    assert!(lines.contains("TurretHeightToHullHeight"));
    assert!(lines.contains("GunProtrusionToHullLength"));
    // Percentage differences are reported, not just pass/fail (Milestone 1 acceptance).
    assert!(lines.contains("Δ%"), "report must carry a percentage-difference column");
    assert!(lines.contains('%'));
}

#[test]
fn measured_ratio_reports_signed_percentage_difference() {
    let pack = ReferencePack::for_vehicle(VehicleKind::T54_1951).expect("T-54 pack");
    let vehicle = bake_production_vehicle(VehicleKind::T54_1951, BakeProfile::Lod0)
        .expect("T-54 should bake");
    let report = pack.measure_baked_vehicle(&vehicle).expect("ratio report");

    let hull = report.measurement(RatioKind::HullLengthToWidth).expect("hull measurement");
    let expected = (hull.measured() - hull.target().target()) / hull.target().target() * 100.0;
    assert!((hull.percent_difference() - expected).abs() < 1.0e-3);
    // A passing ratio sits within tolerance, so its percentage delta is small.
    assert!(hull.percent_difference().abs() < 15.0);
}
