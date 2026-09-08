//! The ONE march (the one program's V0, S21's shot half). The eye stepped the ground every
//! 2 m without interpolation and the shell every 1 m with it: on a ±30° crest the eye saw
//! ~0.9 m through what killed the shell. The sampled ground is PIECEWISE PLANAR by
//! construction (`HeightMap::sample_height` splits every cell on the diagonal the mesh draws),
//! so along a segment the ground clearance is piecewise LINEAR between the cell lines and the
//! diagonals — evaluate it there and the first crossing is exact, no step to choose. A crater
//! bends the surface inside its influence; those pieces are walked at `CRATER_MARCH_STEP_M`.
//! Both consumers — the sight line (a slack, no radius) and the shell (a radius, no slack) —
//! read this one kernel.

use crate::HeightMap;
use crate::heightmap::cell_splits_on_main_diagonal;

/// Inside a crater's influence the surface is a bowl, not a plane: walk it this finely.
pub const CRATER_MARCH_STEP_M: f32 = 0.25;

fn along_segment(from: [f32; 3], to: [f32; 3], t: f32) -> [f32; 3] {
    [
        from[0] + (to[0] - from[0]) * t,
        from[1] + (to[1] - from[1]) * t,
        from[2] + (to[2] - from[2]) * t,
    ]
}

/// The parameters along `from → to` at which the ground's planar pieces change: 0 and 1, every
/// cell line in x and z the segment crosses, and the diagonal of every cell it passes through.
/// Sorted, deduplicated.
fn breakpoints(heightmap: &HeightMap, from: [f32; 3], to: [f32; 3]) -> Vec<f32> {
    let cell = heightmap.cell_size_m();
    let (dx, dz) = (to[0] - from[0], to[2] - from[2]);
    let mut ts = vec![0.0f32, 1.0];
    let mut lines = |a: f32, b: f32, d: f32| {
        if d.abs() < 1.0e-6 {
            return;
        }
        let lo = (a.min(b) / cell).ceil() as i64;
        let hi = (a.max(b) / cell).floor() as i64;
        for k in lo..=hi {
            let t = (k as f32 * cell - a) / d;
            if t > 0.0 && t < 1.0 {
                ts.push(t);
            }
        }
    };
    lines(from[0], to[0], dx);
    lines(from[2], to[2], dz);
    ts.sort_by(|a, b| a.total_cmp(b));
    ts.dedup_by(|a, b| (*a - *b).abs() < 1.0e-7);
    // The diagonal of the cell each piece lies in.
    let mut with_diagonals = Vec::with_capacity(ts.len() * 2);
    let max_x = heightmap.width().saturating_sub(2) as f32;
    let max_z = heightmap.height().saturating_sub(2) as f32;
    for pair in ts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        with_diagonals.push(a);
        let mid = along_segment(from, to, (a + b) * 0.5);
        let gx = (mid[0] / cell).floor();
        let gz = (mid[2] / cell).floor();
        if gx < 0.0 || gz < 0.0 || gx > max_x || gz > max_z {
            continue;
        }
        let (x0, z0) = (gx as usize, gz as usize);
        let h00 = heightmap.sample_at_index(x0, z0);
        let h10 = heightmap.sample_at_index(x0 + 1, z0);
        let h01 = heightmap.sample_at_index(x0, z0 + 1);
        let h11 = heightmap.sample_at_index(x0 + 1, z0 + 1);
        let (cx, cz) = (gx * cell, gz * cell);
        let t = if cell_splits_on_main_diagonal(h00, h10, h01, h11) {
            // x - cx == z - cz
            let d = dx - dz;
            (d.abs() > 1.0e-6).then(|| (cx - cz - from[0] + from[2]) / d)
        } else {
            // (x - cx) + (z - cz) == cell
            let d = dx + dz;
            (d.abs() > 1.0e-6).then(|| (cx + cz + cell - from[0] - from[2]) / d)
        };
        if let Some(t) = t
            && t > a + 1.0e-7
            && t < b - 1.0e-7
        {
            with_diagonals.push(t);
        }
    }
    with_diagonals.push(1.0);
    with_diagonals
}

