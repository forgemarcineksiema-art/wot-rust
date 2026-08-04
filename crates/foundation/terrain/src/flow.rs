//! Where the map's water WANTS to go — the drainage truth under the moisture rule.
//!
//! The heightfield already decides where rain runs: every cell has a steepest downhill
//! neighbour, and following those arrows accumulates catchment area the way a real slope
//! feeds a real gully. This module computes that once, at classifier construction, as a
//! [`FlowField`] — a per-cell wetness in `[0, 1]` that [`crate::GroundClassifier`] folds
//! into the ground rule. One rule, both readers: the splat bake darkens the drainage lines
//! the drive drags through (docs/atmosphere-policy.md, "structural ground moisture").
//!
//! Flow is GEOLOGY, not battle state. It is derived from the authored samples the moment
//! the classifier is built and never recomputed: a crater is five minutes old, a drainage
//! line is ten thousand years old. (`HeightMap::samples` is exactly the crater-free lane.)
//!
//! Design decisions that carry the numbers:
//! * **D8, no pit filling.** A local minimum keeps everything it collects — those basins
//!   are precisely the wet niecki the tone wants dark. Plateaus drain nowhere and stay dry.
//! * **Deterministic order everywhere.** Receiver ties break on a fixed neighbour order;
//!   accumulation processes nodes sorted by `f32::total_cmp` height with an index
//!   tie-break, so the same map yields the same field on every machine — the classifier
//!   feeds physics, and physics feeds the lockstep.
//! * **Catchment ramps on a log scale between two areas.** Accumulation is power-law
//!   distributed: linear normalization would leave everything but the trunk stream at
//!   zero, and counting a cell's own area would wet every hillside. So a cell is dry
//!   until its EXTERNAL catchment beats [`CATCHMENT_DRY_M2`], and reads fully wet at
//!   [`CATCHMENT_WET_M2`] — a gully needs a real hillside behind it before it darkens.

use crate::heightmap::HeightMap;

/// External catchment below this stays dry. Sized so the parallel-strip artifact of D8 on
/// a uniform slope (a 1-cell column accumulating its whole uphill run) does not wet an
/// ordinary hillside: 1500 m² is a 60-cell column at 5 m cells — most open slopes on a
/// 1000 m map feed less into any single cell.
const CATCHMENT_DRY_M2: f32 = 1500.0;

/// External catchment that reads fully wet: ~2.4 ha behind one cell is a genuine draw.
const CATCHMENT_WET_M2: f32 = 24_000.0;

/// The eight D8 neighbours, in the FIXED order that breaks steepest-descent ties — part of
/// the determinism contract, never reordered.
const NEIGHBOURS: [(isize, isize); 8] =
    [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// Per-cell drainage wetness in `[0, 1]`, on the heightmap's own grid.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowField {
    width: usize,
    height: usize,
    cell_m: f32,
    wetness: Vec<f32>,
}

impl FlowField {
    /// Compute the field from the authored samples (crater-free by construction).
    pub fn from_heightmap(map: &HeightMap) -> Self {
        let width = map.width();
        let height = map.height();
        let cell_m = map.cell_size_m();
        let count = width * height;

        // Receiver per node: the steepest strictly-downhill D8 neighbour, or a sink.
        const SINK: u32 = u32::MAX;
        let mut receiver = vec![SINK; count];
        for iz in 0..height {
            for ix in 0..width {
                if let Some((rx, rz)) = d8_receiver(map, ix, iz) {
                    receiver[iz * width + ix] = (rz * width + rx) as u32;
                }
            }
        }

        // Accumulate top-down. A receiver is strictly lower than its sender (slope > 0),
        // so descending height order processes every sender before its receiver. The sort
        // key is total_cmp + index: bit-stable across platforms, unique per node.
        let mut order: Vec<u32> = (0..count as u32).collect();
        order.sort_unstable_by(|&a, &b| {
            let ha = map.samples()[a as usize];
            let hb = map.samples()[b as usize];
            hb.total_cmp(&ha).then(a.cmp(&b))
        });
        let mut accumulation = vec![1.0f32; count];
        for &node in &order {
            let target = receiver[node as usize];
            if target != SINK {
                accumulation[target as usize] += accumulation[node as usize];
            }
        }

        let cell_area = cell_m * cell_m;
        let full_span = (CATCHMENT_WET_M2 / CATCHMENT_DRY_M2).ln();
        let wetness = accumulation
            .iter()
            .map(|&cells| {
                let catchment_m2 = (cells - 1.0) * cell_area;
                if catchment_m2 <= CATCHMENT_DRY_M2 {
                    0.0
                } else {
                    ((catchment_m2 / CATCHMENT_DRY_M2).ln() / full_span).clamp(0.0, 1.0)
                }
            })
            .collect();

        Self { width, height, cell_m, wetness }
    }

    /// Wetness at a world position — bilinear over the cell grid, clamped at the edges so
    /// callers just outside the heightfield read the border value instead of falling dry.
    pub fn wetness_at(&self, x_m: f32, z_m: f32) -> f32 {
        let grid_x = (x_m / self.cell_m).clamp(0.0, (self.width - 1) as f32);
        let grid_z = (z_m / self.cell_m).clamp(0.0, (self.height - 1) as f32);
        let x0 = (grid_x.floor() as usize).min(self.width - 2);
        let z0 = (grid_z.floor() as usize).min(self.height - 2);
        let tx = grid_x - x0 as f32;
        let tz = grid_z - z0 as f32;
        let at = |x: usize, z: usize| self.wetness[z * self.width + x];
        let top = at(x0, z0) + (at(x0 + 1, z0) - at(x0, z0)) * tx;
        let bottom = at(x0, z0 + 1) + (at(x0 + 1, z0 + 1) - at(x0, z0 + 1)) * tx;
        top + (bottom - top) * tz
    }

