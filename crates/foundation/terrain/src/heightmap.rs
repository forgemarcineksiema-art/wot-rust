use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TerrainError {
    #[error("heightmap dimensions must be at least 2x2")]
    DimensionsTooSmall,
    #[error("heightmap sample count does not match dimensions")]
    SampleCountMismatch,
    #[error("heightmap cell size must be positive")]
    InvalidCellSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HeightMapStats {
    pub min_m: f32,
    pub max_m: f32,
    pub average_m: f32,
}

impl HeightMapStats {
    pub fn range_m(self) -> f32 {
        self.max_m - self.min_m
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeightMap {
    width: usize,
    height: usize,
    cell_size_m: f32,
    samples: Vec<f32>,
    /// The battle's crater overlay (protocol v31). The authored `samples` above NEVER mutate;
    /// craters are transient battle state folded into `sample_height`, and `serde(skip)` keeps
    /// them out of baked map assets. The battlefield owner (server loop / client ingest) syncs
    /// this from the replicated ledger via [`Self::set_craters`].
    #[serde(skip)]
    craters: crate::craters::CraterField,
}

/// Which diagonal a cell's two drawn triangles share: `true` = the MAIN diagonal
/// (h00-h11), `false` = the ANTI diagonal (h10-h01). ONE rule for the sampler above and
/// the render mesh (`scene_build::terrain_scene_mesh_full`), chosen per cell as the
/// diagonal with the smaller height difference — the standard fidelity heuristic, and,
/// unlike a fixed split, SYMMETRIC under the mirror a fair map is built on: mirroring
/// swaps the two corner pairs, so the rule picks the same geometric diagonal on both
/// halves and mirrored interior samples stay bit-equal (the fixed anti-diagonal broke
/// exactly that, which the editor's fairness locks caught). Ties pick the main diagonal;
/// a tie with nonzero twist is the one spot mirror equality can still slip, and authored
/// terrain does not produce those twins.
#[inline]
pub fn cell_splits_on_main_diagonal(h00: f32, h10: f32, h01: f32, h11: f32) -> bool {
    (h00 - h11).abs() <= (h10 - h01).abs()
}

impl HeightMap {
    pub fn new(
        width: usize,
        height: usize,
        cell_size_m: f32,
        samples: Vec<f32>,
    ) -> Result<Self, TerrainError> {
        validate_heightmap(width, height, cell_size_m, samples.len())?;
        Ok(Self {
            width,
            height,
            cell_size_m,
            samples,
            craters: crate::craters::CraterField::default(),
        })
    }

    pub fn flat(
        width: usize,
        height: usize,
        cell_size_m: f32,
        height_m: f32,
    ) -> Result<Self, TerrainError> {
        Self::new(width, height, cell_size_m, vec![height_m; width * height])
    }

    pub fn extent_m(&self) -> [f32; 2] {
        [(self.width - 1) as f32 * self.cell_size_m, (self.height - 1) as f32 * self.cell_size_m]
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn cell_size_m(&self) -> f32 {
        self.cell_size_m
    }

    /// `inline` is measured, not decorative: the sim's hot loops (contact, shell ground trace,
    /// spotting LOS) live across the crate boundary, and inlining the fast path buys back the
    /// crater-overlay branch below with interest (see the combat_hot_path bench).
    #[inline]
    pub fn sample_height(&self, x_m: f32, z_m: f32) -> Option<f32> {
        let grid_x = x_m / self.cell_size_m;
        let grid_z = z_m / self.cell_size_m;
        // Valid domain is the closed interval [0, extent] == grid [0, n-1].
        // `contains` also rejects NaN, since any comparison with NaN is false.
        if !(0.0..=(self.width - 1) as f32).contains(&grid_x)
            || !(0.0..=(self.height - 1) as f32).contains(&grid_z)
        {
            return None;
        }

        // Clamp the lower corner so the exact far edge (grid == n-1) still samples
        // the last cell (with tx/tz == 1.0) instead of falling off the grid.
        let x0 = (grid_x.floor() as usize).min(self.width - 2);
        let z0 = (grid_z.floor() as usize).min(self.height - 2);
        let x1 = x0 + 1;
        let z1 = z0 + 1;

        let tx = grid_x - x0 as f32;
        let tz = grid_z - z0 as f32;
        // The sampled surface IS the drawn surface: each cell splits into the two
        // triangles the render mesh draws (`scene_build::terrain_scene_mesh_full` picks
        // its diagonal with the same [`cell_splits_on_main_diagonal`] rule), so the
        // ground a shell resolves against, the ground a sight line grazes and the ground
        // the eye reads are ONE surface by construction. Bilinear interpolation was
        // retired for exactly that seam: inside a twisted cell the two definitions
        // diverged by up to 0.48 m on the shipped maps (the map-atlas ground-parity
        // measurement), and "what blocks the shell blocks the eye" is a promise about
        // this residual. Cell edges evaluate identically to bilinear, so cross-cell
        // continuity and every edge-sampled value are untouched.
        let h00 = self.sample_at_index(x0, z0);
        let h10 = self.sample_at_index(x1, z0);
        let h01 = self.sample_at_index(x0, z1);
        let h11 = self.sample_at_index(x1, z1);
        let base = if cell_splits_on_main_diagonal(h00, h10, h01, h11) {
            if tx >= tz {
                h00 + tx * (h10 - h00) + tz * (h11 - h10)
            } else {
                h00 + tz * (h01 - h00) + tx * (h11 - h01)
            }
        } else if tx + tz <= 1.0 {
            h00 + tx * (h10 - h00) + tz * (h01 - h00)
        } else {
            h11 + (1.0 - tx) * (h01 - h11) + (1.0 - tz) * (h10 - h11)
        };
        // The crater overlay (protocol v31). The empty-ledger check is a plain length load so
        // virgin ground — the overwhelming majority of samples in the hot loops — pays one
        // predictable branch; the deformed path is deliberately out-of-line (see `delta`).
        if self.craters.is_empty() {
            return Some(base);
        }
        Some(base + self.craters.delta(x_m, z_m))
    }

    /// Whether any crater has touched this battlefield yet.
    pub fn has_craters(&self) -> bool {
        !self.craters.is_empty()
    }

    /// Sync the crater overlay from the replicated ledger (idempotent — an unchanged ledger is
    /// a cheap compare-and-return). This is the ONLY seam true deformation enters through:
    /// every `sample_height` consumer — physics, spotting, predictor, bots — sees it at once.
    pub fn set_craters(&mut self, records: &[crate::craters::CraterRecord]) {
        let extent = self.extent_m();
        self.craters.rebuild(records, extent);
    }

    pub fn crater_records(&self) -> &[crate::craters::CraterRecord] {
        self.craters.records()
    }

    pub fn sample_at_index(&self, x: usize, z: usize) -> f32 {
        self.samples[z * self.width + x]
    }

    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    pub fn stats(&self) -> HeightMapStats {
        let mut min_m = f32::INFINITY;
        let mut max_m = f32::NEG_INFINITY;
        let mut sum = 0.0;
        for sample in &self.samples {
            min_m = min_m.min(*sample);
            max_m = max_m.max(*sample);
            sum += *sample;
        }
        HeightMapStats { min_m, max_m, average_m: sum / self.samples.len() as f32 }
    }
}

fn validate_heightmap(
    width: usize,
    height: usize,
    cell_size_m: f32,
    sample_count: usize,
) -> Result<(), TerrainError> {
    if width < 2 || height < 2 {
        return Err(TerrainError::DimensionsTooSmall);
    }
    if cell_size_m <= 0.0 {
        return Err(TerrainError::InvalidCellSize);
    }
    if sample_count != width * height {
        return Err(TerrainError::SampleCountMismatch);
    }
    Ok(())
}
