//! Teren 2.0: the ground's detail MATERIAL as baked, tiling, mipmapped textures.
//!
//! Until 2026-09-04 the terrain pass evaluated its detail as procedural lattice noise per
//! fragment ? three value-noise octaves with an analytic gradient, one carpet for all four
//! splat layers, scaled by a per-layer amplitude. A lattice sampled once per fragment has no
//! mip chain: from a tank's eye it beat against the pixel grid past ~30 m (O1's ripples),
//! the footprint fade that cured that then left the mid-field with no detail at all (T3), and
//! the carpet was the largest per-fragment cost of the largest pass of a fill-bound frame
//! (Q7). A texture with a mip chain is filtered by the hardware for free, at every angle and
//! through every lens, and a 10 m tile is small enough to live in the texture cache.
//!
//! So the detail is now an ASSET baked once per process, here, on the CPU (like the cloud
//! coverage tile ? `cloud_map.rs`), and uploaded by the renderer as one `texture_2d_array`
//! (four layers, splat channel order) plus one macro-variation tile:
//!
//! - **Detail tile, per layer** ([`GROUND_TILE_SIZE`]? texels over [`GROUND_TILE_PERIOD_M`]
//!   metres, ~2 cm per texel): `rg` = the tangent-space normal's xz (0.5 = flat), `b` = the
//!   albedo shade (0.5 = the layer's flat albedo), `a` = the height (0..1, for the height
//!   blend at splat borders ? grass clumps poke through a dirt edge, a dirt edge does not cut
//!   the clumps at a filtered line). Each layer is built like the material it names: grass as
//!   soft clumps, straw as short stubble, dirt as clods and pebbles, rock as fractured plates
//!   with cracks. Art-direction rule 5's two octaves are inside the tile (a ~2 m macro
//!   variation and a 0.3?0.8 m grain), and the mip chain is what keeps "nothing shimmers"
//!   true at every distance without a per-fragment fade.
//! - **Macro tile** ([`GROUND_MACRO_TILE_SIZE`]? over [`GROUND_MACRO_PERIOD_M`] metres, tapped
//!   twice by the shader ? once at its period, once rotated at [`GROUND_MACRO_FAR_RATIO`]?
//!   the period so no repeat shows inside a 1000 m map): `rgb` = a tone drift around 0.5
//!   (lush/dark vs dry/light, with the hue lean real land has), `a` = a lightness lane held
//!   for the erosion bake's macro AO (T2). This is T3's "one macro variation fetch": colour
//!   variation in the 15?120 m band the splat and the field quilt do not carry.
//!
//! Every octave is PERIODIC in the tile by construction and NONE of them sits on the tile's
//! axes: each octave is gradient noise on a lattice ROTATED by a Pythagorean angle
//! (`(3,4,5)`, `(5,12,13)`, `(8,15,17)` ? the tile's translations are then integer lattice
//! vectors, so the lattice wraps exactly), and the octaves' rotations and cell counts never
//! coincide. That is the same de-squaring `noise_common.wgsl` does with its octave frames,
//! done at bake time: no square plates, no grid lines in the normal, and a seam-free tile
//! without any edge blending. Nothing in a tile is DIRECTIONAL (T6: an isotropic tile cannot
//! print furrows).

use crate::texture::Rgba8MipLevel;

/// Detail tile edge in texels. 512 over 10 m is ~2 cm per texel: at least seven texels
/// across the finest grain the recipes carry, so the base level is not itself aliased.
pub const GROUND_TILE_SIZE: u32 = 512;
/// Detail tile period in metres. Ten metres holds five of the 2 m macro cells, so the repeat
/// is not readable in the 20?60 m band where the mip chain still shows that octave.
pub const GROUND_TILE_PERIOD_M: f32 = 10.0;
/// Macro tile edge in texels (0.6 m per texel at the near period, 2.4 m at the far tap).
pub const GROUND_MACRO_TILE_SIZE: u32 = 256;
/// Macro tile period in metres for the near tap.
pub const GROUND_MACRO_PERIOD_M: f32 = 160.0;
/// The far macro tap's period is this many near periods (613 m): irrational enough against
/// 160 m that the pair never lines up inside a map.
pub const GROUND_MACRO_FAR_RATIO: f32 = 3.83;
/// The four splat layers, in splat channel order (grass, straw, dirt, rock).
pub const GROUND_DETAIL_LAYERS: usize = 4;

