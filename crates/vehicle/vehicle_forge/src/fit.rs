//! `tools fit --vehicle <slug>` (Forge 2.0 acceleration, step 3): the machine does what the
//! K3 data passes did by hand — it moves a vehicle's shape numbers until the bake's three
//! silhouettes agree with the traced drawings, while every LOCKED dimension anchor keeps
//! holding and the blueprint lint stays clean.
//!
//! The objective is the mean K0 IoU over the vehicle's traced views (`outlines/<slug>.outline.ron`)
//! of the SHIPPED composition baked from a candidate blueprint (`describe_with_blueprint`, so
//! the library's parts ride the candidate too). The search is coordinate descent with a
//! shrinking step over a curated set of shape fields — the ones the drawings decide and the
//! anchors do not already pin. A candidate that breaks a locked anchor or the lint is
//! rejected, whatever its IoU. The result is a report (before/after per view, every moved
//! field with its old and new value); `--write` puts the new numbers into the blueprint RON
//! in place, one `field: value,` line each, and nothing else.
//!
//! What the first run taught (the Tiger I, 2026-09-05: mean 0.920 → 0.926, side 0.891 → 0.905):
//! the fit fills whatever room the anchors LEAVE. A traced outline carries a percent or two of
//! error, and an anchor at ±0.05 lets the fit push a deck 5 cm past the sheet's own dimension
//! line. So: where the drawing carries a dimension line, the anchor is tight (±0.01–0.02) and
//! the fit cannot move it; the fit decides the numbers the drawing does not dimension. Never
//! `--write` a report without reading it against the dossier's dimension table.

use game_core::{VehicleBlueprint, VehicleKind, lint};
use vehicle_geometry::RunningGearKinematics;

use crate::outline::{composed_triangles, measure};
use crate::{AnchorStatus, ReferencePack};

/// A shape field the fit may move, with its bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitField {
    pub name: &'static str,
    pub lo: f32,
    pub hi: f32,
}

/// One moved field in the report.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldChange {
    pub name: &'static str,
    pub before: f32,
    pub after: f32,
}

/// Per-view IoU before and after.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewScore {
    pub view: String,
    pub before: f32,
    pub after: f32,
    pub floor: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FitReport {
    pub kind: VehicleKind,
    pub evaluations: usize,
    pub views: Vec<ViewScore>,
    pub changes: Vec<FieldChange>,
    pub blueprint: VehicleBlueprint,
}

impl FitReport {
    pub fn mean_before(&self) -> f32 {
        mean(self.views.iter().map(|v| v.before))
    }

    pub fn mean_after(&self) -> f32 {
        mean(self.views.iter().map(|v| v.after))
    }

    pub fn summary(&self) -> String {
        let mut out = format!(
            "{}: mean IoU {:.3} -> {:.3} over {} views, {} bakes\n",
            self.kind.slug(),
            self.mean_before(),
            self.mean_after(),
            self.views.len(),
            self.evaluations
        );
        for view in &self.views {
            out.push_str(&format!(
                "  {:<6} {:.3} -> {:.3}{}\n",
                view.view,
                view.before,
                view.after,
                view.floor.map(|f| format!(" (floor {f:.2})")).unwrap_or_default()
            ));
        }
        if self.changes.is_empty() {
            out.push_str("  no field moved\n");
        }
        for change in &self.changes {
            out.push_str(&format!(
                "  {:<20} {:>8.4} -> {:>8.4}\n",
                change.name, change.before, change.after
            ));
        }
        out
    }
}

fn mean(values: impl Iterator<Item = f32>) -> f32 {
    let (sum, n) = values.fold((0.0_f32, 0usize), |(s, n), v| (s + v, n + 1));
    if n == 0 { 0.0 } else { sum / n as f32 }
}

/// The fields the drawings decide. Bounds are generous but physical; the anchors and the lint
/// do the real fencing.
pub fn default_fields(bp: &VehicleBlueprint) -> Vec<FitField> {
    let around = |v: f32, span: f32| FitField { name: "", lo: v - span, hi: v + span };
    let mut fields = vec![
        FitField { name: "hull.deck_y", ..around(bp.hull.deck_y, 0.25) },
        FitField { name: "hull.sponson_y", ..around(bp.hull.sponson_y, 0.30) },
        FitField { name: "hull.half_width", ..around(bp.hull.half_width, 0.30) },
        FitField { name: "hull.glacis_slope_deg", ..around(bp.hull.glacis_slope_deg, 15.0) },
        FitField { name: "hull.nose_rise", ..around(bp.hull.nose_rise, 0.30) },
        FitField { name: "turret.roof_y", ..around(bp.turret.roof_y, 0.30) },
        FitField { name: "turret.ring_z", ..around(bp.turret.ring_z, 0.50) },
        FitField { name: "turret.plan_half_width", ..around(bp.turret.plan_half_width, 0.30) },
        FitField { name: "turret.plan_half_length", ..around(bp.turret.plan_half_length, 0.40) },
        FitField { name: "turret.plan_front_pad", ..around(bp.turret.plan_front_pad, 0.30) },
        FitField { name: "turret.front_slope_deg", ..around(bp.turret.front_slope_deg, 15.0) },
        FitField { name: "gun.trunnion_y", ..around(bp.gun.trunnion_y, 0.20) },
    ];
    if bp.turret.cupola_height.is_some() {
        fields.push(FitField {
            name: "turret.cupola_height",
            ..around(bp.turret.cupola_height.unwrap_or(0.0), 0.20)
        });
    }
    if bp.armor.hull_bow_shelf.is_some() {
        let (top, setback) = bp.armor.hull_bow_shelf.unwrap_or((0.0, 0.0));
        fields.push(FitField { name: "armor.hull_bow_shelf.top", ..around(top, 0.20) });
        fields.push(FitField { name: "armor.hull_bow_shelf.setback", ..around(setback, 0.40) });
    }
    for field in &mut fields {
        // Never below zero for the lengths and heights; the slopes may swing either way.
        if !field.name.ends_with("_deg") && field.name != "turret.ring_z" {
            field.lo = field.lo.max(0.01);
        }
    }
    fields
}

