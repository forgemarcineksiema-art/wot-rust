//! Conservative maxima of immutable height samples. Derived data never enters map identity.
use std::sync::{Arc, OnceLock};

use crate::HeightMap;

#[derive(Debug, Clone, Default)]
pub(crate) struct HeightBounds(Arc<OnceLock<Vec<Level>>>);

impl PartialEq for HeightBounds {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

#[derive(Debug)]
struct Level {
    width: usize,
    height: usize,
    maxima: Vec<f32>,
}

impl HeightBounds {
    pub(crate) fn clears(&self, map: &HeightMap, from: [f32; 3], to: [f32; 3], slack: f32) -> bool {
        // Raised crater rims are not part of the immutable pyramid. Keep the exact kernel
        // wherever a rim could touch this rectangle, and for exceptional coordinates.
        let extent = map.extent_m();
        if !slack.is_finite()
            || [from, to].iter().any(|p| {
                !p.iter().all(|v| v.is_finite())
                    || !(0.0..=extent[0]).contains(&p[0])
                    || !(0.0..=extent[1]).contains(&p[2])
            })
        {
            return false;
        }
        if map.crater_records().iter().any(|crater| {
            let r = crater.influence_radius_m() + 0.001;
            crater.x_m() + r >= from[0].min(to[0])
                && crater.x_m() - r <= from[0].max(to[0])
                && crater.z_m() + r >= from[2].min(to[2])
                && crater.z_m() - r <= from[2].max(to[2])
        }) {
            return false;
        }
        let levels = self.0.get_or_init(|| {
            let mut levels = Vec::new();
            let scale = map.samples().iter().fold(1.0f32, |m, v| m.max(v.abs()));
            let rounding = 32.0 * f32::EPSILON * scale;
            let (mut width, mut height) = (map.width(), map.height());
            let mut previous = map.samples();
            while width > 1 || height > 1 {
                let (nw, nh) = (width.div_ceil(2), height.div_ceil(2));
                let mut maxima = vec![f32::NEG_INFINITY; nw * nh];
                for z in 0..height {
                    for x in 0..width {
                        let v = previous[z * width + x];
                        let v = if levels.is_empty() { v + rounding } else { v };
                        let dest = &mut maxima[(z / 2) * nw + x / 2];
                        *dest = dest.max(if v.is_finite() { v } else { f32::INFINITY });
                    }
                }
                levels.push(Level { width: nw, height: nh, maxima });
                previous = &levels.last().expect("just pushed").maxima;
                (width, height) = (nw, nh);
            }
            levels
        });
        Self::piece_clear(levels, map, from, to, slack, 0)
    }

    fn piece_clear(
        levels: &[Level],
        map: &HeightMap,
        a: [f32; 3],
        b: [f32; 3],
        slack: f32,
        depth: u32,
    ) -> bool {
        let cell = map.cell_size_m();
        // Include every corner of every touched cell, plus a neighbour for grid rounding.
        let bounds = |axis: usize, size: usize| {
            let lo = ((a[axis].min(b[axis]) / cell).floor() as usize).saturating_sub(1);
            let hi = (((a[axis].max(b[axis]) / cell).ceil() as usize) + 1).min(size - 1);
            (lo, hi)
        };
        let (x0, x1) = bounds(0, map.width());
        let (z0, z1) = bounds(2, map.height());
        let span = (x1 - x0 + 1).max(z1 - z0 + 1);
        let shift = (usize::BITS - (span - 1).leading_zeros()).max(1);
        let level = &levels[(shift as usize - 1).min(levels.len() - 1)];
        let mut max = f32::NEG_INFINITY;
        for z in (z0 >> shift)..=(z1 >> shift).min(level.height - 1) {
            for x in (x0 >> shift)..=(x1 >> shift).min(level.width - 1) {
                max = max.max(level.maxima[z * level.width + x]);
            }
        }
        // The sampler's two multiply-adds can round above a vertex maximum. A generous
        // scale-relative margin sends all grazing cases back to the unchanged march.
        let margin = 32.0 * f32::EPSILON * max.abs().max(a[1].abs()).max(b[1].abs()).max(1.0);
        if a[1].min(b[1]) + slack > max + margin {
            return true;
        }
        if depth >= 5 || span <= 8 {
            return false;
        }
        let mid = std::array::from_fn(|i| a[i] + (b[i] - a[i]) * 0.5);
        Self::piece_clear(levels, map, a, mid, slack, depth + 1)
            && Self::piece_clear(levels, map, mid, b, slack, depth + 1)
    }
}