/// The baked ground material: four detail layers and the macro tile, base level each (the
/// renderer builds the mip chains with `Rgba8MipChain::build(.., MipMode::Box)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundDetailTiles {
    pub layers: Vec<Rgba8MipLevel>,
    pub macro_tile: Rgba8MipLevel,
}

/// Bake the whole material. Deterministic (integer hash lattice), ~1 M texel evaluations.
pub fn bake_ground_detail_tiles() -> GroundDetailTiles {
    let layers =
        (0..GROUND_DETAIL_LAYERS).map(|layer| bake_detail_layer(layer, GROUND_TILE_SIZE)).collect();
    GroundDetailTiles { layers, macro_tile: bake_macro_tile(GROUND_MACRO_TILE_SIZE) }
}

/// A periodic, rotated lattice: lattice coordinates `p = m * (a*u + b*v, -b*u + a*v)` for a
/// Pythagorean pair `(a, b)` with `c = sqrt(a^2 + b^2)`. Moving one tile in `u` moves `p` by
/// the integer vector `m*(a, -b)`, one tile in `v` by `m*(b, a)`, so the lattice repeats with
/// the tile; the cell size is `tile / (m*c)` and the lattice sits at `atan2(b, a)` off the
/// axes.
#[derive(Clone, Copy)]
struct Frame {
    a: i32,
    b: i32,
    m: i32,
}

impl Frame {
    /// Cells across the tile's side ? the recipes' documentation, checked by the tests.
    #[cfg(test)]
    fn cells(self) -> f32 {
        self.m as f32 * ((self.a * self.a + self.b * self.b) as f32).sqrt()
    }
}

// The frames, by cell count over the 10 m tile (~ metres per cell): three rotations, no two
// octaves at the same angle AND scale, no cell count a multiple of another's.
/// 5 cells, 2.0 m, 53 degrees.
const F_BROAD: Frame = Frame { a: 3, b: 4, m: 1 };
/// 10 cells, 1.0 m, 37 degrees.
const F_MID: Frame = Frame { a: 4, b: 3, m: 2 };
/// 13 cells, 0.77 m, 23 degrees.
const F_FINE: Frame = Frame { a: 12, b: 5, m: 1 };
/// 26 cells, 0.38 m, 67 degrees.
const F_GRAIN: Frame = Frame { a: 5, b: 12, m: 2 };
/// 34 cells, 0.29 m, 62 degrees.
const F_MICRO: Frame = Frame { a: 8, b: 15, m: 2 };
/// 15 cells, 0.67 m, 53 degrees (the broad frame's rotation at three times the density).
const F_TUSSOCK: Frame = Frame { a: 3, b: 4, m: 3 };

/// Relief of each layer's height field in metres over the 0..1 height lane: what the
/// tangent normal is derived from. Grass is soft, rock is broken.
const LAYER_RELIEF_M: [f32; GROUND_DETAIL_LAYERS] = [0.055, 0.045, 0.075, 0.11];
/// How strongly each layer's height reads as albedo shade (a clump's shadow side, a crack).
const LAYER_SHADE_CONTRAST: [f32; GROUND_DETAIL_LAYERS] = [0.55, 0.80, 0.70, 0.90];

