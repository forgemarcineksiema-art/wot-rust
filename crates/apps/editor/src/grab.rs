//! The grab tool (Rece do terenu W3): existing terrain forms become handles in the world.
//! H arms it; a click picks the nearest form (a hill term, a drawn stroke, a bench), a
//! drag moves it along the ground (1 m snap), Shift+drag lifts or sinks it (0.25 m snap),
//! `[` `]` widen or narrow it (x1.25, quantized). No numbers in the UI - the status line
//! speaks words and the ghost shows the truth. Every transform lands on BOTH mirror twins:
//! the twin is resolved STRUCTURALLY at pick time (params match under the reflection),
//! never stored, so it cannot drift - and the fairness contract survives direct
//! manipulation by construction.
//!
//! Deliberately ungrabbable: `RidgeGated`, `CrestShelf`, `Deck`, `FlattenToRamp` - their
//! axis-coupled masks ARE the fairness design (the same reasoning that keeps them
//! unstampable), and the river ops follow the river, not the hand.

use glam::Vec3;
use map_forge::blueprint::{Apply, MapBlueprint, StrokeProfile, TerrainOp};
use terrain::HeightMap;

use crate::stamp::quantize_m;

/// A grabbable document entry (always the PRIMARY member of a twin pair).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormRef {
    Gauss2Term { op_index: usize, term_index: usize },
    Stroke { op_index: usize },
    FlattenToGauss { op_index: usize },
}

/// One handle in the world: the primary form, its structural twin (when off-axis on a
/// fair map), the pick box, and the words the status line speaks.
#[derive(Debug, Clone)]
pub struct Form {
    pub primary: FormRef,
    pub twin: Option<FormRef>,
    pub center: Vec3,
    pub half: Vec3,
    /// Marker-ring radius around the handle's foot.
    pub footprint_m: f32,
    pub label: &'static str,
}

/// The accumulated drag, applied identically to the ghost preview and the commit - the
/// snapping lives in [`apply_transform`], so both see the same truth.
#[derive(Debug, Clone, Copy)]
pub struct Transform {
    /// Ground drag in world metres (snapped to 1 m on apply).
    pub move_m: [f32; 2],
    /// Shift-drag lift in metres (snapped to 0.25 m on apply).
    pub raise_m: f32,
    /// Multiplicative width factor (widths re-quantized to 0.5 m on apply).
    pub widen: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self { move_m: [0.0, 0.0], raise_m: 0.0, widen: 1.0 }
    }
}

impl Transform {
    /// Whether the snapped transform would change anything at all - a click without a
    /// drag must not dirty the document.
    pub fn is_meaningful(&self) -> bool {
        quantize_m(self.move_m[0], 1.0) != 0.0
            || quantize_m(self.move_m[1], 1.0) != 0.0
            || quantize_m(self.raise_m, 0.25) != 0.0
            || (self.widen - 1.0).abs() > 1.0e-3
    }
}

