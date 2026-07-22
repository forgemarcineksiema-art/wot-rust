//! The stroke tool (Rece do terenu W2): draw a terrain line instead of parametrizing one.
//! Clicked waypoints become a FITTED curve - deduped, Chaikin-smoothed, arc-length
//! resampled and quantized - committed as `TerrainOp::Stroke` through one `apply_edit`.
//! The document stores the fit, never the raw gesture (the stamp philosophy: quantized
//! data with a readable diff), and on a fair map the twin op carries the REFLECTED fitted
//! points, so the mirror stays bit-exact by construction.

use map_forge::blueprint::{MapBlueprint, StrokeProfile, StrokeSpec, TerrainOp};
use renderer_api::SceneVertex;
use terrain::HeightMap;

use crate::stamp::{Parameter, quantize_m};

/// The profile the C key cycles: what the drawn line does to the ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrokeKind {
    Ridge,
    Valley,
    Plateau,
}

impl StrokeKind {
    pub const CYCLE: [StrokeKind; 3] = [StrokeKind::Ridge, StrokeKind::Valley, StrokeKind::Plateau];

    pub fn label(self) -> &'static str {
        match self {
            StrokeKind::Ridge => "ridge",
            StrokeKind::Valley => "valley",
            StrokeKind::Plateau => "plateau",
        }
    }

    /// The one tunable knob per profile. The plateau's target defaults from the first
    /// click's ground - the bench starts where the hand pointed.
    fn parameter(self) -> Parameter {
        match self {
            StrokeKind::Ridge => {
                Parameter { name: "height", value_m: 4.0, step_m: 0.25, range: (0.5, 20.0) }
            }
            StrokeKind::Valley => {
                Parameter { name: "depth", value_m: 4.0, step_m: 0.25, range: (0.5, 20.0) }
            }
            StrokeKind::Plateau => {
                Parameter { name: "target", value_m: 10.0, step_m: 0.25, range: (0.25, 120.0) }
            }
        }
    }
}

/// The pending stroke: raw clicked waypoints plus its knobs. Raw points live only in the
/// session - the commit stores the fit.
#[derive(Debug, Clone)]
pub struct PendingStroke {
    pub raw: Vec<[f32; 2]>,
    pub kind: StrokeKind,
    pub parameter: Parameter,
    pub half_width_m: f32,
}

impl PendingStroke {
    pub fn new(kind: StrokeKind) -> Self {
        Self { raw: Vec::new(), kind, parameter: kind.parameter(), half_width_m: 6.0 }
    }

    /// Append a waypoint (metre-snapped, like roads). The plateau's first click also
    /// seats its target at that ground height - drawn, not typed.
    pub fn add_point(&mut self, at: [f32; 2], ground_m: f32) {
        if self.raw.is_empty() && self.kind == StrokeKind::Plateau {
            self.parameter.value_m = quantize_m(ground_m, 0.25);
        }
        self.raw.push([at[0].round(), at[1].round()]);
    }

    pub fn pop_point(&mut self) {
        self.raw.pop();
    }

    pub fn adjust_width(&mut self, factor: f32) {
        self.half_width_m = quantize_m((self.half_width_m * factor).clamp(2.0, 40.0), 0.5);
    }
}

/// Fit the gesture into committable op(s) plus a status summary: dedupe -> Chaikin x2 ->
/// arc-length resample -> 0.5 m quantize -> twin on fair maps. `None` under two distinct
/// points - a stroke is a line, not a dot. Pure and deterministic (test-locked): the same
/// clicks always fit the same curve.
pub fn fit_stroke(
    pending: &PendingStroke,
    blueprint: &MapBlueprint,
) -> Option<(Vec<TerrainOp>, String)> {
    let mut distinct: Vec<[f32; 2]> = Vec::new();
    for point in &pending.raw {
        if distinct.last().is_none_or(|last| segment_length(*last, *point) >= 1.0) {
            distinct.push(*point);
        }
    }
    if distinct.len() < 2 {
        return None;
    }
    let smoothed = chaikin(&chaikin(&distinct));
    let mut points = resample(&smoothed);
    for point in &mut points {
        point[0] = quantize_m(point[0], 0.5);
        point[1] = quantize_m(point[1], 0.5);
    }
    points.dedup();

    let half_width_m = quantize_m(pending.half_width_m, 0.5);
    let falloff_m = half_width_m;
    let value = quantize_m(pending.parameter.value_m, 0.25);
    let profile = match pending.kind {
        StrokeKind::Ridge => StrokeProfile::Ridge { amp_m: value },
        StrokeKind::Valley => StrokeProfile::Valley { depth_m: value },
        StrokeKind::Plateau => StrokeProfile::Plateau { target_m: value },
    };
    let length: f32 = points.windows(2).map(|pair| segment_length(pair[0], pair[1])).sum::<f32>();
    let summary = format!(
        "{} stroke: {} points, {length:.0} m, {} {value:.1} m, w {half_width_m:.1} m",
        pending.kind.label(),
        points.len(),
        pending.parameter.name,
    );

    let mut ops = vec![TerrainOp::Stroke(StrokeSpec {
        points: points.clone(),
        profile,
        half_width_m,
        falloff_m,
    })];
    // The fair-map twin carries the REFLECTED fitted points (never a refit): on the 0.5 m
    // lattice the reflection is bit-exact, so check_symmetry can never fire on a gesture.
    if blueprint.symmetry.is_some() {
        let axis_z = blueprint.grid.axis_z();
        let off_axis = points.iter().any(|[_, z]| (z - axis_z).abs() > blueprint.grid.cell_m * 0.5);
        if off_axis {
            ops.push(TerrainOp::Stroke(StrokeSpec {
                points: points.iter().map(|[x, z]| [*x, axis_z * 2.0 - z]).collect(),
                profile,
                half_width_m,
                falloff_m,
            }));
        }
    }
    Some((ops, summary))
}