fn bake_detail_layer(layer: usize, size: u32) -> Rgba8MipLevel {
    let n = size as usize;
    let mut height = vec![0.0f32; n * n];
    let mut tone = vec![0.0f32; n * n];
    for y in 0..n {
        for x in 0..n {
            let u = (x as f32 + 0.5) / size as f32;
            let v = (y as f32 + 0.5) / size as f32;
            let (h, t) = layer_height_and_tone(layer, u, v);
            height[y * n + x] = h;
            tone[y * n + x] = t;
        }
    }
    normalize_unit(&mut height);
    // The shade lane centres on the layer's MEAN height and mean tone, not on 0.5: a pebble
    // field or a crack network skews its height distribution, and the tile must not retint
    // the layer's flat albedo on average.
    let mean_height = height.iter().sum::<f32>() / height.len() as f32;
    let mean_tone = tone.iter().sum::<f32>() / tone.len() as f32;
    let texel_m = GROUND_TILE_PERIOD_M / size as f32;
    let relief = LAYER_RELIEF_M[layer];
    let contrast = LAYER_SHADE_CONTRAST[layer];
    let mut rgba = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let at = |dx: isize, dy: isize| {
                let xi = (x as isize + dx).rem_euclid(n as isize) as usize;
                let yi = (y as isize + dy).rem_euclid(n as isize) as usize;
                height[yi * n + xi]
            };
            // Central differences with wrap: the tile's normal is seamless like its height.
            let dhdx = (at(1, 0) - at(-1, 0)) * relief / (2.0 * texel_m);
            let dhdy = (at(0, 1) - at(0, -1)) * relief / (2.0 * texel_m);
            let inv = 1.0 / (1.0 + dhdx * dhdx + dhdy * dhdy).sqrt();
            let (nx, nz) = (-dhdx * inv, -dhdy * inv);
            let h = height[y * n + x];
            let shade = 0.5 + (h - mean_height) * contrast + (tone[y * n + x] - mean_tone);
            rgba.extend([unorm(nx * 0.5 + 0.5), unorm(nz * 0.5 + 0.5), unorm(shade), unorm(h)]);
        }
    }
    Rgba8MipLevel::new(size, size, rgba)
}

/// The material recipes. `u, v` in tile units [0, 1); returns (raw height, tone offset).
fn layer_height_and_tone(layer: usize, u: f32, v: f32) -> (f32, f32) {
    let seed = layer as u32 * 101;
    match layer {
        // Grass: soft clumps ? a broad 2 m swell, 0.67 m tussocks, a 0.38 m blade grain, and
        // an independent tone mottle so a clump's colour is not only its height.
        0 => {
            let h = 0.35 * gnoise(u, v, F_BROAD, seed + 1)
                + 0.37 * gnoise(u, v, F_TUSSOCK, seed + 2)
                + 0.28 * gnoise(u, v, F_GRAIN, seed + 3);
            let t = (gnoise(u, v, F_MID, seed + 4) - 0.5) * 0.16;
            (h, t)
        }
        // Straw: short stubble ? ridged fine and micro octaves (stalks and their gaps read
        // as thin bright ridges), on a gentle swell. Isotropic: no row direction.
        1 => {
            let h = 0.25 * gnoise(u, v, F_BROAD, seed + 1)
                + 0.45 * ridged(gnoise(u, v, F_FINE, seed + 2))
                + 0.30 * ridged(gnoise(u, v, F_MICRO, seed + 3));
            let t = (gnoise(u, v, F_MID, seed + 4) - 0.5) * 0.10;
            (h, t)
        }
        // Dirt: clods and pebbles ? a rounded clod field (the mid octave pushed toward its
        // peaks) with sparse pebbles from a thresholded micro octave, pebbles reading light.
        2 => {
            let clod = gnoise(u, v, F_MID, seed + 2).powf(1.5);
            let pebble = tile_smoothstep(0.64, 0.80, gnoise(u, v, F_MICRO, seed + 3));
            let h = 0.30 * gnoise(u, v, F_BROAD, seed + 1) + 0.40 * clod + 0.30 * pebble;
            let t = pebble * 0.14 - 0.03;
            (h, t)
        }
        // Rock: fractured plates ? the inverted ridge of one octave is the crack network
        // (low where the ridge is), a second ridged octave splits the plates, a micro grain
        // roughens the faces. Cracks read dark beyond their height.
        _ => {
            let cracks = ridged(gnoise(u, v, F_BROAD, seed + 1)).powf(0.6);
            let split = ridged(gnoise(u, v, F_FINE, seed + 2));
            let h = 0.60 * cracks + 0.25 * split + 0.15 * gnoise(u, v, F_MICRO, seed + 3);
            let t = -(1.0 - cracks) * 0.10;
            (h, t)
        }
    }
}

