//! Ruts with MEMORY (the one program's T8). `GroundProperties::rut_depth_m` said how deep a
//! surface remembers a track and only the mark's opacity read it: a column of tanks left a
//! road that faded in a minute. The rut field is the client's own ledger of where tracks have
//! pressed soft ground — no gameplay, no wire, never folded into `sample_height` — and the
//! ground mesh reads it: every pass presses a little more, up to the softest ground's memory,
//! and the trough stays as long as the battle does. Capped per battle; the oldest press is
//! recycled past the cap.

use std::collections::BTreeSet;

/// One pass of one track presses this much into soft ground.
pub const RUT_PASS_DEPTH_M: f32 = 0.03;
/// Half the width of a track's rut.
pub const RUT_HALF_WIDTH_M: f32 = 0.30;
/// Presses the field remembers at once; the oldest is recycled past this.
pub const MAX_RUT_SEGMENTS: usize = 2048;

/// One press: a track's run between two ground points, how deep this pass pressed, and the
/// deepest the ground under it will ever go.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RutSegment {
    pub from: [f32; 2],
    pub to: [f32; 2],
    pub half_width_m: f32,
    pub depth_m: f32,
    pub cap_m: f32,
}

impl RutSegment {
    fn bounds(&self) -> [f32; 4] {
        [
            self.from[0].min(self.to[0]) - self.half_width_m,
            self.from[1].min(self.to[1]) - self.half_width_m,
            self.from[0].max(self.to[0]) + self.half_width_m,
            self.from[1].max(self.to[1]) + self.half_width_m,
        ]
    }

    /// Planar distance from a point to the press's run.
    fn distance_to(&self, x: f32, z: f32) -> f32 {
        let (ax, az) = (self.from[0], self.from[1]);
        let (dx, dz) = (self.to[0] - ax, self.to[1] - az);
        let len_sq = dx * dx + dz * dz;
        let t = if len_sq <= 1.0e-9 {
            0.0
        } else {
            (((x - ax) * dx + (z - az) * dz) / len_sq).clamp(0.0, 1.0)
        };
        let (px, pz) = (ax + dx * t, az + dz * t);
        ((x - px) * (x - px) + (z - pz) * (z - pz)).sqrt()
    }
}

/// The client's ledger of pressed ground.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RutField {
    segments: Vec<RutSegment>,
    next: usize,
}

impl RutField {
    /// Press one track run into ground that remembers `cap_m` at most (its `rut_depth_m`);
    /// ground that remembers nothing takes no press and spends no budget.
    pub fn press(&mut self, from: [f32; 2], to: [f32; 2], cap_m: f32) {
        if cap_m < 0.01 {
            return;
        }
        let segment = RutSegment {
            from,
            to,
            half_width_m: RUT_HALF_WIDTH_M,
            depth_m: RUT_PASS_DEPTH_M.min(cap_m),
            cap_m,
        };
        if self.segments.len() < MAX_RUT_SEGMENTS {
            self.segments.push(segment);
        } else {
            self.segments[self.next] = segment;
            self.next = (self.next + 1) % MAX_RUT_SEGMENTS;
        }
    }

    pub fn segments(&self) -> &[RutSegment] {
        &self.segments
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// The rut's depth at a point: every pass over it adds its press across the track's
    /// width, capped by the deepest memory of the ground pressed there — a column deepens a
    /// rut; a road of them never digs a trench.
    pub fn depth_at(&self, x: f32, z: f32) -> f32 {
        let mut sum = 0.0f32;
        let mut cap = 0.0f32;
        for segment in &self.segments {
            let [x0, z0, x1, z1] = segment.bounds();
            if x < x0 || x > x1 || z < z0 || z > z1 {
                continue;
            }
            let distance = segment.distance_to(x, z);
            if distance >= segment.half_width_m {
                continue;
            }
            let across = distance / segment.half_width_m;
            sum += segment.depth_m * (1.0 - across * across);
            cap = cap.max(segment.cap_m);
        }
        sum.min(cap)
    }

    /// The base-grid cells (of `cell_m`, indices up to `max_x`/`max_z` inclusive) any press
    /// touches — the ground bakes them at patch resolution so the trough can show.
    pub fn touched_cells(
        &self,
        cell_m: f32,
        max_x: usize,
        max_z: usize,
    ) -> BTreeSet<(usize, usize)> {
        let mut cells = BTreeSet::new();
        for segment in &self.segments {
            let [x0, z0, x1, z1] = segment.bounds();
            let lo_x = ((x0 / cell_m).floor().max(0.0) as usize).min(max_x);
            let hi_x = ((x1 / cell_m).floor().max(0.0) as usize).min(max_x);
            let lo_z = ((z0 / cell_m).floor().max(0.0) as usize).min(max_z);
            let hi_z = ((z1 / cell_m).floor().max(0.0) as usize).min(max_z);
            for z in lo_z..=hi_z {
                for x in lo_x..=hi_x {
                    cells.insert((x, z));
                }
            }
        }
        cells
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T8: a pass presses a little, a column presses to the ground's memory and no further,
    /// the press is a trough across the track's width and nothing beyond it, rock takes
    /// nothing, and the ledger recycles its oldest past the cap.
    #[test]
    fn passes_add_up_to_the_grounds_memory_and_no_further() {
        let mut field = RutField::default();
        field.press([10.0, 0.0], [10.0, 40.0], 0.09);
        assert!((field.depth_at(10.0, 20.0) - RUT_PASS_DEPTH_M).abs() < 1e-6, "one pass");
        assert!(
            field.depth_at(10.0 + RUT_HALF_WIDTH_M * 0.5, 20.0) < RUT_PASS_DEPTH_M,
            "shallower off centre"
        );
        for _ in 0..4 {
            field.press([10.0, 0.0], [10.0, 40.0], 0.09);
        }
        assert!((field.depth_at(10.0, 20.0) - 0.09).abs() < 1e-6, "five passes cap at dirt");
        assert_eq!(field.depth_at(10.0 + RUT_HALF_WIDTH_M + 0.01, 20.0), 0.0, "and nothing beyond");
        assert_eq!(field.depth_at(10.0, 45.0), 0.0, "nor past the run's end");
        field.press([50.0, 0.0], [50.0, 40.0], 0.0);
        assert_eq!(field.depth_at(50.0, 20.0), 0.0, "rock remembers nothing");
        assert_eq!(field.segments().len(), 5, "and spent no budget");
        let cells = field.touched_cells(5.0, 39, 39);
        assert!(cells.contains(&(1, 4)) && cells.contains(&(2, 4)), "the run's cells");
        assert!(!cells.contains(&(5, 4)));
        for _ in 0..MAX_RUT_SEGMENTS {
            field.press([0.0, 0.0], [1.0, 0.0], 0.05);
        }
        assert_eq!(field.segments().len(), MAX_RUT_SEGMENTS, "capped per battle");
        assert_eq!(field.depth_at(10.0, 20.0), 0.0, "the oldest presses were recycled");
    }
}
