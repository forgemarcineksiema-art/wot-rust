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
        // CupolaDiameter left this list in PR-16: the drum is the documented 624 mm.
        DimensionKind::TrackWidth,
        DimensionKind::TrackGauge,
        DimensionKind::GroundClearance,
        DimensionKind::TrackLinkCountPerSide,
        DimensionKind::RoadWheelCount,
    ] {
        assert!(report.measurement(kind).is_some(), "{kind:?} measured");
    }
    // EVERY anchor gates. The Target tier is not gone from the mechanism — it is the FLOOR/TARGET
    // pattern the whole workshop is built on, and `a_target_anchor_reports_without_failing` below
    // still exercises it — but this vehicle has nothing left in it. That is what finishing looks
    // like: the pilot no longer needs the tier it was written to survive.
    assert!(
        report.measurements().iter().all(|m| m.target().status() == AnchorStatus::Locked),
        "the T-54 is fully Locked: {:?}",
        report
            .measurements()
            .iter()
            .filter(|m| m.target().status() != AnchorStatus::Locked)
            .map(vehicle_forge::MeasuredDimension::kind)
            .collect::<Vec<_>>()
    );
    // The markdown table carries the columns an author reads in the Studio report.
    let table = report.markdown_summary();
    for column in ["Measured", "Target", "Δ", "Δ%", "Tolerance", "Status", "Basis", "Source"] {
        assert!(table.contains(column), "summary must carry the {column} column");
    }
    assert!(
        !table.contains("DIMENSION DEBT"),
        "with no Target anchors left there is no debt section to print"
    );
}

/// The finish line the programme document names: "when both registers are empty and every
/// `Target` anchor has flipped to `Locked`, this document becomes history".
///
/// The list below is empty, and the comments are the record of how it emptied. Left as a list
/// rather than collapsed to `assert!(debts.is_empty())` on purpose — the next vehicle through
/// this workshop starts with a full one, and this is the shape it shrinks by.
#[test]
fn t54_locked_anchors_hold_and_known_debts_are_the_registered_ones() {
    let pack = ReferencePack::for_vehicle(VehicleKind::T54_1951).expect("T-54 pack");
    let baked = authoritative_baked_vehicle(VehicleKind::T54_1951).expect("bake");
    let report = pack.measure_dimensions(&baked).expect("report");
    assert!(report.all_locked_pass(), "every Locked T-54 anchor holds");
    let mut debts: Vec<DimensionKind> =
        report.debts().map(vehicle_forge::MeasuredDimension::kind).collect();
    debts.sort_by_key(|kind| format!("{kind:?}"));
    let mut expected: Vec<DimensionKind> = vec![
        // CupolaDiameter left this list in PR-16: the drum is the documented 624 mm across.
        // The two height anchors left this list in PR-15: the dome is built at its documented
        // 2.40 m roof and the cupola stands its documented 131 mm proud of it.
        // HullLength left this list in PR-14: the hull is built at its documented 6.235 m and
        // the anchor is Locked.
        // The two track anchors left this list in PR-18: the belt is the documented 580 mm on
        // the documented 2640 mm gauge, and the tub narrowed to the space that leaves.
        // GroundClearance left it in PR-20, and it never was a geometry debt: the belly has been
        // at the documented 0.425 since PR-14. What was wrong was the INSTRUMENT — it looked in a
        // strip 55% of the widest thing on the vehicle, and the widest thing on a T-54 is its
        // fenders, so the window was 0.90 wide while the floor's corners sit at 1.03. The floor
        // was never in it.
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