    /// Raw wetness at a grid node (tests and instruments; render/physics read `wetness_at`).
    pub fn wetness_at_index(&self, ix: usize, iz: usize) -> f32 {
        self.wetness[iz * self.width + ix]
    }
}

/// The steepest strictly-downhill D8 neighbour of a node, `None` for sinks. Diagonal drops
/// are discounted by their longer run; ties keep the FIRST candidate in [`NEIGHBOURS`]
/// order (strict `>` below), which is what makes the arrows deterministic.
fn d8_receiver(map: &HeightMap, ix: usize, iz: usize) -> Option<(usize, usize)> {
    let here = map.sample_at_index(ix, iz);
    let mut best: Option<((usize, usize), f32)> = None;
    for (dx, dz) in NEIGHBOURS {
        let nx = ix as isize + dx;
        let nz = iz as isize + dz;
        if nx < 0 || nz < 0 || nx as usize >= map.width() || nz as usize >= map.height() {
            continue;
        }
        let run = if dx != 0 && dz != 0 { std::f32::consts::SQRT_2 } else { 1.0 };
        let drop = (here - map.sample_at_index(nx as usize, nz as usize)) / run;
        if drop > 0.0 && best.is_none_or(|(_, steepest)| drop > steepest) {
            best = Some(((nx as usize, nz as usize), drop));
        }
    }
    best.map(|(node, _)| node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_build::heightmap_from_fn;

    /// A V-shaped valley running along z — the analytic map every assertion below reads.
    /// The cross-slope (0.08) beats the down-valley slope (0.01) everywhere, so shoulders
    /// genuinely CONVERGE into the trough instead of running parallel columns — the same
    /// convergence real drainage needs before it earns wetness.
    fn valley_map() -> HeightMap {
        heightmap_from_fn(101, 5.0, |x, z| 20.0 + (x - 250.0).abs() * 0.08 - z * 0.01)
    }

    fn bowl_map() -> HeightMap {
        heightmap_from_fn(101, 5.0, |x, z| {
            let dx = x - 250.0;
            let dz = z - 250.0;
            (dx * dx + dz * dz) * 0.001
        })
    }

    #[test]
    fn the_field_is_deterministic() {
        let map = valley_map();
        assert_eq!(FlowField::from_heightmap(&map), FlowField::from_heightmap(&map));
    }

    #[test]
    fn receivers_point_strictly_downhill() {
        let map = valley_map();
        for iz in 0..map.height() {
            for ix in 0..map.width() {
                if let Some((rx, rz)) = d8_receiver(&map, ix, iz) {
                    assert!(
                        map.sample_at_index(rx, rz) < map.sample_at_index(ix, iz),
                        "receiver of ({ix},{iz}) must sit lower"
                    );
                }
            }
        }
    }

    #[test]
    fn wetness_never_decreases_downstream() {
        // Wetness is a monotone function of catchment, and catchment only grows along the
        // receiver arrows — walk every chain and demand it.
        let map = valley_map();
        let field = FlowField::from_heightmap(&map);
        for iz in 0..map.height() {
            for ix in 0..map.width() {
                if let Some((rx, rz)) = d8_receiver(&map, ix, iz) {
                    assert!(
                        field.wetness_at_index(rx, rz) >= field.wetness_at_index(ix, iz),
                        "flow dried up moving downstream from ({ix},{iz}) to ({rx},{rz})"
                    );
                }
            }
        }
    }

    #[test]
    fn the_valley_trough_out_wets_its_shoulders_and_the_crests_stay_dry() {
        let map = valley_map();
        let field = FlowField::from_heightmap(&map);
        // Deep in the run the trough has collected the whole slope; the mid-shoulder has
        // only its own thin strip; the outer crest sheds everything and keeps nothing.
        let trough = field.wetness_at(250.0, 400.0);
        let shoulder = field.wetness_at(125.0, 400.0);
        let crest = field.wetness_at(0.0, 400.0);
        assert!(trough > 0.5, "the trough is the drainage line (got {trough})");
        assert!(trough > shoulder, "the trough must out-wet its shoulder ({trough} vs {shoulder})");
        assert_eq!(crest, 0.0, "a crest collects nothing");
    }

    #[test]
    fn a_basin_floor_collects_everything() {
        // No pit filling by design: the bowl's minimum keeps its whole catchment.
        let map = bowl_map();
        let field = FlowField::from_heightmap(&map);
        assert_eq!(field.wetness_at(250.0, 250.0), 1.0, "a 500 m bowl saturates its floor");
        assert_eq!(field.wetness_at(10.0, 10.0), 0.0, "the rim stays dry");
    }

    #[test]
    fn wetness_lookup_clamps_at_the_edges() {
        let map = valley_map();
        let field = FlowField::from_heightmap(&map);
        assert_eq!(field.wetness_at(-40.0, 400.0), field.wetness_at(0.0, 400.0));
        assert_eq!(field.wetness_at(250.0, 9999.0), field.wetness_at(250.0, 500.0));
    }
}