/// Every grabbable form in the document, twins resolved. Rebuilt at pick time from
/// structure - cheap (a walk over the op list) and impossible to desynchronize.
pub fn enumerate_forms(blueprint: &MapBlueprint, heightmap: &HeightMap) -> Vec<Form> {
    let axis_z = blueprint.grid.axis_z();
    let near = |a: f32, b: f32| (a - b).abs() < 0.5;
    let ground = |x: f32, z: f32| heightmap.sample_height(x, z).unwrap_or(0.0);
    let mut out = Vec::new();

    for (op_index, op) in blueprint.terrain.ops.iter().enumerate() {
        match op {
            TerrainOp::Gauss2 { apply, terms } => {
                for (term_index, term) in terms.iter().enumerate() {
                    // The twin is a sibling term in the SAME op whose center reflects and
                    // whose shape matches; emit the pair once, from its lower index.
                    let twin = terms.iter().enumerate().find(|(other_index, other)| {
                        *other_index != term_index
                            && near(other.x, term.x)
                            && near(other.z, axis_z * 2.0 - term.z)
                            && near(other.sx, term.sx)
                            && near(other.sz, term.sz)
                            && near(other.amp, term.amp)
                    });
                    if let Some((twin_index, _)) = twin
                        && twin_index < term_index
                    {
                        continue; // already emitted from the twin's side
                    }
                    out.push(Form {
                        primary: FormRef::Gauss2Term { op_index, term_index },
                        twin: twin.map(|(twin_index, _)| FormRef::Gauss2Term {
                            op_index,
                            term_index: twin_index,
                        }),
                        center: Vec3::new(term.x, ground(term.x, term.z), term.z),
                        half: Vec3::new(term.sx * 2.0, term.amp.abs().max(4.0), term.sz * 2.0),
                        footprint_m: term.sx.max(term.sz),
                        label: match apply {
                            Apply::Add => "hill",
                            Apply::Subtract => "bowl",
                        },
                    });
                }
            }
            TerrainOp::Stroke(spec) => {
                let twin = blueprint.terrain.ops.iter().enumerate().find(|(other_index, other)| {
                    let TerrainOp::Stroke(other) = other else { return false };
                    *other_index != op_index
                        && other.profile == spec.profile
                        && near(other.half_width_m, spec.half_width_m)
                        && near(other.falloff_m, spec.falloff_m)
                        && other.points.len() == spec.points.len()
                        && other
                            .points
                            .iter()
                            .zip(&spec.points)
                            .all(|(a, b)| near(a[0], b[0]) && near(a[1], axis_z * 2.0 - b[1]))
                });
                if let Some((twin_index, _)) = twin
                    && twin_index < op_index
                {
                    continue;
                }
                let (mut min_x, mut min_z) = (f32::MAX, f32::MAX);
                let (mut max_x, mut max_z) = (f32::MIN, f32::MIN);
                for [x, z] in &spec.points {
                    min_x = min_x.min(*x);
                    min_z = min_z.min(*z);
                    max_x = max_x.max(*x);
                    max_z = max_z.max(*z);
                }
                let reach = spec.half_width_m + spec.falloff_m;
                let center_x = (min_x + max_x) * 0.5;
                let center_z = (min_z + max_z) * 0.5;
                out.push(Form {
                    primary: FormRef::Stroke { op_index },
                    twin: twin.map(|(twin_index, _)| FormRef::Stroke { op_index: twin_index }),
                    center: Vec3::new(center_x, ground(center_x, center_z), center_z),
                    half: Vec3::new(
                        (max_x - min_x) * 0.5 + reach,
                        6.0,
                        (max_z - min_z) * 0.5 + reach,
                    ),
                    footprint_m: reach.max(8.0),
                    label: match spec.profile {
                        StrokeProfile::Ridge { .. } => "ridge stroke",
                        StrokeProfile::Valley { .. } => "valley stroke",
                        StrokeProfile::Plateau { .. } => "bench stroke",
                    },
                });
            }
            TerrainOp::FlattenToGauss { target_m, x, z, sx, sz } => {
                let twin = blueprint.terrain.ops.iter().enumerate().find(|(other_index, other)| {
                    let TerrainOp::FlattenToGauss { target_m: t2, x: x2, z: z2, sx: sx2, sz: sz2 } =
                        other
                    else {
                        return false;
                    };
                    *other_index != op_index
                        && near(*x2, *x)
                        && near(*z2, axis_z * 2.0 - z)
                        && near(*t2, *target_m)
                        && near(*sx2, *sx)
                        && near(*sz2, *sz)
                });
                if let Some((twin_index, _)) = twin
                    && twin_index < op_index
                {
                    continue;
                }
                out.push(Form {
                    primary: FormRef::FlattenToGauss { op_index },
                    twin: twin
                        .map(|(twin_index, _)| FormRef::FlattenToGauss { op_index: twin_index }),
                    center: Vec3::new(*x, *target_m, *z),
                    half: Vec3::new(sx * 2.0, 4.0, sz * 2.0),
                    footprint_m: sx.max(*sz),
                    label: "bench",
                });
            }
            _ => {}
        }
    }
    out
}

/// Apply the SNAPPED transform to the form and its twin. The one shared door: the ghost
/// preview and the commit both walk through it, so release always sets exactly what the
/// eye saw. Returns the words for the status line.
pub fn apply_transform(blueprint: &mut MapBlueprint, form: &Form, transform: &Transform) -> String {
    let dx = quantize_m(transform.move_m[0], 1.0);
    let dz = quantize_m(transform.move_m[1], 1.0);
    let raise = quantize_m(transform.raise_m, 0.25);
    let widen = transform.widen;
    // The twin mirrors the ground motion; lift and width are shared.
    apply_to(blueprint, form.primary, dx, dz, raise, widen);
    if let Some(twin) = form.twin {
        apply_to(blueprint, twin, dx, -dz, raise, widen);
    }
    let mut spoken: Vec<&str> = Vec::new();
    if dx != 0.0 || dz != 0.0 {
        spoken.push("moved");
    }
    if raise > 0.0 {
        spoken.push("taller");
    }
    if raise < 0.0 {
        spoken.push("lower");
    }
    if widen > 1.0 {
        spoken.push("wider");
    }
    if widen < 1.0 {
        spoken.push("narrower");
    }
    if spoken.is_empty() {
        format!("{} - untouched", form.label)
    } else {
        format!("{} - {}", form.label, spoken.join(", "))
    }
}

