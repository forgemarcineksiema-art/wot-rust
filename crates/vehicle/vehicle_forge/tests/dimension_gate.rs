//! The absolute-dimension gate: every vehicle whose reference pack carries dimension anchors
//! (metres, sourced from its dossier) must bake within tolerance of the real tank's tape
//! measure. Ratios alone pass at the wrong scale — this is the gate that catches a whole
//! vehicle quietly growing or shrinking. Vehicles without authored anchors are skipped, so the
//! bar rises vehicle by vehicle as the fleet's dossiers land (masterplan W1/W2).

use game_core::VehicleKind;
use vehicle_forge::{
    DimensionKind, DimensionTarget, ReferencePack, ReferenceSource, authoritative_baked_vehicle,
};

#[test]
fn every_authored_dimension_anchor_holds_on_the_authoritative_bake() {
    let mut gated = 0;
    for kind in VehicleKind::PLAYABLE {
        let Some(pack) = ReferencePack::for_vehicle(kind) else { continue };
        if pack.dimensions().is_empty() {
            continue;
        }
        let baked = authoritative_baked_vehicle(kind).expect("authoritative bake");
        let report = pack.measure_dimensions(&baked).expect("dimension report");
        for measurement in report.measurements() {
            assert!(
                measurement.passed(),
                "{kind:?}: {} measured {:.3} m against target {:.3} m ±{:.3} (Δ {:+.3} m, {:+.1}%) — \
                 source: {}",
                measurement.kind().label(),
                measurement.measured_m(),
                measurement.target().target_m(),
                measurement.target().tolerance_m(),
                measurement.delta_m(),
                measurement.percent_difference(),
                measurement.target().source().label(),
            );
        }
        gated += 1;
    }
    assert!(gated >= 1, "at least the T-54 pilot must be gated — the gate must never go silent");
}

#[test]
fn the_t54_pilot_reports_every_authored_anchor() {
    let pack = ReferencePack::for_vehicle(VehicleKind::T54_1951).expect("T-54 pack");
    assert!(pack.dimensions().len() >= 5, "the pilot anchors the five headline dimensions");
    let baked = authoritative_baked_vehicle(VehicleKind::T54_1951).expect("bake");
    let report = pack.measure_dimensions(&baked).expect("report");
    assert_eq!(report.measurements().len(), pack.dimensions().len());
    for kind in [
        DimensionKind::HullLength,
        DimensionKind::HullWidth,
        DimensionKind::HeightToTurretRoof,
        DimensionKind::OverallLengthWithGun,
        DimensionKind::RoadWheelDiameter,
    ] {
        assert!(report.measurement(kind).is_some(), "{kind:?} measured");
    }
    // The markdown table carries the columns an author reads in the Studio report.
    let table = report.markdown_summary();
    for column in ["Measured m", "Target m", "Δ m", "Δ%", "Tolerance", "Source"] {
        assert!(table.contains(column), "summary must carry the {column} column");
    }
}

#[test]
fn a_wrong_scale_fails_the_gate_where_ratios_stay_silent() {
    // The reason this gate exists: scale every dimension by 5% and each anchor must fail,
    // even though every RATIO of the same mesh would be unchanged.
    let source = ReferenceSource::new("test", "n/a", "synthetic");
    let target = DimensionTarget::new(DimensionKind::HullLength, 6.04, 0.15, source);
    let scaled = 6.04 * 1.05;
    assert!(
        !vehicle_forge::MeasuredDimension::new(target, scaled).passed(),
        "a 5% scale error must fail an absolute anchor"
    );
}
