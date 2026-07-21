//! Structural op stamps (M4b): two-click gestures that place QUANTIZED `TerrainOp`s into
//! the document's program — the op list stays the design, the editor only writes into it.
//! Positions snap to the heightmap cell, amplitudes to half metres, sigmas to metres: a
//! stamp is deterministic data with a readable diff, never float soup.
//!
//! The stampable set is the vocabulary's FREE-PLACEMENT ops: Hill/Bowl (`Gauss2`), the
//! hull-down Crest (`CrestShelf` window) and the river Deck. `FlattenToRamp` and
//! `RidgeGated` keep axis-coupled masks by design (the fairness contract) and stay
//! RON-authored. On a mirror-fair map every stamp lands with its twin — `Gauss2` grows a
//! mirrored term, `CrestShelf` a mirrored window, `Deck` a mirrored op — so the symmetry
//! Error can never fire on an editor gesture.

use map_forge::blueprint::{Apply, Gauss2Term, MapBlueprint, TerrainOp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StampKind {
    Hill,
    Bowl,
    Crest,
    Deck,
}

impl StampKind {
    pub const CYCLE: [StampKind; 4] =
        [StampKind::Hill, StampKind::Bowl, StampKind::Crest, StampKind::Deck];

    pub fn label(self) -> &'static str {
        match self {
            StampKind::Hill => "hill",
            StampKind::Bowl => "bowl",
            StampKind::Crest => "crest",
            StampKind::Deck => "deck",
        }
    }

    /// What the second click means, for the footer hint.
    pub fn gesture_hint(self) -> &'static str {
        match self {
            StampKind::Hill | StampKind::Bowl => "click 1: centre, click 2: radius",
            StampKind::Crest => "click 1: crest, click 2: shelf side + window",
            StampKind::Deck => "click 1: deck centre, click 2: width",
        }
    }

    /// The tunable parameters and their defaults, in inspector order.
    pub fn parameters(self) -> Vec<Parameter> {
        match self {
            StampKind::Hill | StampKind::Bowl => {
                vec![Parameter { name: "height", value_m: 6.0, step_m: 0.5, range: (0.5, 30.0) }]
            }
            StampKind::Crest => vec![
                Parameter { name: "crest h", value_m: 1.5, step_m: 0.5, range: (0.5, 6.0) },
                Parameter { name: "shelf cut", value_m: 1.0, step_m: 0.5, range: (0.0, 4.0) },
                Parameter { name: "sigma x", value_m: 9.0, step_m: 1.0, range: (4.0, 30.0) },
            ],
            StampKind::Deck => vec![
                Parameter { name: "deck h", value_m: 1.4, step_m: 0.1, range: (0.3, 4.0) },
                Parameter { name: "half len", value_m: 30.0, step_m: 2.0, range: (10.0, 60.0) },
            ],
        }
    }
}

/// One inspector knob: a named metre value with its quantum and range.
#[derive(Debug, Clone, Copy)]
pub struct Parameter {
    pub name: &'static str,
    pub value_m: f32,
    pub step_m: f32,
    pub range: (f32, f32),
}

impl Parameter {
    pub fn adjust(&mut self, steps: f32) {
        self.value_m = quantize_m(
            (self.value_m + steps * self.step_m).clamp(self.range.0, self.range.1),
            self.step_m,
        );
    }
}

