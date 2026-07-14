//! True terrain deformation (protocol v31): shell craters as replicated state.
//!
//! The base heightmap NEVER mutates. A battle's craters live in a small replicated ledger
//! (`CraterRecord`, quantized so the server and every client apply bit-identical numbers), and
//! [`CraterField`] folds them into `HeightMap::sample_height` as an analytic overlay. That call
//! is the single seam every consumer already reads — tank contact, shell ground trace, spotting
//! LOS, the client predictor and the bots — so all of them stand in the same hole automatically.
//!
//! Photographic reference for the profile: an HE burst leaves a bowl sunk below grade with a
//! RAISED rim of displaced spoil just past the lip, falling back to the untouched field. The
//! bowl is `depth * (1 - t^2)^2` (flat-bottomed, tangent to grade at the lip) and the rim is a
//! low bump peaking at 1.25 radii, gone entirely by 1.5.

use serde::{Deserialize, Serialize};

/// Ground-position quantization step. 0.25 m keeps a u16 good for a 16 km map edge.
pub const CRATER_POSITION_STEP_M: f32 = 0.25;
/// Radius quantization step: u8 covers up to 25.5 m.
pub const CRATER_RADIUS_STEP_M: f32 = 0.1;
/// Depth quantization step: u8 covers up to 12.75 m.
pub const CRATER_DEPTH_STEP_M: f32 = 0.05;
/// A crater's ground influence ends at this multiple of its radius (past the rim).
pub const CRATER_INFLUENCE_FACTOR: f32 = 1.5;
/// Rim height as a fraction of bowl depth — displaced spoil is a low bank, not a wall.
pub const CRATER_RIM_FRACTION: f32 = 0.25;

/// A high-explosive ground burst (the only kind so far).
pub const CRATER_KIND_HIGH_EXPLOSIVE: u8 = 0;

/// One replicated crater. Stored QUANTIZED — the sim quantizes at append time and both sides
/// dequantize the same integers, so predictor↔server height parity is exact by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CraterRecord {
    pub x_q: u16,
    pub z_q: u16,
    pub radius_q: u8,
    pub depth_q: u8,
    pub kind: u8,
}

impl CraterRecord {
    pub fn from_world(x_m: f32, z_m: f32, radius_m: f32, depth_m: f32, kind: u8) -> Self {
        Self {
            x_q: quantize_u16(x_m, CRATER_POSITION_STEP_M),
            z_q: quantize_u16(z_m, CRATER_POSITION_STEP_M),
            radius_q: quantize_u8(radius_m, CRATER_RADIUS_STEP_M).max(1),
            depth_q: quantize_u8(depth_m, CRATER_DEPTH_STEP_M).max(1),
            kind,
        }
    }

    pub fn x_m(&self) -> f32 {
        self.x_q as f32 * CRATER_POSITION_STEP_M
    }

    pub fn z_m(&self) -> f32 {
        self.z_q as f32 * CRATER_POSITION_STEP_M
    }

    pub fn radius_m(&self) -> f32 {
        self.radius_q as f32 * CRATER_RADIUS_STEP_M
    }

    pub fn depth_m(&self) -> f32 {
        self.depth_q as f32 * CRATER_DEPTH_STEP_M
    }

    pub fn influence_radius_m(&self) -> f32 {
        self.radius_m() * CRATER_INFLUENCE_FACTOR
    }

    /// Signed height change this crater alone contributes at planar distance `dist_m` from its
    /// center: the sunk bowl inside the radius, the raised spoil rim just past it, zero beyond.
    pub fn height_delta_at(&self, dist_m: f32) -> f32 {
        let radius = self.radius_m();
        let t = dist_m / radius;
        if t < 1.0 {
            let s = 1.0 - t * t;
            -self.depth_m() * s * s
        } else if t < CRATER_INFLUENCE_FACTOR {
            // A parabolic bump: zero at the lip and at the influence edge, peaking halfway.
            let w = (t - 1.0) / (CRATER_INFLUENCE_FACTOR - 1.0);
            self.depth_m() * CRATER_RIM_FRACTION * 4.0 * w * (1.0 - w)
        } else {
            0.0
        }
    }
}

fn quantize_u16(value_m: f32, step_m: f32) -> u16 {
    (value_m / step_m).round().clamp(0.0, u16::MAX as f32) as u16
}

fn quantize_u8(value_m: f32, step_m: f32) -> u8 {
    (value_m / step_m).round().clamp(0.0, u8::MAX as f32) as u8
}

/// Bucket cell size for the spatial grid. Bigger than any crater's influence, so a query only
/// ever touches its own cell and the three neighbours toward the crater's center.
const BUCKET_CELL_M: f32 = 16.0;

/// The height overlay: the current ledger plus a bucket grid over the map extent, so the common
/// case — a sample nowhere near any crater — costs one empty-bucket check.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CraterField {
    records: Vec<CraterRecord>,
    cols: usize,
    rows: usize,
    buckets: Vec<Vec<u8>>,
}