pub fn get_field(bp: &VehicleBlueprint, name: &str) -> f32 {
    match name {
        "hull.deck_y" => bp.hull.deck_y,
        "hull.sponson_y" => bp.hull.sponson_y,
        "hull.half_width" => bp.hull.half_width,
        "hull.glacis_slope_deg" => bp.hull.glacis_slope_deg,
        "hull.nose_rise" => bp.hull.nose_rise,
        "turret.roof_y" => bp.turret.roof_y,
        "turret.ring_z" => bp.turret.ring_z,
        "turret.plan_half_width" => bp.turret.plan_half_width,
        "turret.plan_half_length" => bp.turret.plan_half_length,
        "turret.plan_front_pad" => bp.turret.plan_front_pad,
        "turret.front_slope_deg" => bp.turret.front_slope_deg,
        "turret.cupola_height" => bp.turret.cupola_height.unwrap_or(0.0),
        "gun.trunnion_y" => bp.gun.trunnion_y,
        "armor.hull_bow_shelf.top" => bp.armor.hull_bow_shelf.map(|(t, _)| t).unwrap_or(0.0),
        "armor.hull_bow_shelf.setback" => bp.armor.hull_bow_shelf.map(|(_, s)| s).unwrap_or(0.0),
        other => panic!("fit: unknown field {other}"),
    }
}

pub fn set_field(bp: &mut VehicleBlueprint, name: &str, value: f32) {
    match name {
        "hull.deck_y" => bp.hull.deck_y = value,
        "hull.sponson_y" => bp.hull.sponson_y = value,
        "hull.half_width" => bp.hull.half_width = value,
        "hull.glacis_slope_deg" => bp.hull.glacis_slope_deg = value,
        "hull.nose_rise" => bp.hull.nose_rise = value,
        "turret.roof_y" => bp.turret.roof_y = value,
        "turret.ring_z" => bp.turret.ring_z = value,
        "turret.plan_half_width" => bp.turret.plan_half_width = value,
        "turret.plan_half_length" => bp.turret.plan_half_length = value,
        "turret.plan_front_pad" => bp.turret.plan_front_pad = value,
        "turret.front_slope_deg" => bp.turret.front_slope_deg = value,
        "turret.cupola_height" => bp.turret.cupola_height = Some(value),
        "gun.trunnion_y" => bp.gun.trunnion_y = value,
        "armor.hull_bow_shelf.top" => {
            let setback = bp.armor.hull_bow_shelf.map(|(_, s)| s).unwrap_or(0.0);
            bp.armor.hull_bow_shelf = Some((value, setback));
        }
        "armor.hull_bow_shelf.setback" => {
            let top = bp.armor.hull_bow_shelf.map(|(t, _)| t).unwrap_or(0.0);
            bp.armor.hull_bow_shelf = Some((top, value));
        }
        other => panic!("fit: unknown field {other}"),
    }
}

/// The RON field name of a fit field, and how a value is written there.
pub fn ron_field(name: &str) -> Option<&'static str> {
    Some(match name {
        "hull.deck_y" => "deck_y",
        "hull.sponson_y" => "sponson_y",
        "hull.half_width" => "half_width",
        "hull.glacis_slope_deg" => "glacis_slope_deg",
        "hull.nose_rise" => "nose_rise",
        "turret.roof_y" => "roof_y",
        "turret.ring_z" => "ring_z",
        "turret.plan_half_width" => "plan_half_width",
        "turret.plan_half_length" => "plan_half_length",
        "turret.plan_front_pad" => "plan_front_pad",
        "turret.front_slope_deg" => "front_slope_deg",
        "turret.cupola_height" => "cupola_height",
        "gun.trunnion_y" => "trunnion_y",
        _ => return None,
    })
}