/// One Chaikin corner-cut over an open polyline: endpoints stay, every segment yields its
/// quarter points - the classic subdivision that rounds a click path into a drawn line.
fn chaikin(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity(points.len() * 2);
    out.push(points[0]);
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        out.push([a[0] * 0.75 + b[0] * 0.25, a[1] * 0.75 + b[1] * 0.25]);
        out.push([a[0] * 0.25 + b[0] * 0.75, a[1] * 0.25 + b[1] * 0.75]);
    }
    out.push(points[points.len() - 1]);
    out
}

/// Arc-length resample at ~4 m steps, stretching the interval past the 64-point budget so
/// a map-crossing stroke still fits the report contract; the last point is always kept.
fn resample(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let total: f32 = points.windows(2).map(|pair| segment_length(pair[0], pair[1])).sum();
    if total <= f32::EPSILON {
        return vec![points[0], points[points.len() - 1]];
    }
    let interval = (total / 63.0).max(4.0);
    let mut out = vec![points[0]];
    let mut next_at = interval;
    let mut walked = 0.0_f32;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = segment_length(a, b);
        while next_at <= walked + length && length > f32::EPSILON {
            let t = (next_at - walked) / length;
            out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            next_at += interval;
        }
        walked += length;
    }
    // The hand's endpoint always survives the resample — appended when a full metre of
    // line remains (the report's wiggle floor), fused into the last sample otherwise.
    if segment_length(*out.last().expect("seeded"), points[points.len() - 1]) > 1.0 {
        out.push(points[points.len() - 1]);
    } else {
        *out.last_mut().expect("seeded") = points[points.len() - 1];
    }
    out
}

fn segment_length(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
}