/// Whether the piece `[a, b]` of the segment runs through any crater's influence.
fn piece_meets_a_crater(
    heightmap: &HeightMap,
    from: [f32; 3],
    to: [f32; 3],
    a: f32,
    b: f32,
) -> bool {
    if !heightmap.has_craters() {
        return false;
    }
    let pa = along_segment(from, to, a);
    let pb = along_segment(from, to, b);
    heightmap.crater_records().iter().any(|record| {
        let reach = record.influence_radius_m();
        let (cx, cz) = (record.x_m(), record.z_m());
        // The segment piece to the crater centre: closest-point distance in the plane.
        let (ex, ez) = (pb[0] - pa[0], pb[2] - pa[2]);
        let len_sq = ex * ex + ez * ez;
        let t = if len_sq <= 1.0e-9 {
            0.0
        } else {
            (((cx - pa[0]) * ex + (cz - pa[2]) * ez) / len_sq).clamp(0.0, 1.0)
        };
        let (qx, qz) = (pa[0] + ex * t, pa[2] + ez * t);
        (qx - cx) * (qx - cx) + (qz - cz) * (qz - cz) < reach * reach
    })
}

/// Walk the segment's pieces in order, calling `visit(t)` at every parameter where the
/// clearance is worth reading: the piece ends, and inside a crater's influence every
/// `CRATER_MARCH_STEP_M`. Returns early when `visit` says so.
fn march(heightmap: &HeightMap, from: [f32; 3], to: [f32; 3], mut visit: impl FnMut(f32) -> bool) {
    let ts = breakpoints(heightmap, from, to);
    let length = {
        let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    };
    if visit(ts[0]) {
        return;
    }
    for pair in ts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if piece_meets_a_crater(heightmap, from, to, a, b) {
            let steps = (((b - a) * length) / CRATER_MARCH_STEP_M).ceil().max(1.0) as u32;
            for step in 1..steps {
                if visit(a + (b - a) * step as f32 / steps as f32) {
                    return;
                }
            }
        }
        if visit(b) {
            return;
        }
    }
}

/// The eye's half: whether the ground rises more than `slack_m` above the segment anywhere
/// strictly between its ends (a start or an end already under the ground counts too). Ground
/// off the map is no ground.
pub fn ground_blocks_segment(
    heightmap: &HeightMap,
    from: [f32; 3],
    to: [f32; 3],
    slack_m: f32,
) -> bool {
    if heightmap.bounds_clear_segment(from, to, slack_m) {
        return false;
    }
    ground_blocks_segment_exact(heightmap, from, to, slack_m)
}

fn ground_blocks_segment_exact(
    heightmap: &HeightMap,
    from: [f32; 3],
    to: [f32; 3],
    slack_m: f32,
) -> bool {
    let clearance = |t: f32| {
        let p = along_segment(from, to, t);
        heightmap.sample_height(p[0], p[2]).map_or(f32::INFINITY, |ground| p[1] + slack_m - ground)
    };
    let mut blocked = false;
    march(heightmap, from, to, |t| {
        let f = clearance(t);
        blocked = if t <= 0.0 || t >= 1.0 { f < 0.0 } else { f <= 0.0 };
        blocked
    });
    blocked
}

/// The shell's half: where a projectile of `radius_m` first meets the ground coming from above
/// it — the point ON the surface — or `None` if it clears the segment.
pub fn first_ground_impact(
    heightmap: &HeightMap,
    from: [f32; 3],
    to: [f32; 3],
    radius_m: f32,
) -> Option<[f32; 3]> {
    let clearance = |t: f32| {
        let p = along_segment(from, to, t);
        heightmap.sample_height(p[0], p[2]).map_or(f32::INFINITY, |ground| p[1] - ground - radius_m)
    };
    let mut previous: Option<(f32, f32)> = None;
    let mut hit = None;
    march(heightmap, from, to, |t| {
        let f = clearance(t);
        if let Some((pt, pf)) = previous
            && pf > 0.0
            && f <= 0.0
            && pf.is_finite()
        {
            let s = pt + (t - pt) * (pf / (pf - f)).clamp(0.0, 1.0);
            let p = along_segment(from, to, s);
            hit = heightmap.sample_height(p[0], p[2]).map(|ground| [p[0], ground, p[2]]);
            return hit.is_some();
        }
        previous = Some((t, f));
        false
    });
    hit
}