fn apply_to(blueprint: &mut MapBlueprint, form: FormRef, dx: f32, dz: f32, raise: f32, widen: f32) {
    let width = |value: f32| quantize_m((value * widen).clamp(2.0, 120.0), 0.5);
    match form {
        FormRef::Gauss2Term { op_index, term_index } => {
            let Some(TerrainOp::Gauss2 { terms, .. }) = blueprint.terrain.ops.get_mut(op_index)
            else {
                return;
            };
            let Some(term) = terms.get_mut(term_index) else { return };
            term.x += dx;
            term.z += dz;
            term.amp = quantize_m((term.amp + raise).clamp(0.5, 40.0), 0.25);
            term.sx = width(term.sx);
            term.sz = width(term.sz);
        }
        FormRef::Stroke { op_index } => {
            let Some(TerrainOp::Stroke(spec)) = blueprint.terrain.ops.get_mut(op_index) else {
                return;
            };
            for point in &mut spec.points {
                point[0] += dx;
                point[1] += dz;
            }
            spec.half_width_m = quantize_m((spec.half_width_m * widen).clamp(2.0, 40.0), 0.5);
            spec.falloff_m = spec.half_width_m;
            spec.profile = match spec.profile {
                StrokeProfile::Ridge { amp_m } => StrokeProfile::Ridge {
                    amp_m: quantize_m((amp_m + raise).clamp(0.5, 20.0), 0.25),
                },
                StrokeProfile::Valley { depth_m } => StrokeProfile::Valley {
                    depth_m: quantize_m((depth_m - raise).clamp(0.5, 20.0), 0.25),
                },
                StrokeProfile::Plateau { target_m } => {
                    StrokeProfile::Plateau { target_m: quantize_m(target_m + raise, 0.25) }
                }
            };
        }
        FormRef::FlattenToGauss { op_index } => {
            let Some(TerrainOp::FlattenToGauss { target_m, x, z, sx, sz }) =
                blueprint.terrain.ops.get_mut(op_index)
            else {
                return;
            };
            *x += dx;
            *z += dz;
            *target_m = quantize_m(*target_m + raise, 0.25);
            *sx = width(*sx);
            *sz = width(*sz);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use map_forge::blueprint::{Gauss2Term, StrokeSpec, SymmetrySpec};

    /// A fair document with one of each grabbable pair plus an on-axis single.
    fn fixture() -> MapBlueprint {
        let mut blueprint = crate::EditorDocument::new_scratch().blueprint().clone();
        blueprint.symmetry = Some(SymmetrySpec::MirrorZ);
        let axis = blueprint.grid.axis_z();
        blueprint.terrain.ops.push(TerrainOp::Gauss2 {
            apply: Apply::Add,
            terms: vec![
                Gauss2Term { x: 100.0, z: 100.0, sx: 20.0, sz: 20.0, amp: 6.0 },
                Gauss2Term { x: 100.0, z: axis * 2.0 - 100.0, sx: 20.0, sz: 20.0, amp: 6.0 },
                Gauss2Term { x: 220.0, z: axis, sx: 15.0, sz: 15.0, amp: 4.0 },
            ],
        });
        let south = StrokeSpec {
            points: vec![[60.0, 60.0], [90.0, 80.0], [120.0, 90.0]],
            profile: StrokeProfile::Ridge { amp_m: 4.0 },
            half_width_m: 6.0,
            falloff_m: 6.0,
        };
        let north = StrokeSpec {
            points: south.points.iter().map(|[x, z]| [*x, axis * 2.0 - z]).collect(),
            ..south.clone()
        };
        blueprint.terrain.ops.push(TerrainOp::Stroke(south));
        blueprint.terrain.ops.push(TerrainOp::Stroke(north));
        blueprint.terrain.ops.push(TerrainOp::FlattenToGauss {
            target_m: 9.0,
            x: 200.0,
            z: 110.0,
            sx: 18.0,
            sz: 18.0,
        });
        blueprint.terrain.ops.push(TerrainOp::FlattenToGauss {
            target_m: 9.0,
            x: 200.0,
            z: axis * 2.0 - 110.0,
            sx: 18.0,
            sz: 18.0,
        });
        blueprint
    }

    #[test]
    fn grab_enumerates_every_v1_form_with_its_structural_twin() {
        let blueprint = fixture();
        let heightmap = map_forge::compile(&blueprint).0.heightmap;
        let forms = enumerate_forms(&blueprint, &heightmap);
        // One hill pair, one on-axis hill, one stroke pair, one bench pair - pairs emit ONCE.
        assert_eq!(forms.len(), 4, "got {:?}", forms.iter().map(|f| f.label).collect::<Vec<_>>());
        let hills: Vec<_> = forms.iter().filter(|form| form.label == "hill").collect();
        assert_eq!(hills.len(), 2);
        assert!(hills.iter().any(|form| form.twin.is_some()), "the off-axis pair resolves");
        assert!(hills.iter().any(|form| form.twin.is_none()), "the on-axis single stands alone");
        assert!(
            forms.iter().any(|form| form.label == "ridge stroke" && form.twin.is_some()),
            "the stroke pair resolves"
        );
        assert!(
            forms.iter().any(|form| form.label == "bench" && form.twin.is_some()),
            "the bench pair resolves"
        );
    }

    #[test]
    fn moving_a_hill_moves_both_twins_and_the_compiled_map_stays_fair() {
        let mut blueprint = fixture();
        let heightmap = map_forge::compile(&blueprint).0.heightmap;
        let forms = enumerate_forms(&blueprint, &heightmap);
        let hill = forms
            .iter()
            .find(|form| form.label == "hill" && form.twin.is_some())
            .expect("the pair exists");
        let transform = Transform { move_m: [12.2, 6.8], raise_m: 0.0, widen: 1.0 };
        let spoken = apply_transform(&mut blueprint, hill, &transform);
        assert!(spoken.contains("moved"));
        let FormRef::Gauss2Term { op_index, .. } = hill.primary else { panic!() };
        let TerrainOp::Gauss2 { terms, .. } = &blueprint.terrain.ops[op_index] else { panic!() };
        assert_eq!(terms[0].x, 112.0, "x snaps to the metre");
        assert_eq!(terms[0].z, 107.0);
        assert_eq!(terms[1].x, 112.0, "the twin shares the x move");
        assert_eq!(terms[1].z, blueprint.grid.axis_z() * 2.0 - 107.0, "the twin mirrors dz");
        let (_, report) = map_forge::compile(&blueprint);
        assert!(
            !report.errors().any(|entry| entry.check == "symmetry"),
            "direct manipulation keeps the map fair"
        );
    }

    #[test]
    fn a_grab_transform_snaps_and_is_one_undo_step() {
        let mut document = crate::EditorDocument::new_scratch();
        document.apply_edit(|blueprint| *blueprint = fixture());
        let heightmap = map_forge::compile(document.blueprint()).0.heightmap;
        let forms = enumerate_forms(document.blueprint(), &heightmap);
        let stroke = forms.iter().find(|form| form.label == "ridge stroke").expect("exists");

        // A sub-snap drag is meaningless - the caller must not dirty the document with it.
        let idle = Transform { move_m: [0.3, 0.4], raise_m: 0.05, widen: 1.0 };
        assert!(!idle.is_meaningful());

        let before = document.blueprint().clone();
        let lift = Transform { move_m: [0.0, 0.0], raise_m: 0.13, widen: 1.25 };
        assert!(lift.is_meaningful());
        let stroke_form = stroke.clone();
        document.apply_edit(|blueprint| {
            apply_transform(blueprint, &stroke_form, &lift);
        });
        let FormRef::Stroke { op_index } = stroke_form.primary else { panic!() };
        let TerrainOp::Stroke(spec) = &document.blueprint().terrain.ops[op_index] else { panic!() };
        assert_eq!(spec.profile, StrokeProfile::Ridge { amp_m: 4.25 }, "lift snaps to 0.25");
        assert_eq!(spec.half_width_m, 7.5, "width x1.25 re-quantizes to 0.5");
        assert_eq!(spec.falloff_m, 7.5, "the falloff rides the width");
        assert!(document.undo(), "one gesture, one undo step");
        assert_eq!(document.blueprint(), &before);
    }
}