/// The surveyor's chalk for a pending stroke: the raw polyline draped as a band-wide
/// ribbon plus a pad on every waypoint (the roads ribbon's sibling, in the stroke's own
/// chalk tone). The REAL ground arrives with the ghost preview and the commit.
pub fn chalk_mesh(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    heightmap: &HeightMap,
    points: &[[f32; 2]],
    half_width_m: f32,
) {
    const CHALK: [f32; 3] = [0.86, 0.78, 0.52];
    let ground = |x: f32, z: f32| heightmap.sample_height(x, z).unwrap_or(0.0) + 0.24;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let length = (dx * dx + dz * dz).sqrt();
        if length < 0.5 {
            continue;
        }
        let steps = (length / 4.0).ceil() as u32;
        let (nx, nz) = (-dz / length, dx / length);
        let base = vertices.len() as u32;
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let (x, z) = (a[0] + dx * t, a[1] + dz * t);
            for side in [-1.0_f32, 1.0] {
                let (px, pz) = (x + nx * half_width_m * side, z + nz * half_width_m * side);
                vertices.push(SceneVertex::new([px, ground(px, pz), pz], [0.0, 1.0, 0.0], CHALK));
            }
        }
        for step in 0..steps {
            let row = base + step * 2;
            indices.extend_from_slice(&[row, row + 1, row + 3, row, row + 3, row + 2]);
        }
    }
    for point in points {
        crate::markers::probe_marker(
            vertices,
            indices,
            glam::Vec3::new(point[0], ground(point[0], point[1]) - 0.1, point[1]),
            0.8,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use map_forge::blueprint::SymmetrySpec;

    fn scratch() -> MapBlueprint {
        crate::EditorDocument::new_scratch().blueprint().clone()
    }

    fn zigzag(kind: StrokeKind) -> PendingStroke {
        let mut pending = PendingStroke::new(kind);
        for at in [[60.3, 60.7], [90.0, 70.0], [95.0, 110.0], [140.0, 130.0], [180.0, 128.0]] {
            pending.add_point(at, 5.0);
        }
        pending
    }

    #[test]
    fn a_committed_stroke_is_smoothed_quantized_and_grows_a_twin_on_fair_maps() {
        let blueprint = scratch();
        let (ops, summary) = fit_stroke(&zigzag(StrokeKind::Ridge), &blueprint).expect("fits");
        assert!(summary.contains("ridge stroke"));
        assert_eq!(ops.len(), 1, "no twin without symmetry");
        let TerrainOp::Stroke(spec) = &ops[0] else { panic!("a stroke commits a Stroke op") };
        assert!(spec.points.len() >= 2 && spec.points.len() <= 64);
        for [x, z] in &spec.points {
            assert_eq!(x * 2.0, (x * 2.0).round(), "points quantize to the 0.5 m lattice");
            assert_eq!(z * 2.0, (z * 2.0).round());
        }
        // The fit rounds the raw corner: the sharp waypoint at (95, 110) is not kept.
        assert!(
            !spec.points.iter().any(|[x, z]| *x == 95.0 && *z == 110.0),
            "Chaikin cuts the raw corner"
        );

        let mut fair = blueprint.clone();
        fair.symmetry = Some(SymmetrySpec::MirrorZ);
        let (ops, _) = fit_stroke(&zigzag(StrokeKind::Ridge), &fair).expect("fits");
        assert_eq!(ops.len(), 2, "fair maps grow the twin");
        let (TerrainOp::Stroke(south), TerrainOp::Stroke(north)) = (&ops[0], &ops[1]) else {
            panic!()
        };
        let axis = fair.grid.axis_z();
        for (a, b) in south.points.iter().zip(&north.points) {
            assert_eq!(a[0].to_bits(), b[0].to_bits(), "the twin shares x bit-for-bit");
            assert_eq!((axis * 2.0 - a[1]).to_bits(), b[1].to_bits(), "z reflects bit-for-bit");
        }
        // End to end: the compiled fair map stays fair.
        let mut with_stroke = fair.clone();
        crate::stamp::insert(&mut with_stroke, ops);
        let (_, report) = map_forge::compile(&with_stroke);
        assert!(
            !report.errors().any(|entry| entry.check == "symmetry"),
            "a drawn fair map stays fair"
        );
    }

    #[test]
    fn stroke_fitting_is_deterministic_and_caps_the_point_budget() {
        let blueprint = scratch();
        let (first, _) = fit_stroke(&zigzag(StrokeKind::Valley), &blueprint).expect("fits");
        let (second, _) = fit_stroke(&zigzag(StrokeKind::Valley), &blueprint).expect("fits");
        assert_eq!(first, second, "the same clicks always fit the same curve");

        // A stroke across the whole map still fits the 64-point report budget.
        let mut long = PendingStroke::new(StrokeKind::Ridge);
        for step in 0..40 {
            let t = step as f32;
            long.add_point([10.0 + t * 7.0, 100.0 + 30.0 * (t * 0.7).sin()], 5.0);
        }
        let (ops, _) = fit_stroke(&long, &blueprint).expect("fits");
        let TerrainOp::Stroke(spec) = &ops[0] else { panic!() };
        assert!(
            spec.points.len() <= 64,
            "the resample stretches its interval past the budget, got {}",
            spec.points.len()
        );
        let (_, report) = {
            let mut with_stroke = blueprint.clone();
            crate::stamp::insert(&mut with_stroke, ops);
            map_forge::compile(&with_stroke)
        };
        assert!(!report.entries.iter().any(|entry| entry.check == "stroke"));
    }

    #[test]
    fn one_click_refuses_two_clicks_commit_and_the_commit_is_one_undo_step() {
        let mut document = crate::EditorDocument::new_scratch();
        let ops_before = document.blueprint().terrain.ops.len();

        let mut dot = PendingStroke::new(StrokeKind::Ridge);
        dot.add_point([100.0, 100.0], 5.0);
        assert!(fit_stroke(&dot, document.blueprint()).is_none(), "a line, not a dot");

        let mut line = dot.clone();
        line.add_point([160.0, 120.0], 5.0);
        let (ops, _) = fit_stroke(&line, document.blueprint()).expect("two clicks fit");
        document.apply_edit(|blueprint| crate::stamp::insert(blueprint, ops));
        assert_eq!(document.blueprint().terrain.ops.len(), ops_before + 1);
        assert!(document.undo(), "one gesture, one undo step");
        assert_eq!(document.blueprint().terrain.ops.len(), ops_before);
    }

    #[test]
    fn the_plateau_seats_its_target_at_the_first_clicks_ground() {
        let mut pending = PendingStroke::new(StrokeKind::Plateau);
        pending.add_point([100.0, 100.0], 7.13);
        pending.add_point([160.0, 100.0], 2.0);
        assert_eq!(pending.parameter.value_m, 7.25, "target = first click's ground, quantized");
        let (ops, _) = fit_stroke(&pending, &scratch()).expect("fits");
        let TerrainOp::Stroke(spec) = &ops[0] else { panic!() };
        assert_eq!(spec.profile, StrokeProfile::Plateau { target_m: 7.25 });
    }
}