/// The macro tone tile: lightness drift and a lush/dry lean, each a three-octave fbm over the
/// tile's 160 m (32 / 16 / 12 m cells at the near tap, 3.83x that at the far one).
fn bake_macro_tile(size: u32) -> Rgba8MipLevel {
    let n = size as usize;
    let mut light = vec![0.0f32; n * n];
    let mut dry = vec![0.0f32; n * n];
    for y in 0..n {
        for x in 0..n {
            let u = (x as f32 + 0.5) / size as f32;
            let v = (y as f32 + 0.5) / size as f32;
            light[y * n + x] = 0.5 * gnoise(u, v, F_BROAD, 900)
                + 0.3 * gnoise(u, v, F_MID, 901)
                + 0.2 * gnoise(u, v, F_FINE, 902);
            dry[y * n + x] = 0.5 * gnoise(u, v, F_BROAD, 950)
                + 0.3 * gnoise(u, v, F_MID, 951)
                + 0.2 * gnoise(u, v, F_FINE, 952);
        }
    }
    normalize_unit(&mut light);
    normalize_unit(&mut dry);
    let mean_light = light.iter().sum::<f32>() / light.len() as f32;
    let mean_dry = dry.iter().sum::<f32>() / dry.len() as f32;
    let mut rgba = Vec::with_capacity(n * n * 4);
    for i in 0..n * n {
        let l = light[i] - mean_light;
        let d = dry[i] - mean_dry;
        // Dry land leans yellow (red up, blue down); lush land leans the other way.
        rgba.extend([
            unorm(0.5 + l * 0.5 + d * 0.30),
            unorm(0.5 + l * 0.5 + d * 0.10),
            unorm(0.5 + l * 0.5 - d * 0.30),
            unorm(0.5 + l),
        ]);
    }
    Rgba8MipLevel::new(size, size, rgba)
}

/// Integer-domain lattice hash (the PCG-style mix of `noise_common.wgsl::lattice_hash`),
/// seeded per octave so no two octaves share corner values.
fn tile_hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B9)
        ^ (y as u32).wrapping_mul(0x85EB_CA6B)
        ^ seed.wrapping_mul(0x27D4_EB2F);
    h = (h ^ (h >> 15)).wrapping_mul(0xC2B2_AE35);
    h ^= h >> 13;
    (h & 0x00FF_FFFF) as f32 / 16_777_215.0
}

/// Periodic gradient noise on a rotated lattice, in [0, 1]. The lattice point `i` is reduced
/// to a canonical representative modulo the tile's two translation vectors before hashing
/// (they are orthogonal, so the floor-coefficient reduction is unique), which is what makes
/// the octave wrap exactly with the tile. Quintic fade: C2, no lattice-line kinks in the
/// derived normal.
fn gnoise(u: f32, v: f32, frame: Frame, seed: u32) -> f32 {
    let Frame { a, b, m } = frame;
    let (af, bf, mf) = (a as f32, b as f32, m as f32);
    // A per-octave offset so two octaves on the same frame never share a lattice origin.
    let (ou, ov) = (tile_hash(seed as i32, 7, 3), tile_hash(seed as i32, 11, 5));
    let (uu, vv) = (u + ou, v + ov);
    let px = mf * (af * uu + bf * vv);
    let py = mf * (-bf * uu + af * vv);
    let (ix, iy) = (px.floor(), py.floor());
    let (fx, fy) = (px - ix, py - iy);
    let (ix, iy) = (ix as i32, iy as i32);
    // Tile translations in lattice space and their common squared length.
    let (t1x, t1y) = (m * a, -m * b);
    let (t2x, t2y) = (m * b, m * a);
    let len_sq = m * m * (a * a + b * b);
    let reduce = |x: i32, y: i32| -> (i32, i32) {
        let k1 = (x * t1x + y * t1y).div_euclid(len_sq);
        let k2 = (x * t2x + y * t2y).div_euclid(len_sq);
        (x - k1 * t1x - k2 * t2x, y - k1 * t1y - k2 * t2y)
    };
    let gradient_dot = |cx: i32, cy: i32, dx: f32, dy: f32| -> f32 {
        let (rx, ry) = reduce(cx, cy);
        let angle = tile_hash(rx, ry, seed) * std::f32::consts::TAU;
        angle.cos() * dx + angle.sin() * dy
    };
    let n00 = gradient_dot(ix, iy, fx, fy);
    let n10 = gradient_dot(ix + 1, iy, fx - 1.0, fy);
    let n01 = gradient_dot(ix, iy + 1, fx, fy - 1.0);
    let n11 = gradient_dot(ix + 1, iy + 1, fx - 1.0, fy - 1.0);
    let (wx, wy) = (tile_fade(fx), tile_fade(fy));
    let value = tile_lerp(tile_lerp(n00, n10, wx), tile_lerp(n01, n11, wx), wy);
    // Gradient noise spans about +-0.7 with a quintic fade; map into [0, 1].
    (value * 0.7 + 0.5).clamp(0.0, 1.0)
}