/// What the fit scores: the candidate's shipped bake against every traced view, or `None`
/// when the candidate breaks the lint or a locked anchor (the fences the drawing may not
/// push through).
fn score(kind: VehicleKind, pack: &ReferencePack, bp: &VehicleBlueprint) -> Option<Vec<f32>> {
    if lint::validate_blueprint(bp).iter().any(|issue| issue.severity == lint::Severity::Error) {
        return None;
    }
    let description = vehicle_recipes::describe_with_blueprint(bp)?;
    let baked = description.build();
    let report = pack.measure_dimensions_live(&baked, bp)?;
    if report
        .measurements()
        .iter()
        .any(|m| m.target().status() == AnchorStatus::Locked && !m.passed())
    {
        return None;
    }
    let kin = RunningGearKinematics::from_track(&bp.track);
    let tris = composed_triangles(&baked, Some(&kin));
    let _ = kind;
    Some(pack.outlines().iter().map(|spec| measure(&tris, spec).iou()).collect())
}

/// Fit `kind`'s blueprint to its traced outlines. `rounds` coordinate-descent sweeps; the step
/// starts at a quarter of each field's span and halves whenever a sweep moves nothing.
pub fn fit_blueprint(kind: VehicleKind, rounds: usize) -> Option<FitReport> {
    let pack = ReferencePack::for_vehicle(kind)?;
    if pack.outlines().is_empty() {
        return None;
    }
    let start = VehicleBlueprint::for_vehicle(kind)?;
    let fields = default_fields(&start);
    let mut evaluations = 0usize;
    let mut best = start;
    let before = score(kind, &pack, &best)?;
    evaluations += 1;
    let mut best_mean = mean(before.iter().copied());
    let mut steps: Vec<f32> = fields.iter().map(|f| (f.hi - f.lo) * 0.25).collect();
    for _round in 0..rounds {
        let mut moved = false;
        for (i, field) in fields.iter().enumerate() {
            let current = get_field(&best, field.name);
            for direction in [1.0_f32, -1.0] {
                let candidate_value = (current + direction * steps[i]).clamp(field.lo, field.hi);
                if (candidate_value - current).abs() < 1.0e-4 {
                    continue;
                }
                let mut candidate = best;
                set_field(&mut candidate, field.name, candidate_value);
                evaluations += 1;
                let Some(scores) = score(kind, &pack, &candidate) else { continue };
                let m = mean(scores.iter().copied());
                if m > best_mean + 1.0e-4 {
                    best = candidate;
                    best_mean = m;
                    moved = true;
                    break;
                }
            }
        }
        if !moved {
            for step in &mut steps {
                *step *= 0.5;
            }
            if steps.iter().all(|s| *s < 0.002) {
                break;
            }
        }
    }
    let after = score(kind, &pack, &best)?;
    let views = pack
        .outlines()
        .iter()
        .zip(before.iter().zip(after.iter()))
        .map(|(spec, (b, a))| ViewScore {
            view: spec.view().label().to_string(),
            before: *b,
            after: *a,
            floor: spec.floor_iou(),
        })
        .collect();
    let changes = fields
        .iter()
        .filter_map(|field| {
            let (b, a) = (get_field(&start, field.name), get_field(&best, field.name));
            ((a - b).abs() > 1.0e-4).then_some(FieldChange {
                name: field.name,
                before: b,
                after: a,
            })
        })
        .collect();
    Some(FitReport { kind, evaluations, views, changes, blueprint: best })
}

/// Rewrite the moved fields in a blueprint RON text in place: each `field: value,` line once,
/// the rest of the file untouched. The shelf tuple is rewritten whole.
pub fn apply_to_ron(text: &str, report: &FitReport) -> Result<String, String> {
    let mut out = text.to_string();
    let mut shelf: Option<(f32, f32)> = None;
    for change in &report.changes {
        if change.name.starts_with("armor.hull_bow_shelf") {
            shelf = report.blueprint.armor.hull_bow_shelf;
            continue;
        }
        let Some(field) = ron_field(change.name) else { continue };
        let value = if change.name == "turret.cupola_height" {
            format!("Some({})", trim(change.after))
        } else {
            trim(change.after)
        };
        out = replace_field(&out, field, &value)?;
    }
    if let Some((top, setback)) = shelf {
        out = replace_field(
            &out,
            "hull_bow_shelf",
            &format!("Some(({}, {}))", trim(top), trim(setback)),
        )?;
    }
    Ok(out)
}

fn trim(value: f32) -> String {
    let s = format!("{value:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.contains('.') { s } else { format!("{s}.0") }
}

fn replace_field(text: &str, field: &str, value: &str) -> Result<String, String> {
    let mut hits = 0;
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix(field)
            && rest.starts_with(':')
        {
            hits += 1;
            let indent = &line[..line.len() - trimmed.len()];
            out.push_str(&format!("{indent}{field}: {value},\n"));
        } else {
            out.push_str(line);
        }
    }
    if hits != 1 {
        return Err(format!("`{field}` appears {hits} times in the blueprint RON, expected once"));
    }
    Ok(out)
}
