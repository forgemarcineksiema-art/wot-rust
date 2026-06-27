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
}

impl HeightMap {
    pub fn new(
        width: usize,
        height: usize,
        cell_size_m: f32,
        samples: Vec<f32>,
    ) -> Result<Self, TerrainError> {
        validate_heightmap(width, height, cell_size_m, samples.len())?;
        Ok(Self { width, height, cell_size_m, samples })
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
        let hx0 = lerp(self.sample_at_index(x0, z0), self.sample_at_index(x1, z0), tx);
        let hx1 = lerp(self.sample_at_index(x0, z1), self.sample_at_index(x1, z1), tx);
        Some(lerp(hx0, hx1, tz))
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

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
