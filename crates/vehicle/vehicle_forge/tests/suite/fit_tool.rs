//! The fit instrument's own locks (Forge 2.0 acceleration, step 3): a fit never lowers the mean
//! K0 score, never breaks a locked anchor, and its RON rewrite touches only the moved lines.

use game_core::{VehicleBlueprint, VehicleKind};
use vehicle_forge::fit::{
    FieldChange, FitReport, ViewScore, apply_to_ron, default_fields, fit_blueprint, get_field,
    set_field,
};

/// Every default field reads back what it writes, on every playable vehicle's blueprint.
#[test]
fn every_fit_field_round_trips_through_the_blueprint() {
    let mut checked = 0;
    for kind in VehicleKind::PLAYABLE {
        let Some(bp) = VehicleBlueprint::for_vehicle(kind) else { continue };
        for field in default_fields(&bp) {
            checked += 1;
            assert!(field.lo < field.hi, "{kind:?} {}: bounds are a span", field.name);
            let mut probe = bp;
            let value = (field.lo + field.hi) * 0.5;
            set_field(&mut probe, field.name, value);
            assert!(
                (get_field(&probe, field.name) - value).abs() < 1.0e-6,
                "{kind:?} {}: set then get",
                field.name
            );
        }
    }
    assert!(checked >= 8 * 12, "the fleet's fields were walked: {checked}");
}

/// A short fit on the Tiger I (the traced vehicle) never lowers the mean score and keeps every
/// view at or above its floor — the fences hold and the objective is monotone.
#[test]
fn a_fit_never_lowers_the_mean_score_or_breaks_a_floor() {
    let report = fit_blueprint(VehicleKind::TigerI, 1).expect("the Tiger is traced");
    assert!(report.mean_after() >= report.mean_before() - 1.0e-6, "{}", report.summary());
    for view in &report.views {
        if let Some(floor) = view.floor {
            assert!(view.after >= floor, "{}: {} under its floor", view.view, view.after);
        }
    }
    assert!(report.evaluations >= 2, "the fit baked candidates: {}", report.evaluations);
}

/// The RON rewrite changes exactly the moved lines and nothing else, and refuses an ambiguous
/// field.
#[test]
fn the_ron_rewrite_touches_only_the_moved_lines() {
    let text = "BlueprintFile(\n    hull: HullShape(\n        half_len: 3.16,\n        deck_y: 1.778,\n    ),\n    turret: TurretShape(\n        roof_y: 2.60,\n        cupola_height: Some(0.26),\n    ),\n)\n";
    let mut blueprint = VehicleBlueprint::for_vehicle(VehicleKind::TigerI).expect("Tiger");
    blueprint.turret.roof_y = 2.65;
    let report = FitReport {
        kind: VehicleKind::TigerI,
        evaluations: 0,
        views: vec![ViewScore { view: "side".into(), before: 0.5, after: 0.6, floor: None }],
        changes: vec![
            FieldChange { name: "turret.roof_y", before: 2.60, after: 2.65 },
            FieldChange { name: "turret.cupola_height", before: 0.26, after: 0.25 },
        ],
        blueprint,
    };
    let out = apply_to_ron(text, &report).expect("rewrites");
    assert!(out.contains("        roof_y: 2.65,\n"), "{out}");
    assert!(out.contains("        cupola_height: Some(0.25),\n"), "{out}");
    assert!(out.contains("        half_len: 3.16,\n"), "untouched lines stay: {out}");
    assert_eq!(out.lines().count(), text.lines().count());
    let ambiguous = "a(\n roof_y: 1.0,\n roof_y: 2.0,\n)\n";
    assert!(apply_to_ron(ambiguous, &report).is_err(), "an ambiguous field is refused");
}