/// Build the op(s) a completed gesture places, plus a human summary for the status line.
/// `None` when the gesture cannot land on this document (a deck without a river, a
/// degenerate extent) — the caller reports why via [`refusal`].
pub fn build_stamp(
    kind: StampKind,
    parameters: &[Parameter],
    blueprint: &MapBlueprint,
    anchor: [f32; 2],
    second: [f32; 2],
) -> Option<(Vec<TerrainOp>, String)> {
    let cell = blueprint.grid.cell_m;
    let axis_z = blueprint.grid.axis_z();
    let mirrored = blueprint.symmetry.is_some();
    let snap = |value: f32| quantize_m(value, cell);
    let anchor = [snap(anchor[0]), snap(anchor[1])];
    let second = [snap(second[0]), snap(second[1])];
    let value = |index: usize| parameters.get(index).map_or(0.0, |parameter| parameter.value_m);

    match kind {
        StampKind::Hill | StampKind::Bowl => {
            let dx = second[0] - anchor[0];
            let dz = second[1] - anchor[1];
            let radius = quantize_m((dx * dx + dz * dz).sqrt(), 1.0).max(cell);
            let sigma = (radius * 0.5).max(cell * 0.5);
            let amp = value(0);
            let mut terms =
                vec![Gauss2Term { x: anchor[0], z: anchor[1], sx: sigma, sz: sigma, amp }];
            if mirrored && (anchor[1] - axis_z).abs() > cell * 0.5 {
                terms.push(Gauss2Term {
                    x: anchor[0],
                    z: axis_z * 2.0 - anchor[1],
                    sx: sigma,
                    sz: sigma,
                    amp,
                });
            }
            let apply = if kind == StampKind::Hill { Apply::Add } else { Apply::Subtract };
            let verb = if kind == StampKind::Hill { "hill" } else { "bowl" };
            let summary = format!(
                "{verb} at {:.0},{:.0}  r {radius:.0} m  h {amp:.1} m",
                anchor[0], anchor[1]
            );
            Some((vec![TerrainOp::Gauss2 { apply, terms }], summary))
        }
        StampKind::Crest => {
            let crest_x = anchor[0];
            let shelf_x = if (second[0] - crest_x).abs() < cell {
                crest_x - 25.0
            } else {
                quantize_m(second[0], cell)
            };
            let window_sigma = quantize_m((second[1] - anchor[1]).abs(), 1.0).max(20.0);
            let mut windows = vec![[anchor[1], window_sigma]];
            if mirrored && (anchor[1] - axis_z).abs() > cell * 0.5 {
                windows.push([axis_z * 2.0 - anchor[1], window_sigma]);
            }
            let summary = format!(
                "crest at x {crest_x:.0} (shelf x {shelf_x:.0})  window z {:.0} +/- {window_sigma:.0} m",
                anchor[1]
            );
            Some((
                vec![TerrainOp::CrestShelf {
                    crest_x,
                    shelf_x,
                    sigma_x: value(2),
                    crest_h: value(0),
                    shelf_cut: value(1),
                    windows,
                }],
                summary,
            ))
        }
        StampKind::Deck => {
            let (river, water) = (blueprint.river.as_ref()?, blueprint.water.as_ref()?);
            let _ = river;
            let dz = quantize_m(anchor[1] - axis_z, cell);
            let half_width = quantize_m((second[1] - anchor[1]).abs(), 1.0).clamp(3.0, 12.0);
            let deck_m = water.surface_level_m + value(0);
            let mut ops = vec![TerrainOp::Deck {
                dz_m: dz,
                deck_m,
                half_width_m: half_width,
                half_length_m: value(1),
                width_falloff_m: 6.0,
                length_falloff_m: 14.0,
            }];
            if mirrored && dz.abs() > cell * 0.5 {
                ops.push(TerrainOp::Deck {
                    dz_m: -dz,
                    deck_m,
                    half_width_m: half_width,
                    half_length_m: value(1),
                    width_falloff_m: 6.0,
                    length_falloff_m: 14.0,
                });
            }
            let summary =
                format!("deck at dz {dz:.0}  half w {half_width:.0} m  {deck_m:.1} m ASL");
            Some((ops, summary))
        }
    }
}

/// Why [`build_stamp`] refused, for the status line.
pub fn refusal(kind: StampKind, blueprint: &MapBlueprint) -> &'static str {
    match kind {
        StampKind::Deck if blueprint.river.is_none() || blueprint.water.is_none() => {
            "a deck needs a river and standing water - this document has neither"
        }
        _ => "the gesture does not land on this document",
    }
}

/// Insert placed ops into the program: before the trailing `ClampMin` when the document
/// ends with one (the floor clamp stays last — "decks applied last" stays visible), at the
/// end otherwise.
pub fn insert(blueprint: &mut MapBlueprint, ops: Vec<TerrainOp>) {
    let at = match blueprint.terrain.ops.last() {
        Some(TerrainOp::ClampMin(_)) => blueprint.terrain.ops.len() - 1,
        _ => blueprint.terrain.ops.len(),
    };
    for (offset, op) in ops.into_iter().enumerate() {
        blueprint.terrain.ops.insert(at + offset, op);
    }
}

