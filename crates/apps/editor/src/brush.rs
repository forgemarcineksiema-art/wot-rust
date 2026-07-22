//! The terrain brushes (M4): freeform sculpting into the document's D1 `sculpt` layer.
//! A stroke is a live accumulation of dabs over the compiled heightmap; on release it
//! quantizes into the blueprint's sparse delta samples through the document's single
//! `apply_edit` door — one stroke, one undo step, full recompile after.
//!
//! Mirror-fairness is BY CONSTRUCTION: on a symmetric map every dab also lands at its
//! mirrored centre, and base-dependent modes (Flatten/Smooth) read heights through the
//! canonical south-half twin — both halves receive bit-identical deltas, so the report's
//! sculpt-mirror Error can never fire on an editor stroke.

use std::collections::BTreeMap;

use map_forge::blueprint::{MapBlueprint, SculptSpec};
use terrain::{HeightMap, smoothstep01};

/// The default quantum for a document that has no sculpt layer yet: 5 cm — fine enough
/// for a ramp, coarse enough that a stroke is honest data.
pub const DEFAULT_STEP_M: f32 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushMode {
    Raise,
    Lower,
    Flatten,
    Smooth,
    /// Macro (Rece do terenu W4): an anisotropic crest along the DRAG direction — the
    /// hand's line, not a blob trail. The first dab (no tangent yet) raises isotropically.
    Ridge,
    /// Macro: flatten toward the nearest multiple of `terrace_step_m` — risers emerge
    /// naturally where the rounding flips.
    Terrace,
    /// Macro: wide-kernel relaxation with a peak bias — peaks shed faster than hollows
    /// fill (the thermal-erosion read, with slight honest mass loss).
    Erode,
}

