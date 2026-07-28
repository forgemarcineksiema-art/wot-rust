//! The absolute-dimension gate: every vehicle whose reference pack carries dimension anchors
//! (metres, sourced from its dossier) must bake within tolerance of the real tank's tape
//! measure. Ratios alone pass at the wrong scale — this is the gate that catches a whole
//! vehicle quietly growing or shrinking. `Locked` anchors assert; `Target` anchors are the
//! Model Idealny program's declared debt — measured and reported every run, asserted the day
//! their geometry PR flips them. Vehicles without authored anchors are skipped, so the bar
//! rises vehicle by vehicle as the fleet's dossiers land (masterplan W1/W2).

use game_core::VehicleKind;
use vehicle_forge::{
    AnchorStatus, DimensionKind, DimensionTarget, ReferencePack, ReferenceSource,
    authoritative_baked_vehicle,
};

#[test]
fn every_locked_dimension_anchor_holds_on_the_authoritative_bake() {
    let mut gated = 0;
    let mut debts = Vec::new();
    for kind in VehicleKind::PLAYABLE {
        let Some(pack) = ReferencePack::for_vehicle(kind) else { continue };
        if pack.dimensions().is_empty() {
            continue;
        }
        let baked = authoritative_baked_vehicle(kind).expect("authoritative bake");
        let report = pack.measure_dimensions(&baked).expect("dimension report");
        for measurement in report.measurements() {
            // A measurement that could not be produced is a broken instrument regardless of
            // anchor status — NaN must never let a Target row rot unmeasured.
            assert!(
                measurement.measured_m().is_finite(),
                "{kind:?}: {} could not be measured (instrument broken)",
                measurement.kind().label(),
            );
            match measurement.target().status() {
                AnchorStatus::Locked => assert!(
                    measurement.passed(),
                    "{kind:?}: {} measured {:.3} against LOCKED target {:.3} ±{:.3} \
                     (Δ {:+.3}, {:+.1}%) — source: {}",
                    measurement.kind().label(),
                    measurement.measured_m(),
                    measurement.target().target_m(),
                    measurement.target().tolerance_m(),
                    measurement.delta_m(),
                    measurement.percent_difference(),
                    measurement.target().source().label(),
                ),
                AnchorStatus::Target => {
                    if !measurement.passed() {
                        debts.push(format!(
                            "{kind:?}: {} {:.3} vs documented {:.3} (Δ {:+.3})",
                            measurement.kind().label(),
                            measurement.measured_m(),
                            measurement.target().target_m(),
                            measurement.delta_m(),
                        ));
                    }
                }
            }
        }
        gated += 1;
    }
    assert!(gated >= 1, "at least the T-54 pilot must be gated — the gate must never go silent");
    // Debt is visible, never fatal — the register in docs/model-idealny-t54.md owns the fixes.
    for line in &debts {
        println!("DIMENSION DEBT: {line}");
    }
}

#[test]
fn the_t54_pilot_reports_every_authored_anchor() {
    let pack = ReferencePack::for_vehicle(VehicleKind::T54_1951).expect("T-54 pack");
    assert!(pack.dimensions().len() >= 14, "the pilot anchors the full dossier table");
    let baked = authoritative_baked_vehicle(VehicleKind::T54_1951).expect("bake");
    let report = pack.measure_dimensions(&baked).expect("report");
    assert_eq!(report.measurements().len(), pack.dimensions().len());
    for kind in [
        DimensionKind::HullLength,
        DimensionKind::HullWidth,
        DimensionKind::HeightToTurretRoof,
        DimensionKind::HeightToTurretRoofBare,
        DimensionKind::OverallLengthWithGun,
        DimensionKind::RoadWheelDiameter,
        DimensionKind::TurretRingDiameter,
        DimensionKind::FireLineHeight,
        DimensionKind::CupolaDiameter,
        DimensionKind::TrackWidth,
        DimensionKind::TrackGauge,
        DimensionKind::GroundClearance,
        DimensionKind::TrackLinkCountPerSide,
        DimensionKind::RoadWheelCount,
    ] {
        assert!(report.measurement(kind).is_some(), "{kind:?} measured");
    }
    // Both tiers are present: the gate has teeth AND the program debt is on the record.
    assert!(
        report.measurements().iter().any(|m| m.target().status() == AnchorStatus::Locked),
        "at least one anchor gates"
    );
    assert!(
        report.measurements().iter().any(|m| m.target().status() == AnchorStatus::Target),
        "the declared program debt is measured every run"
    );
    // The markdown table carries the columns an author reads in the Studio report.
    let table = report.markdown_summary();
    for column in ["Measured", "Target", "Δ", "Δ%", "Tolerance", "Status", "Basis", "Source"] {
        assert!(table.contains(column), "summary must carry the {column} column");
    }
    assert!(table.contains("DIMENSION DEBT"), "outstanding Target anchors are summarized");
}

#[test]
fn t54_locked_anchors_hold_and_known_debts_are_the_registered_ones() {
    // The M-register in docs/model-idealny-t54.md names exactly which documented numbers the
    // model misses today. The gate must agree: those and ONLY those may be in debt.
    let pack = ReferencePack::for_vehicle(VehicleKind::T54_1951).expect("T-54 pack");
    let baked = authoritative_baked_vehicle(VehicleKind::T54_1951).expect("bake");
    let report = pack.measure_dimensions(&baked).expect("report");
    assert!(report.all_locked_pass(), "every Locked T-54 anchor holds");
    let mut debts: Vec<DimensionKind> =
        report.debts().map(vehicle_forge::MeasuredDimension::kind).collect();
    debts.sort_by_key(|kind| format!("{kind:?}"));
    let mut expected = vec![
        DimensionKind::CupolaDiameter,
        DimensionKind::GroundClearance,
        DimensionKind::HeightToTurretRoof,
        DimensionKind::HeightToTurretRoofBare,
        DimensionKind::HullLength,
        DimensionKind::TrackGauge,
        DimensionKind::TrackWidth,
    ];
    expected.sort_by_key(|kind| format!("{kind:?}"));
    assert_eq!(debts, expected, "debt list must match the M-register exactly");
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