/// Snap a metre value to its quantum — the shared discipline of every gesture tool
/// (stamps and strokes alike): committed data is readable, never float soup.
pub(crate) fn quantize_m(value: f32, step: f32) -> f32 {
    (value / step).round() * step
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> MapBlueprint {
        crate::EditorDocument::new_scratch().blueprint().clone()
    }

    #[test]
    fn a_hill_stamp_is_quantized_and_grows_a_twin_on_fair_maps() {
        let mut blueprint = scratch();
        let parameters = StampKind::Hill.parameters();
        let (ops, summary) =
            build_stamp(StampKind::Hill, &parameters, &blueprint, [101.3, 98.7], [131.9, 98.7])
                .expect("lands");
        assert!(summary.contains("hill"));
        let TerrainOp::Gauss2 { apply: Apply::Add, terms } = &ops[0] else {
            panic!("a hill is an additive Gauss2")
        };
        assert_eq!(terms.len(), 1, "no twin without symmetry");
        assert_eq!(terms[0].x, 100.0, "positions snap to the 5 m cell");
        assert_eq!(terms[0].z, 100.0);
        assert_eq!(terms[0].sx, 15.0, "sigma = half the quantized radius");

        blueprint.symmetry = Some(map_forge::blueprint::SymmetrySpec::MirrorZ);
        let (ops, _) =
            build_stamp(StampKind::Hill, &parameters, &blueprint, [100.0, 100.0], [130.0, 100.0])
                .expect("lands");
        let TerrainOp::Gauss2 { terms, .. } = &ops[0] else { panic!() };
        assert_eq!(terms.len(), 2, "fair maps stamp the twin");
        assert_eq!(terms[1].z, 200.0, "mirrored across the 150 m axis");

        // The compiled result honours the mirror contract end to end.
        insert(&mut blueprint, ops);
        let (_, report) = map_forge::compile(&blueprint);
        assert!(
            !report.errors().any(|entry| entry.check == "symmetry"),
            "a stamped fair map stays fair"
        );
    }

    #[test]
    fn a_deck_needs_a_river_and_ops_insert_before_the_floor_clamp() {
        let blueprint = scratch();
        let parameters = StampKind::Deck.parameters();
        assert!(
            build_stamp(StampKind::Deck, &parameters, &blueprint, [150.0, 150.0], [150.0, 156.0])
                .is_none(),
            "a riverless deck is refused"
        );
        assert!(refusal(StampKind::Deck, &blueprint).contains("river"));

        // Insertion honours a trailing floor clamp.
        let mut with_clamp = blueprint.clone();
        with_clamp.terrain.ops.push(TerrainOp::ClampMin(0.2));
        let (ops, _) = build_stamp(
            StampKind::Hill,
            &StampKind::Hill.parameters(),
            &with_clamp,
            [100.0, 100.0],
            [120.0, 100.0],
        )
        .expect("lands");
        insert(&mut with_clamp, ops);
        assert!(
            matches!(with_clamp.terrain.ops.last(), Some(TerrainOp::ClampMin(_))),
            "the floor clamp stays last"
        );
        assert!(matches!(
            with_clamp.terrain.ops[with_clamp.terrain.ops.len() - 2],
            TerrainOp::Gauss2 { .. }
        ));
    }

    #[test]
    fn a_crest_stamp_builds_a_hull_down_window_where_clicked() {
        let blueprint = scratch();
        let parameters = StampKind::Crest.parameters();
        let (ops, _) =
            build_stamp(StampKind::Crest, &parameters, &blueprint, [200.0, 120.0], [175.0, 150.0])
                .expect("lands");
        let TerrainOp::CrestShelf { crest_x, shelf_x, windows, crest_h, .. } = &ops[0] else {
            panic!("a crest stamp is a CrestShelf")
        };
        assert_eq!(*crest_x, 200.0);
        assert_eq!(*shelf_x, 175.0, "the shelf side follows the second click");
        assert_eq!(windows[0][0], 120.0);
        assert_eq!(windows[0][1], 30.0, "window sigma from the click span");
        assert_eq!(*crest_h, 1.5, "the inspector default");
        let mut parameter = parameters[0];
        parameter.adjust(3.0);
        assert_eq!(parameter.value_m, 3.0, "knobs move in their quantum");
    }
}