fn tile_fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn tile_lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn ridged(n: f32) -> f32 {
    1.0 - (2.0 * n - 1.0).abs()
}

fn tile_smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Stretch a field to exactly [0, 1] so the height blend and the shade contrast mean the
/// same thing in every layer.
fn normalize_unit(field: &mut [f32]) {
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for &v in field.iter() {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    let span = (hi - lo).max(1.0e-6);
    for v in field.iter_mut() {
        *v = (*v - lo) / span;
    }
}

fn unorm(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texel(level: &Rgba8MipLevel, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * level.width() + x) * 4) as usize;
        level.rgba()[i..i + 4].try_into().unwrap()
    }

    fn channel_mean(level: &Rgba8MipLevel, channel: usize) -> f32 {
        let bytes = level.rgba();
        let count = (level.width() * level.height()) as f32;
        bytes.chunks_exact(4).map(|t| t[channel] as f32 / 255.0).sum::<f32>() / count
    }

    /// Every octave frame wraps with the tile: the noise one tile over is the noise here.
    #[test]
    fn every_frame_is_periodic_in_the_tile() {
        for frame in [F_BROAD, F_MID, F_FINE, F_GRAIN, F_MICRO, F_TUSSOCK] {
            for (u, v) in [(0.13, 0.71), (0.5, 0.5), (0.999, 0.001), (0.37, 0.92)] {
                let here = gnoise(u, v, frame, 17);
                for (du, dv) in [(1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (-1.0, 2.0)] {
                    let there = gnoise(u + du, v + dv, frame, 17);
                    assert!(
                        (here - there).abs() < 1.0e-4,
                        "frame {}x{}/{} not periodic at ({u}, {v}) + ({du}, {dv}): {here} vs {there}",
                        frame.a,
                        frame.b,
                        frame.m
                    );
                }
            }
        }
    }

    /// The tile is periodic: the last column continues into the first without a step. A
    /// seam here would print a 10 m grid on every meadow ? the very thing a tile must not do.
    #[test]
    fn every_tile_is_seamless_at_its_edges() {
        let tiles = bake_ground_detail_tiles();
        let all = tiles.layers.iter().chain(std::iter::once(&tiles.macro_tile));
        for level in all {
            let n = level.width();
            // Neighbour steps across the wrap versus across every interior column/row: the
            // wrap is one more texel step of the same continuous field, so its worst step
            // may not exceed the field's own worst interior step (per channel).
            let step = |x0: u32, y0: u32, x1: u32, y1: u32, c: usize| {
                (texel(level, x0, y0)[c] as i32 - texel(level, x1, y1)[c] as i32).abs()
            };
            for c in 0..4 {
                let mut seam = 0;
                let mut interior = 0;
                for y in 0..n {
                    seam = seam.max(step(0, y, n - 1, y, c));
                    for x in 1..n {
                        interior = interior.max(step(x, y, x - 1, y, c));
                    }
                }
                for x in 0..n {
                    seam = seam.max(step(x, 0, x, n - 1, c));
                    for y in 1..n {
                        interior = interior.max(step(x, y, x, y - 1, c));
                    }
                }
                assert!(
                    seam <= interior,
                    "channel {c}: wrap step {seam} exceeds the worst interior step {interior} \
                     on a {n}x{n} tile ? the tile is not periodic"
                );
            }
        }
    }

    /// Flat on average: a tile's normal lane centres on "up" and its shade lane on "no
    /// change", so tiling it over the ground changes neither the mean albedo nor the mean
    /// lighting ? only the detail. (A bias here would retint every map.)
    #[test]
    fn the_tiles_are_neutral_on_average() {
        let tiles = bake_ground_detail_tiles();
        for (i, level) in tiles.layers.iter().enumerate() {
            let nx = channel_mean(level, 0);
            let nz = channel_mean(level, 1);
            let shade = channel_mean(level, 2);
            let height = channel_mean(level, 3);
            assert!(
                (nx - 0.5).abs() < 0.01 && (nz - 0.5).abs() < 0.01,
                "layer {i} normal {nx} {nz}"
            );
            assert!((shade - 0.5).abs() < 0.02, "layer {i} shade mean {shade}");
            assert!(height > 0.3 && height < 0.7, "layer {i} height mean {height} uses its range");
        }
        for c in 0..4 {
            let mean = channel_mean(&tiles.macro_tile, c);
            assert!((mean - 0.5).abs() < 0.02, "macro channel {c} mean {mean}");
        }
    }

    /// Four MATERIALS, not one carpet with four amplitudes: the layers' height fields are
    /// decorrelated, and the fractured rock carries more normal energy than the soft grass.
    #[test]
    fn the_four_layers_are_distinct_materials() {
        let tiles = bake_ground_detail_tiles();
        let heights: Vec<Vec<f32>> = tiles
            .layers
            .iter()
            .map(|l| l.rgba().chunks_exact(4).map(|t| t[3] as f32 / 255.0).collect())
            .collect();
        let correlation = |a: &[f32], b: &[f32]| {
            let n = a.len() as f32;
            let (ma, mb) = (a.iter().sum::<f32>() / n, b.iter().sum::<f32>() / n);
            let cov: f32 = a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum::<f32>() / n;
            let va: f32 = a.iter().map(|x| (x - ma) * (x - ma)).sum::<f32>() / n;
            let vb: f32 = b.iter().map(|y| (y - mb) * (y - mb)).sum::<f32>() / n;
            cov / (va * vb).sqrt().max(1.0e-6)
        };
        for i in 0..GROUND_DETAIL_LAYERS {
            for j in (i + 1)..GROUND_DETAIL_LAYERS {
                let r = correlation(&heights[i], &heights[j]);
                assert!(r.abs() < 0.25, "layers {i} and {j} share their height field (r = {r})");
            }
        }
        let slope_energy = |level: &Rgba8MipLevel| {
            level
                .rgba()
                .chunks_exact(4)
                .map(|t| {
                    let x = t[0] as f32 / 127.5 - 1.0;
                    let z = t[1] as f32 / 127.5 - 1.0;
                    x * x + z * z
                })
                .sum::<f32>()
        };
        assert!(
            slope_energy(&tiles.layers[3]) > slope_energy(&tiles.layers[0]) * 1.5,
            "rock is more broken than grass"
        );
    }

    /// No two octave frames share both rotation and scale, no frame is axis-aligned, and the
    /// finest frame still has at least seven texels per cell on the base level.
    #[test]
    fn the_frames_are_rotated_distinct_and_resolved() {
        let frames = [F_BROAD, F_MID, F_FINE, F_GRAIN, F_MICRO, F_TUSSOCK];
        for (i, f) in frames.iter().enumerate() {
            assert!(f.a != 0 && f.b != 0, "frame {i} is axis-aligned");
            let texels_per_cell = GROUND_TILE_SIZE as f32 / f.cells();
            assert!(texels_per_cell >= 7.0, "frame {i}: {texels_per_cell} texels per cell");
            for g in &frames[i + 1..] {
                let same_angle = f.a * g.b == f.b * g.a;
                let same_scale = (f.cells() - g.cells()).abs() < 0.5;
                assert!(!(same_angle && same_scale), "two frames coincide");
            }
        }
        assert!(GROUND_TILE_SIZE.is_power_of_two() && GROUND_MACRO_TILE_SIZE.is_power_of_two());
        // The far macro tap must not line up with the near one inside a map.
        let far = GROUND_MACRO_PERIOD_M * GROUND_MACRO_FAR_RATIO;
        assert!((far / GROUND_MACRO_PERIOD_M).fract() > 0.3, "far/near ratio near-integer");
        assert!(far < 1000.0, "the far period still varies inside a 1000 m map");
    }
}