impl CraterField {
    /// Replace the ledger (idempotent: same records, same field). Called by the battlefield
    /// owner whenever the replicated ledger changes; with ≤ 64 tiny records the rebuild is
    /// cheap enough to run on any snapshot that moved it.
    pub fn rebuild(&mut self, records: &[CraterRecord], extent_m: [f32; 2]) {
        if self.records == records {
            return;
        }
        self.records = records.to_vec();
        self.cols = (extent_m[0] / BUCKET_CELL_M).ceil().max(1.0) as usize;
        self.rows = (extent_m[1] / BUCKET_CELL_M).ceil().max(1.0) as usize;
        self.buckets = vec![Vec::new(); self.cols * self.rows];
        for (index, record) in self.records.iter().enumerate() {
            let reach = record.influence_radius_m();
            let col_lo = self.col_at(record.x_m() - reach);
            let col_hi = self.col_at(record.x_m() + reach);
            let row_lo = self.row_at(record.z_m() - reach);
            let row_hi = self.row_at(record.z_m() + reach);
            for row in row_lo..=row_hi {
                for col in col_lo..=col_hi {
                    self.buckets[row * self.cols + col].push(index as u8);
                }
            }
        }
    }

    pub fn records(&self) -> &[CraterRecord] {
        &self.records
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Total signed height change at a ground point. Overlapping craters sum — repeated
    /// shelling digs deeper — which the ledger's merge rule keeps bounded.
    ///
    /// Deliberately `inline(never)`: `sample_height` is THE hot call of the whole sim (contact,
    /// shell trace, spotting LOS), and keeping the crater math out of its body keeps the
    /// virgin-ground fast path as small as it was before deformation existed.
    #[inline(never)]
    pub fn delta(&self, x_m: f32, z_m: f32) -> f32 {
        if self.records.is_empty() {
            return 0.0;
        }
        let bucket = &self.buckets[self.row_at(z_m) * self.cols + self.col_at(x_m)];
        let mut delta = 0.0;
        for &index in bucket {
            let record = &self.records[index as usize];
            let dx = x_m - record.x_m();
            let dz = z_m - record.z_m();
            let dist = (dx * dx + dz * dz).sqrt();
            if dist < record.influence_radius_m() {
                delta += record.height_delta_at(dist);
            }
        }
        delta
    }

    fn col_at(&self, x_m: f32) -> usize {
        ((x_m / BUCKET_CELL_M).floor().max(0.0) as usize).min(self.cols - 1)
    }

    fn row_at(&self, z_m: f32) -> usize {
        ((z_m / BUCKET_CELL_M).floor().max(0.0) as usize).min(self.rows - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HeightMap;

    fn flat_with_crater() -> (HeightMap, CraterRecord) {
        let mut map = HeightMap::flat(64, 64, 4.0, 10.0).expect("flat map");
        let record = CraterRecord::from_world(100.0, 100.0, 2.2, 0.8, CRATER_KIND_HIGH_EXPLOSIVE);
        map.set_craters(&[record]);
        (map, record)
    }

    #[test]
    fn a_crater_digs_a_bowl_and_raises_a_rim_of_spoil() {
        let (map, record) = flat_with_crater();
        let center = map.sample_height(record.x_m(), record.z_m()).expect("center");
        assert!(
            (center - (10.0 - record.depth_m())).abs() < 1e-4,
            "the bowl floor sits a full depth below grade: {center}"
        );
        let rim_peak = record.x_m() + record.radius_m() * 1.25;
        let rim = map.sample_height(rim_peak, record.z_m()).expect("rim");
        assert!(rim > 10.0 + record.depth_m() * 0.2, "the rim rises above grade: {rim}");
        let beyond = record.x_m() + record.influence_radius_m() + 0.01;
        let untouched = map.sample_height(beyond, record.z_m()).expect("field");
        assert_eq!(untouched, 10.0, "past the influence edge the field is EXACTLY as authored");
    }

    #[test]
    fn the_overlay_is_a_pure_function_of_the_ledger() {
        let (map, record) = flat_with_crater();
        // A second map given the same replicated records answers bit-identically everywhere —
        // this is the predictor↔server parity the quantized wire guarantees.
        let mut twin = HeightMap::flat(64, 64, 4.0, 10.0).expect("flat map");
        twin.set_craters(&[record]);
        for step in 0..64 {
            let x = 92.0 + step as f32 * 0.25;
            assert_eq!(map.sample_height(x, 100.3), twin.sample_height(x, 100.3));
        }
        // And clearing the ledger restores the authored ground exactly.
        let mut cleared = map.clone();
        cleared.set_craters(&[]);
        assert_eq!(cleared.sample_height(100.0, 100.0), Some(10.0));
    }

    #[test]
    fn crater_records_quantize_onto_the_shared_wire_grid() {
        let record = CraterRecord::from_world(101.13, 202.31, 2.24, 0.77, 0);
        assert!((record.x_m() - 101.25).abs() < 1e-4);
        assert!((record.z_m() - 202.25).abs() < 1e-4);
        assert!((record.radius_m() - 2.2).abs() < 1e-4);
        assert!((record.depth_m() - 0.75).abs() < 1e-4);
    }

    #[test]
    fn baked_map_assets_never_carry_battle_craters() {
        let (map, _) = flat_with_crater();
        let bytes = bincode::serialize(&map).expect("serialize");
        let reloaded: HeightMap = bincode::deserialize(&bytes).expect("deserialize");
        assert!(reloaded.crater_records().is_empty(), "craters are battle state, not map data");
        assert_eq!(reloaded.sample_height(100.0, 100.0), Some(10.0));
    }
}
