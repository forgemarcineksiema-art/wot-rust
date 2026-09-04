// Shared world-anchored detail noise for the lit ground passes (scene, terrain) — ONE copy of
// the hash, the lattice and the octave frames, so the ground and the statics standing on it
// carry the same grain (one picture; the dedup is locked by wgsl_layout tests).
//
// The de-squaring contract (the ground twin of `sky_cloud_field_is_lattice_decorrelated`):
// the old ground grain showed hard ~0.3-0.6 m square plates from three compounding roots,
// each fixed here and locked by `ground_grain_is_lattice_decorrelated`:
// - `fract(sin(dot))` collapses once its argument leaves the GPU sin's accurate range
//   (dot(world * 1.7, (127.1, 311.7)) passes 1e5 within metres of the origin), hashing whole
//   lattice cells to correlated corners — flat hard-edged plates. The lattice hash below is
//   integer-domain, exactly the fix the sky pass took.
// - Every octave sampled on the bare world axes shares ONE square lattice; the octave frames
//   rotate each scale off the axes and off each other.
// - The light-catch "gradient" was a finite difference stepped at over half a lattice cell,
//   which facets the field into tiles; `value_noise_grad` returns the ANALYTIC derivative.

// Corner-tone hash for SMALL lattice indices (field-quilt plots, plank/tile tones). Kept for
// the treatments whose inputs stay near the origin; world-metre lattices must go through
// `lattice_hash` instead (sin loses the mantissa out there — the square-plate collapse).
fn detail_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// Integer-domain lattice hash (PCG-style mix), stable across the whole 1000 m map and its
// 1500 m apron: cells arrive as floor()ed floats, biased positive so the u32 cast is exact.
fn lattice_hash(cell: vec2<f32>) -> f32 {
    let q = vec2<u32>(cell + vec2<f32>(32768.0, 32768.0));
    var h = q.x * 0x9E3779B9u ^ q.y * 0x85EBCA6Bu;
    h = (h ^ (h >> 15u)) * 0xC2B2AE35u;
    h = h ^ (h >> 13u);
    return f32(h & 0xFFFFFFu) / 16777215.0;
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = lattice_hash(i);
    let b = lattice_hash(i + vec2<f32>(1.0, 0.0));
    let c = lattice_hash(i + vec2<f32>(0.0, 1.0));
    let d = lattice_hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Value noise WITH its analytic derivative: x = value, yz = d(value)/d(p). One lattice
// evaluation replaces the old three-sample finite difference — cheaper AND smooth at every
// step size (the FD's ~0.6-cell step is what faceted the grain into square plates).
fn value_noise_grad(p: vec2<f32>) -> vec3<f32> {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let du = 6.0 * f * (1.0 - f);
    let a = lattice_hash(i);
    let b = lattice_hash(i + vec2<f32>(1.0, 0.0));
    let c = lattice_hash(i + vec2<f32>(0.0, 1.0));
    let d = lattice_hash(i + vec2<f32>(1.0, 1.0));
    // n = a + (b-a)ux + (c-a)uy + (a-b-c+d)uxuy; differentiate through the smoothstep fade.
    let dx = mix(b - a, d - c, u.y) * du.x;
    let dy = mix(c - a, d - b, u.x) * du.y;
    return vec3<f32>(mix(mix(a, b, u.x), mix(c, d, u.x), u.y), dx, dy);
}

// Octave frames: fixed unit rotations (exact cos/sin pairs) that take each scale off the
// world axes. Chain-rule partners live in ground_grain/micro_grain — change them TOGETHER.
fn octave_frame_broad(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(p.x * 0.96 - p.y * 0.28, p.x * 0.28 + p.y * 0.96);
}
fn octave_frame_fine(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(p.x * 0.8 + p.y * 0.6, -p.x * 0.6 + p.y * 0.8);
}

// The two-octave ground grain (~2.5 m patches with ~0.6 m grain) shared by the terrain pass
// and the statics' generic ground treatment: x = detail mix in [0, 1], yz = the WORLD-space
// analytic gradient of the fine octave (for the grain-catches-light normal bend).
fn ground_grain(world_xz: vec2<f32>) -> vec3<f32> {
    let broad = value_noise(octave_frame_broad(world_xz) * 0.4);
    let fine = value_noise_grad(octave_frame_fine(world_xz) * 1.7);
    // Chain rule: grad_world = scale * R^T * grad_lattice for frame R = [[0.8, 0.6], [-0.6, 0.8]].
    let g = fine.yz * 1.7;
    return vec3<f32>(
        broad * 0.6 + fine.x * 0.4,
        0.8 * g.x - 0.6 * g.y,
        0.6 * g.x + 0.8 * g.y,
    );
}

// The interior grain (Hala v4 P5): the fine octave of `ground_grain` ALONE, because that is
// all the interior bend ever read — its `.x` was discarded by the C1 arm, so the broad
// octave was a lattice evaluation per interior pixel thrown away. Same frame, same scale,
// same chain rule as ground_grain's fine half: the returned gradient is BIT-IDENTICAL to
// ground_grain(p).yz, which is what lets the garage goldens prove this cut byte-for-byte.
// x = the fine octave's value (unused by the bend; kept so a future sheen can share the
// evaluation instead of paying a fourth one).
fn interior_grain(world_xz: vec2<f32>) -> vec3<f32> {
    let fine = value_noise_grad(octave_frame_fine(world_xz) * 1.7);
    let g = fine.yz * 1.7;
    return vec3<f32>(fine.x, 0.8 * g.x - 0.6 * g.y, 0.6 * g.x + 0.8 * g.y);
}

// The near-eye micro octave (~31 cm crumb): x = value, yz = world-space analytic gradient.
// Shares the broad frame's rotation (different scale — the lattices cannot align anyway).
fn micro_grain(world_xz: vec2<f32>) -> vec3<f32> {
    let m = value_noise_grad(octave_frame_broad(world_xz) * 3.2);
    // R = [[0.96, -0.28], [0.28, 0.96]] -> R^T folds the lattice gradient back to world axes.
    let g = m.yz * 3.2;
    return vec3<f32>(m.x, 0.96 * g.x + 0.28 * g.y, -0.28 * g.x + 0.96 * g.y);
}

// --- Footprint filtering (Inny Poziom T3 / O1) -------------------------------------------------
//
// A procedural octave has no mipmaps. A texture seen at a grazing angle is blurred by its mip
// chain; a lattice evaluated per fragment is sampled at ONE point no matter how many metres
// of ground the fragment covers. From a tank's eye 3 m up, the depth footprint of a pixel
// grows with the square of the distance (~0.3 m at 30 m, ~2.5 m at 100 m), so the 0.6 m fine
// octave and its light-catching gradient beat against the pixel grid from ~30 m out — the
// concentric "fingerprint" ripples the owner photographed on flat meadow. The fix is the
// analytic twin of a mip: each octave fades to its mean once its period covers fewer than
// GRAIN_SHOWN_PX_PER_PERIOD pixels, and is gone at Nyquist (GRAIN_GONE_PX_PER_PERIOD). Fading
// to the MEAN (0.5) keeps the average albedo, so nothing steps in brightness where an octave
// leaves. Periods are 1 / the lattice scales in ground_grain / micro_grain; the constants are
// mirrored in renderer_api (`GRAIN_*`) and locked by wgsl_layout.
const GRAIN_BROAD_PERIOD_M: f32 = 2.5;
const GRAIN_FINE_PERIOD_M: f32 = 0.588;
const GRAIN_MICRO_PERIOD_M: f32 = 0.3125;
const GRAIN_SHOWN_PX_PER_PERIOD: f32 = 4.0;
const GRAIN_GONE_PX_PER_PERIOD: f32 = 2.0;

// How much of an octave of `period_m` a fragment covering `footprint_m` metres may show.
fn octave_reach(period_m: f32, footprint_m: f32) -> f32 {
    return 1.0 - smoothstep(
        period_m / GRAIN_SHOWN_PX_PER_PERIOD,
        period_m / GRAIN_GONE_PX_PER_PERIOD,
        footprint_m,
    );
}

// `ground_grain` filtered by the fragment's footprint: same frames, same scales, same chain
// rule — each octave scaled toward its mean by its own reach, and NOT evaluated at all once
// its reach is zero (a far fragment pays for no lattice it cannot show).
fn ground_grain_filtered(world_xz: vec2<f32>, footprint_m: f32) -> vec3<f32> {
    let broad_reach = octave_reach(GRAIN_BROAD_PERIOD_M, footprint_m);
    let fine_reach = octave_reach(GRAIN_FINE_PERIOD_M, footprint_m);
    var broad = 0.5;
    if (broad_reach > 0.001) {
        broad = mix(0.5, value_noise(octave_frame_broad(world_xz) * 0.4), broad_reach);
    }
    var fine = vec3<f32>(0.5, 0.0, 0.0);
    if (fine_reach > 0.001) {
        let f = value_noise_grad(octave_frame_fine(world_xz) * 1.7);
        fine = vec3<f32>(mix(0.5, f.x, fine_reach), f.yz * 1.7 * fine_reach);
    }
    return vec3<f32>(
        broad * 0.6 + fine.x * 0.4,
        0.8 * fine.y - 0.6 * fine.z,
        0.6 * fine.y + 0.8 * fine.z,
    );
}

// `micro_grain` filtered the same way.
fn micro_grain_filtered(world_xz: vec2<f32>, footprint_m: f32) -> vec3<f32> {
    let reach = octave_reach(GRAIN_MICRO_PERIOD_M, footprint_m);
    if (reach <= 0.001) {
        return vec3<f32>(0.5, 0.0, 0.0);
    }
    let m = value_noise_grad(octave_frame_broad(world_xz) * 3.2);
    let g = m.yz * 3.2 * reach;
    return vec3<f32>(mix(0.5, m.x, reach), 0.96 * g.x + 0.28 * g.y, -0.28 * g.x + 0.96 * g.y);
}

// Break the square lattice of value_noise before thresholding it into rain pools. Two rotated
// scales keep the broad patches coherent while the finer scale erodes their grid-aligned edges.
fn puddle_pool(world_xz: vec2<f32>, fill: f32) -> f32 {
    let edge_p = vec2(
        world_xz.x * 0.197 + world_xz.y * -0.151,
        world_xz.x * 0.151 + world_xz.y * 0.197,
    ) + vec2<f32>(19.7, 43.1);
    let edge = value_noise(edge_p);
    let warp = (edge - 0.5) * 5.5;
    let warped = world_xz + vec2<f32>(warp, warp * -0.73);
    let broad_p = vec2(
        warped.x * 0.110 + warped.y * 0.071,
        warped.x * -0.071 + warped.y * 0.110,
    );
    let basin = value_noise(broad_p) * 0.72 + edge * 0.28;
    let threshold = mix(0.80, 0.54, clamp(fill, 0.0, 1.0));
    return smoothstep(threshold, threshold + 0.16, basin);
}

// --- Screen-door LOD cross-fade ----------------------------------------------------------------
// An ordered 4x4 Bayer threshold per screen pixel, 1/32 .. 31/32. The instance's window
// `[lo, hi)` is its share of it; the rungs (and the impostor's two quads) of one tree
// partition [0, 1), so every pixel is drawn by exactly one of them — a swap becomes a 20 m
// grain instead of a pop, and a crossed impostor never doubles its silhouette.
fn bayer4_threshold(frag: vec2<f32>) -> f32 {
    let x = u32(frag.x) & 3u;
    let y = u32(frag.y) & 3u;
    // The 4x4 Bayer matrix by bit interleaving: index = reverse2(x ^ y) | reverse2(y) style.
    let a = x ^ y;
    let index = ((a & 1u) << 3u) | ((y & 1u) << 2u) | ((a & 2u) << 0u) | ((y & 2u) >> 1u);
    return (f32(index) + 0.5) / 16.0;
}

// `window` is the instance's [lo, hi) share of the threshold; [0, 1] keeps every pixel.
fn dither_keeps(window: vec2<f32>, frag: vec2<f32>) -> bool {
    if (window.x <= 0.0 && window.y >= 1.0) {
        return true;
    }
    let threshold = bayer4_threshold(frag);
    return threshold >= window.x && threshold < window.y;
}