/// The ground's normal at a point: the plane of the triangle under it (a central difference
/// well inside one cell reads the plane exactly; across an edge it averages the two).
pub fn ground_normal_at(heightmap: &HeightMap, x: f32, z: f32) -> [f32; 3] {
    let step = heightmap.cell_size_m() * 0.1;
    let sample = |px: f32, pz: f32| heightmap.sample_height(px, pz);
    let (Some(l), Some(r), Some(d), Some(u)) =
        (sample(x - step, z), sample(x + step, z), sample(x, z - step), sample(x, z + step))
    else {
        return [0.0, 1.0, 0.0];
    };
    let n = [l - r, 2.0 * step, d - u];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1.0e-6);
    [n[0] / len, n[1] / len, n[2] / len]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rolling() -> HeightMap {
        crate::heightmap_from_fn(41, 5.0, |x, z| {
            10.0 + (x * 0.11).sin() * 3.5 + (z * 0.07).cos() * 2.5 + ((x + z) * 0.05).sin() * 1.5
        })
    }

    /// Brute force: a centimetre march, the reference the kernel is measured against.
    fn brute_blocks(heightmap: &HeightMap, from: [f32; 3], to: [f32; 3], slack: f32) -> bool {
        let n = 20_000;
        (1..n).any(|i| {
            let p = along_segment(from, to, i as f32 / n as f32);
            heightmap.sample_height(p[0], p[2]).is_some_and(|g| g > p[1] + slack)
        })
    }

    fn brute_impact(heightmap: &HeightMap, from: [f32; 3], to: [f32; 3]) -> Option<[f32; 3]> {
        let n = 20_000;
        let mut prev = f32::INFINITY;
        for i in 0..=n {
            let p = along_segment(from, to, i as f32 / n as f32);
            let f = heightmap.sample_height(p[0], p[2]).map_or(f32::INFINITY, |g| p[1] - g);
            if prev > 0.0 && f <= 0.0 {
                return Some(p);
            }
            prev = f;
        }
        None
    }

    fn splitmix(seed: &mut u64) -> f32 {
        *seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 40) as f32 / (1u64 << 24) as f32
    }

    #[test]
    fn the_height_pyramid_preserves_the_exact_march_and_map_identity() {
        let mut seed = 81;
        for (width, height) in [(2, 2), (41, 41), (64, 19), (17, 66)] {
            let samples = (0..width * height).map(|_| splitmix(&mut seed) * 30.0 - 15.0).collect();
            let mut map = HeightMap::new(width, height, 5.0, samples).unwrap();
            let original = map.clone();
            let bytes = bincode::serialize(&map).unwrap();
            let [ex, ez] = map.extent_m();
            assert!(map.bounds_clear_segment([0.0, 100.0, 0.0], [ex, 100.0, ez], 0.0));
            assert_eq!(bincode::serialize(&map).unwrap(), bytes);
            assert_eq!(map, original);
            let decoded: HeightMap = bincode::deserialize(&bytes).unwrap();
            assert!(decoded.bounds_clear_segment([0.0, 100.0, 0.0], [ex, 100.0, ez], 0.0));
            for crater in [false, true] {
                if crater {
                    map.set_craters(&[crate::CraterRecord::from_world(
                        ex * 0.5,
                        ez * 0.5,
                        2.4,
                        0.9,
                        0,
                    )]);
                }
                for _ in 0..1000 {
                    let from = [
                        splitmix(&mut seed) * (ex + 4.0) - 2.0,
                        splitmix(&mut seed) * 50.0 - 20.0,
                        splitmix(&mut seed) * (ez + 4.0) - 2.0,
                    ];
                    let to = [
                        splitmix(&mut seed) * (ex + 4.0) - 2.0,
                        splitmix(&mut seed) * 50.0 - 20.0,
                        splitmix(&mut seed) * (ez + 4.0) - 2.0,
                    ];
                    for slack in [0.0, 0.15, -0.1] {
                        assert_eq!(
                            ground_blocks_segment(&map, from, to, slack),
                            ground_blocks_segment_exact(&map, from, to, slack),
                            "{from:?} -> {to:?}, slack {slack}"
                        );
                    }
                }
            }
        }
        let flat = HeightMap::flat(41, 41, 5.0, 10.0).unwrap();
        for y in [9.99, 10.0, 10.000001, 10.01] {
            for (from, to) in
                [([0.0, y, 0.0], [200.0, y, 200.0]), ([200.0, y, 200.0], [200.0, y, 200.0])]
            {
                assert_eq!(
                    ground_blocks_segment(&flat, from, to, 0.0),
                    ground_blocks_segment_exact(&flat, from, to, 0.0)
                );
            }
        }
    }

    /// V0: the kernel agrees with a centimetre march on three hundred random segments over
    /// rolling ground — blocked or clear, exactly; the impact point within two centimetres —
    /// and its breakpoints are the cell lines and diagonals, not a step anyone chose.
    #[test]
    fn the_march_is_exact_on_the_planar_surface() {
        let map = rolling();
        let mut seed = 7u64;
        let (mut blocked, mut clear, mut hits) = (0, 0, 0);
        for _ in 0..300 {
            let ax = 10.0 + splitmix(&mut seed) * 180.0;
            let az = 10.0 + splitmix(&mut seed) * 180.0;
            let bx = 10.0 + splitmix(&mut seed) * 180.0;
            let bz = 10.0 + splitmix(&mut seed) * 180.0;
            let lift_a = 0.5 + splitmix(&mut seed) * 3.0;
            let lift_b = 0.5 + splitmix(&mut seed) * 3.0;
            let from = [ax, map.sample_height(ax, az).unwrap() + lift_a, az];
            let to = [bx, map.sample_height(bx, bz).unwrap() + lift_b, bz];
            let kernel = ground_blocks_segment(&map, from, to, 0.0);
            let brute = brute_blocks(&map, from, to, 0.0);
            assert_eq!(kernel, brute, "the eye's verdict from {from:?} to {to:?}");
            if kernel {
                blocked += 1;
            } else {
                clear += 1;
            }
            match (first_ground_impact(&map, from, to, 0.0), brute_impact(&map, from, to)) {
                (Some(k), Some(b)) => {
                    hits += 1;
                    let d = ((k[0] - b[0]).powi(2) + (k[2] - b[2]).powi(2)).sqrt();
                    assert!(d < 0.02, "the shell's impact within two centimetres: {d}");
                }
                (None, None) => {}
                (k, b) => panic!("the shell's verdict differs: {k:?} vs {b:?}"),
            }
        }
        assert!(
            blocked > 30 && clear > 30 && hits > 30,
            "{blocked} blocked, {clear} clear, {hits} hits"
        );
        let ts = breakpoints(&map, [12.0, 20.0, 12.0], [48.0, 20.0, 33.0]);
        assert!(ts.len() > 15, "cell lines and diagonals: {} breakpoints", ts.len());
        assert!(ts.windows(2).all(|w| w[0] < w[1]), "sorted");
    }

    /// V0: a crater bends the surface; the march walks its bowl finely. A low line over the
    /// bowl that the flat ground blocked now passes; a plunging shell lands in the bowl below
    /// the old ground, on the surface.
    #[test]
    fn the_march_follows_a_crater_into_its_bowl() {
        let mut flat = HeightMap::flat(41, 41, 5.0, 10.0).expect("flat");
        let from = [90.0, 10.05, 100.0];
        let to = [110.0, 10.05, 100.0];
        assert!(!ground_blocks_segment(&flat, from, to, 0.0), "flat ground, a hair over it");
        assert!(ground_blocks_segment(&flat, [90.0, 9.95, 100.0], to, 0.0), "a hair under it");
        let crater = crate::CraterRecord::from_world(100.0, 100.0, 2.4, 0.9, 0);
        flat.set_craters(&[crater]);
        // The rim rises past the lip: the same hair-over line is blocked by the spoil.
        assert!(ground_blocks_segment(&flat, from, to, 0.0), "the rim stands in the way");
        let impact = first_ground_impact(&flat, [100.0, 14.0, 100.0], [100.0, 6.0, 100.0], 0.0)
            .expect("the plunge lands");
        assert!(impact[1] < 10.0 - 0.8, "in the bowl: y {}", impact[1]);
        let normal = ground_normal_at(&flat, 101.0, 100.0);
        assert!(normal[1] > 0.7 && normal[0] < -0.05, "the bowl's wall leans: {normal:?}");
    }
}