impl BrushMode {
    pub const CYCLE: [BrushMode; 7] = [
        BrushMode::Raise,
        BrushMode::Lower,
        BrushMode::Flatten,
        BrushMode::Smooth,
        BrushMode::Ridge,
        BrushMode::Terrace,
        BrushMode::Erode,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BrushMode::Raise => "raise",
            BrushMode::Lower => "lower",
            BrushMode::Flatten => "flatten",
            BrushMode::Smooth => "smooth",
            BrushMode::Ridge => "ridge",
            BrushMode::Terrace => "terrace",
            BrushMode::Erode => "erode",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BrushSettings {
    pub mode: BrushMode,
    pub radius_m: f32,
    /// Full-strength vertical rate at the brush centre, metres per second.
    pub rate_m_s: f32,
    /// The terrace riser height (Terrace mode only; Tab cycles 1 / 2 / 4 m).
    pub terrace_step_m: f32,
}

/// One live stroke: the working delta (metres, unquantized) over the stroke's base
/// heightmap. The base already contains the document's committed sculpt — a stroke stacks.
pub struct Stroke {
    side: usize,
    cell_m: f32,
    min_height_m: f32,
    step_m: f32,
    mirrored: bool,
    base: Vec<f32>,
    delta_m: Vec<f32>,
    flatten_target_m: Option<f32>,
    touched: bool,
    last_dab: Option<[f32; 2]>,
    /// The drag's unit direction, updated when the hand actually moves — a stationary
    /// Ridge hold keeps building the crest it was drawing, not a blob.
    tangent: Option<[f32; 2]>,
}

impl Stroke {
    /// Begin over the CURRENT compiled heightmap (its samples are the stroke's base truth).
    pub fn begin(blueprint: &MapBlueprint, heightmap: &HeightMap) -> Self {
        Self {
            side: heightmap.width(),
            cell_m: heightmap.cell_size_m(),
            min_height_m: blueprint.grid.min_height_m,
            step_m: blueprint.sculpt.as_ref().map_or(DEFAULT_STEP_M, |sculpt| sculpt.step_m),
            mirrored: blueprint.symmetry.is_some(),
            base: heightmap.samples().to_vec(),
            delta_m: vec![0.0; heightmap.width() * heightmap.height()],
            flatten_target_m: None,
            touched: false,
            last_dab: None,
            tangent: None,
        }
    }

    pub fn touched(&self) -> bool {
        self.touched
    }

    /// The effective height at a sample under the live stroke (floor-clamped like compile).
    fn effective(&self, index: usize) -> f32 {
        (self.base[index] + self.delta_m[index]).max(self.min_height_m)
    }

    /// The canonical (south-half) twin of a sample — base reads for base-dependent modes go
    /// through it so both halves of a fair map compute IDENTICAL deltas.
    fn canonical(&self, xi: usize, zi: usize) -> usize {
        let zi = if self.mirrored { zi.min(self.side - 1 - zi) } else { zi };
        zi * self.side + xi
    }

    /// One painting step at the cursor's world point. A fast drag is SPACED, not sampled:
    /// intermediate dabs land along the path every ~third of the radius, splitting the
    /// frame's deposition between them — no machine-gun dots, one continuous stroke at any
    /// mouse speed. A stationary hold keeps depositing at the frame cadence.
    pub fn dab(&mut self, center: [f32; 2], settings: &BrushSettings, dt_s: f32) {
        let spacing = (settings.radius_m * 0.3).max(self.cell_m * 0.5);
        let centers: Vec<[f32; 2]> = match self.last_dab {
            Some(last) => {
                let dx = center[0] - last[0];
                let dz = center[1] - last[1];
                let path = (dx * dx + dz * dz).sqrt();
                if path > self.cell_m * 0.25 {
                    self.tangent = Some([dx / path, dz / path]);
                }
                if path <= spacing {
                    vec![center]
                } else {
                    let count = (path / spacing).ceil() as usize;
                    (1..=count)
                        .map(|step| {
                            let t = step as f32 / count as f32;
                            [last[0] + dx * t, last[1] + dz * t]
                        })
                        .collect()
                }
            }
            None => vec![center],
        };
        self.last_dab = Some(center);
        let dt_each = dt_s / centers.len() as f32;
        for dab_center in centers {
            self.mirrored_dab(dab_center, settings, dt_each);
        }
    }

    /// The dab and — on a mirrored map — its twin. BOTH halves are STAGED against the same
    /// pre-dab state and committed together: a base-dependent mode (Flatten, Smooth,
    /// Terrace, Erode) must never see the south half's fresh writes through its canonical
    /// reads, or the twins' deltas drift apart. The twin's drag tangent mirrors with it
    /// (dz negated), so a Ridge crest and its twin bend the same way about the axis.
    fn mirrored_dab(&mut self, center: [f32; 2], settings: &BrushSettings, dt_s: f32) {
        let tangent = self.tangent;
        let mut staged: Vec<(usize, f32)> = Vec::new();
        self.stage_dab(center, settings, dt_s, tangent, &mut staged);
        if self.mirrored {
            let axis_z = (self.side - 1) as f32 * self.cell_m * 0.5;
            let mirrored_z = axis_z * 2.0 - center[1];
            if (mirrored_z - center[1]).abs() > f32::EPSILON {
                let mirrored_tangent = tangent.map(|[tx, tz]| [tx, -tz]);
                self.stage_dab(
                    [center[0], mirrored_z],
                    settings,
                    dt_s,
                    mirrored_tangent,
                    &mut staged,
                );
            }
        }
        for (index, change) in staged {
            self.delta_m[index] += change;
            self.touched = true;
        }
    }

    /// Stage one dab's contributions (no writes): every read sees the pre-dab state.
    fn stage_dab(
        &mut self,
        center: [f32; 2],
        settings: &BrushSettings,
        dt_s: f32,
        tangent: Option<[f32; 2]>,
        staged: &mut Vec<(usize, f32)>,
    ) {
        let radius_cells = (settings.radius_m / self.cell_m).ceil() as isize;
        let cx = (center[0] / self.cell_m).round() as isize;
        let cz = (center[1] / self.cell_m).round() as isize;
        if self.flatten_target_m.is_none() && settings.mode == BrushMode::Flatten {
            let index = self.canonical(
                cx.clamp(0, self.side as isize - 1) as usize,
                cz.clamp(0, self.side as isize - 1) as usize,
            );
            self.flatten_target_m = Some(self.effective(index));
        }
        for zi in (cz - radius_cells).max(0)..=(cz + radius_cells).min(self.side as isize - 1) {
            for xi in (cx - radius_cells).max(0)..=(cx + radius_cells).min(self.side as isize - 1) {
                let (xi, zi) = (xi as usize, zi as usize);
                let dx = xi as f32 * self.cell_m - center[0];
                let dz = zi as f32 * self.cell_m - center[1];
                let distance = (dx * dx + dz * dz).sqrt();
                if distance > settings.radius_m {
                    continue;
                }
                // Ridge with a drag tangent swaps the radial falloff for an anisotropic
                // crest weight: tight across the line (0.35 r), full along it.
                let falloff = match (settings.mode, tangent) {
                    (BrushMode::Ridge, Some([tx, tz])) => {
                        let along = (dx * tx + dz * tz).abs();
                        let across = (dx * tz - dz * tx).abs();
                        smoothstep01(1.0 - across / (settings.radius_m * 0.35))
                            * smoothstep01(1.0 - along / settings.radius_m)
                    }
                    _ => smoothstep01(1.0 - distance / settings.radius_m),
                };
                let border = self.border_fade(xi, zi);
                if border <= 0.0 || falloff <= 0.0 {
                    continue;
                }
                let index = zi * self.side + xi;
                let canonical = self.canonical(xi, zi);
                let change = match settings.mode {
                    BrushMode::Raise | BrushMode::Ridge => settings.rate_m_s * dt_s,
                    BrushMode::Lower => -settings.rate_m_s * dt_s,
                    BrushMode::Flatten => {
                        let target = self.flatten_target_m.unwrap_or(self.base[canonical]);
                        (target - self.effective(canonical))
                            * (settings.rate_m_s * dt_s * 0.5).min(1.0)
                    }
                    BrushMode::Smooth => {
                        (self.neighbour_mean(canonical) - self.effective(canonical))
                            * (settings.rate_m_s * dt_s * 0.5).min(1.0)
                    }
                    BrushMode::Terrace => {
                        let step = settings.terrace_step_m.max(0.5);
                        let current = self.effective(canonical);
                        let target = (current / step).round() * step;
                        (target - current) * (settings.rate_m_s * dt_s * 0.5).min(1.0)
                    }
                    BrushMode::Erode => {
                        let diff = self.wide_mean(canonical) - self.effective(canonical);
                        let bias = if diff < 0.0 { 1.15 } else { 0.85 };
                        diff * (settings.rate_m_s * dt_s * 0.5).min(1.0) * bias
                    }
                };
                staged.push((index, change * falloff * border));
            }
        }
    }

    /// The neighbour average in canonical space (base-dependent, so canonical reads).
    fn neighbour_mean(&self, index: usize) -> f32 {
        let (xi, zi) = (index % self.side, index / self.side);
        let mut sum = 0.0;
        let mut count = 0.0;
        for (nx, nz) in
            [(xi.wrapping_sub(1), zi), (xi + 1, zi), (xi, zi.wrapping_sub(1)), (xi, zi + 1)]
        {
            if nx < self.side && nz < self.side {
                sum += self.effective(self.canonical(nx, nz));
                count += 1.0;
            }
        }
        if count > 0.0 { sum / count } else { self.effective(index) }
    }

    /// The erosion kernel: the mean over the 8 offsets at a 2-cell ring — wide enough to
    /// see past a crest, canonical reads so both halves relax identically.
    fn wide_mean(&self, index: usize) -> f32 {
        let (xi, zi) = ((index % self.side) as isize, (index / self.side) as isize);
        let mut sum = 0.0;
        let mut count = 0.0;
        for (ox, oz) in [(-2, -2), (-2, 0), (-2, 2), (0, -2), (0, 2), (2, -2), (2, 0), (2, 2)] {
            let (nx, nz) = (xi + ox, zi + oz);
            if nx >= 0 && nz >= 0 && (nx as usize) < self.side && (nz as usize) < self.side {
                sum += self.effective(self.canonical(nx as usize, nz as usize));
                count += 1.0;
            }
        }
        if count > 0.0 { sum / count } else { self.effective(index) }
    }

    /// Zero on the border ring, full strength four cells in — the report's seam contract
    /// (`sculpt` border Error) can never fire on a brush stroke.
    fn border_fade(&self, xi: usize, zi: usize) -> f32 {
        let edge = xi.min(zi).min(self.side - 1 - xi).min(self.side - 1 - zi);
        (edge as f32 / 4.0).min(1.0)
    }

    /// The live heightmap for the viewport's geometry swap during the stroke.
    pub fn working_heightmap(&self) -> HeightMap {
        let side = self.side;
        let cell = self.cell_m;
        let base = &self.base;
        let delta = &self.delta_m;
        let min = self.min_height_m;
        terrain::heightmap_from_fn(side, cell, |x, z| {
            let xi = (x / cell).round() as usize;
            let zi = (z / cell).round() as usize;
            let index = zi * side + xi;
            (base[index] + delta[index]).max(min)
        })
    }

    /// Fold the stroke into the document's sculpt layer: quantize, merge with the existing
    /// samples, drop zeros, keep the canonical sorted form. Returns `None` for a stroke
    /// that changed nothing measurable (no dirty document for a hover).
    pub fn committed(&self, existing: Option<&SculptSpec>) -> Option<SculptSpec> {
        let mut merged: BTreeMap<u32, i32> = BTreeMap::new();
        if let Some(existing) = existing {
            for &(index, quanta) in &existing.samples {
                merged.insert(index, i32::from(quanta));
            }
        }
        let mut changed = false;
        for (index, delta) in self.delta_m.iter().enumerate() {
            let quanta = (delta / self.step_m).round() as i32;
            if quanta != 0 {
                changed = true;
                *merged.entry(index as u32).or_insert(0) += quanta;
            }
        }
        if !changed {
            return None;
        }
        let samples: Vec<(u32, i16)> = merged
            .into_iter()
            .filter(|(_, quanta)| *quanta != 0)
            .map(|(index, quanta)| (index, quanta.clamp(i16::MIN as i32, i16::MAX as i32) as i16))
            .collect();
        Some(SculptSpec { step_m: self.step_m, samples })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use map_forge::blueprint::SymmetrySpec;

    fn scratch() -> (MapBlueprint, HeightMap) {
        let document = crate::EditorDocument::new_scratch();
        let compiled = document.recompile();
        (document.blueprint().clone(), compiled.battlefield.heightmap.clone())
    }

    fn settings(mode: BrushMode) -> BrushSettings {
        BrushSettings { mode, radius_m: 12.0, rate_m_s: 6.0, terrace_step_m: 2.0 }
    }

    #[test]
    fn a_raise_stroke_lands_quantized_and_stacks_onto_the_existing_layer() {
        let (blueprint, heightmap) = scratch();
        let mut stroke = Stroke::begin(&blueprint, &heightmap);
        stroke.dab([150.0, 150.0], &settings(BrushMode::Raise), 1.0);
        assert!(stroke.touched());
        let sculpt = stroke.committed(None).expect("a real stroke commits");
        assert_eq!(sculpt.step_m, DEFAULT_STEP_M);
        assert!(sculpt.samples.windows(2).all(|pair| pair[0].0 < pair[1].0), "canonical order");
        let side = 61_u32;
        let centre = 30 * side + 30;
        let raised = sculpt.samples.iter().find(|(index, _)| *index == centre).expect("centre");
        assert_eq!(raised.1, (6.0_f32 / DEFAULT_STEP_M).round() as i16, "full rate, quantized");

        // A second identical stroke on top of the committed layer doubles the quanta.
        let mut again = Stroke::begin(&blueprint, &heightmap);
        again.dab([150.0, 150.0], &settings(BrushMode::Raise), 1.0);
        let stacked = again.committed(Some(&sculpt)).expect("stacks");
        let doubled = stacked.samples.iter().find(|(index, _)| *index == centre).expect("centre");
        assert_eq!(doubled.1, raised.1 * 2);
    }

    #[test]
    fn a_mirrored_map_gets_bit_identical_twin_stamps_for_every_mode() {
        let (mut blueprint, _) = scratch();
        blueprint.symmetry = Some(SymmetrySpec::MirrorZ);
        let compiled = map_forge::compile(&blueprint).0;
        let side = 61_u32;
        for mode in BrushMode::CYCLE {
            let mut stroke = Stroke::begin(&blueprint, &compiled.heightmap);
            stroke.dab([150.0, 100.0], &settings(mode), 0.7);
            let Some(sculpt) = stroke.committed(None) else { continue };
            for &(index, quanta) in &sculpt.samples {
                let (xi, zi) = (index % side, index / side);
                let mirrored = (side - 1 - zi) * side + xi;
                let twin = sculpt.delta_m_at(mirrored);
                assert_eq!(
                    twin,
                    f32::from(quanta) * sculpt.step_m,
                    "{mode:?}: sample {index} must have an exact twin at {mirrored}"
                );
            }
            // And the compiled result passes the report's sculpt checks.
            blueprint.sculpt = Some(sculpt);
            let (_, report) = map_forge::compile(&blueprint);
            assert!(
                !report.errors().any(|entry| entry.check == "sculpt"),
                "{mode:?}: an editor stroke can never trip the sculpt contract"
            );
            blueprint.sculpt = None;
        }
    }

    #[test]
    fn a_fast_drag_leaves_a_continuous_stroke_not_dots() {
        let (blueprint, heightmap) = scratch();
        let mut stroke = Stroke::begin(&blueprint, &heightmap);
        let raise = BrushSettings {
            mode: BrushMode::Raise,
            radius_m: 8.0,
            rate_m_s: 6.0,
            terrace_step_m: 2.0,
        };
        // Two frames, 90 m apart — far beyond the radius. Spacing must fill the gap.
        stroke.dab([60.0, 150.0], &raise, 0.05);
        stroke.dab([150.0, 150.0], &raise, 0.05);
        let sculpt = stroke.committed(None).expect("commits");
        let side = 61_u32;
        let row = 30_u32;
        // Every column between the two dab centres must carry sculpt on the stroke row.
        for xi in 13..=29 {
            assert!(
                sculpt.samples.iter().any(|(index, _)| *index == row * side + xi),
                "gap at column {xi} — the drag stitched dots instead of a stroke"
            );
        }
    }

    #[test]
    fn a_ridge_drag_builds_a_crest_along_the_drag_not_a_blob_trail() {
        let (blueprint, heightmap) = scratch();
        let mut stroke = Stroke::begin(&blueprint, &heightmap);
        let ridge = settings(BrushMode::Ridge);
        // Drag east along z = 150: the second dab carries the tangent.
        stroke.dab([90.0, 150.0], &ridge, 0.3);
        stroke.dab([150.0, 150.0], &ridge, 0.3);
        let sculpt = stroke.committed(None).expect("commits");
        let side = 61_u32;
        let quanta_at = |xi: u32, zi: u32| {
            sculpt
                .samples
                .iter()
                .find(|(index, _)| *index == zi * side + xi)
                .map_or(0, |(_, quanta)| *quanta)
        };
        // On the drag line the crest stands; 10 m across it there is nothing — the
        // anisotropic weight (0.35 r across) is what separates a line from a blob trail.
        assert!(quanta_at(24, 30) > 0, "the crest rises on the drag line");
        assert!(
            quanta_at(24, 32) == 0,
            "10 m across the line the ridge weight is zero, got {}",
            quanta_at(24, 32)
        );
    }

    #[test]
    fn terrace_pulls_ground_onto_quantized_levels_with_honest_risers() {
        // A sloping base so neighbouring cells round to DIFFERENT levels (risers emerge).
        let (mut blueprint, _) = scratch();
        blueprint.terrain.ops.push(map_forge::blueprint::TerrainOp::Gauss2 {
            apply: map_forge::blueprint::Apply::Add,
            terms: vec![map_forge::blueprint::Gauss2Term {
                x: 150.0,
                z: 150.0,
                sx: 40.0,
                sz: 40.0,
                amp: 5.0,
            }],
        });
        let compiled = map_forge::compile(&blueprint).0;
        let mut stroke = Stroke::begin(&blueprint, &compiled.heightmap);
        let terrace = settings(BrushMode::Terrace);
        for _ in 0..60 {
            stroke.dab([150.0, 150.0], &terrace, 0.2);
        }
        // The held centre converges onto a multiple of the step.
        let index = (30 * 61 + 30) as usize;
        let level = stroke.effective(index) / terrace.terrace_step_m;
        assert!(
            (level - level.round()).abs() < 0.1,
            "the centre sits on a terrace level, got height {}",
            stroke.effective(index)
        );
    }

    #[test]
    fn erode_relaxes_peaks_faster_than_pits() {
        let (blueprint, heightmap) = scratch();
        let index = (30 * 61 + 30) as usize;

        // A raised bump, then one erosion pass at its crest.
        let mut peak = Stroke::begin(&blueprint, &heightmap);
        peak.dab([150.0, 150.0], &settings(BrushMode::Raise), 1.0);
        let before = peak.effective(index);
        peak.dab([150.0, 150.0], &settings(BrushMode::Erode), 0.3);
        let shed = before - peak.effective(index);
        assert!(shed > 0.0, "a peak sheds");

        // The same magnitude pit, same erosion pass.
        let mut pit = Stroke::begin(&blueprint, &heightmap);
        pit.dab([150.0, 150.0], &settings(BrushMode::Lower), 1.0);
        let before = pit.effective(index);
        pit.dab([150.0, 150.0], &settings(BrushMode::Erode), 0.3);
        let filled = pit.effective(index) - before;
        assert!(filled > 0.0, "a pit fills");
        assert!(
            shed > filled * 1.2,
            "peaks shed faster than pits fill (the 1.15/0.85 bias): shed {shed}, filled {filled}"
        );
    }

    #[test]
    fn the_border_ring_stays_zero_and_flatten_pulls_toward_its_anchor() {
        let (blueprint, heightmap) = scratch();
        let mut stroke = Stroke::begin(&blueprint, &heightmap);
        // Paint right into the corner: the fade must keep the ring untouched.
        stroke.dab([2.0, 2.0], &settings(BrushMode::Raise), 1.0);
        let sculpt = stroke.committed(None).expect("commits");
        let side = 61_u32;
        for &(index, _) in &sculpt.samples {
            let (xi, zi) = (index % side, index / side);
            assert!(
                xi > 0 && zi > 0 && xi < side - 1 && zi < side - 1,
                "the border ring must stay zero (index {index})"
            );
        }

        // Flatten: raise a bump first, then flatten from flat ground next to it — the bump
        // must come DOWN toward the anchor height, not average upward.
        let mut bump = Stroke::begin(&blueprint, &heightmap);
        bump.dab([150.0, 150.0], &settings(BrushMode::Raise), 1.0);
        let bumped = bump.committed(None).expect("bump");
        let mut with_bump = blueprint.clone();
        with_bump.sculpt = Some(bumped);
        let compiled = map_forge::compile(&with_bump).0;
        let mut flatten = Stroke::begin(&with_bump, &compiled.heightmap);
        for _ in 0..40 {
            flatten.dab([150.0, 150.0], &settings(BrushMode::Flatten), 0.2);
        }
        let index = (30 * 61 + 30) as usize;
        let flattened = flatten.effective(index);
        let anchor = flatten.flatten_target_m.expect("anchored");
        assert!(
            (flattened - anchor).abs() < 0.4,
            "flatten converges to its anchor: {flattened} vs {anchor}"
        );
    }
}
