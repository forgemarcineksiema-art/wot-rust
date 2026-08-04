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

// The near-eye micro octave (~31 cm crumb): x = value, yz = world-space analytic gradient.
// Shares the broad frame's rotation (different scale — the lattices cannot align anyway).
fn micro_grain(world_xz: vec2<f32>) -> vec3<f32> {
    let m = value_noise_grad(octave_frame_broad(world_xz) * 3.2);
    // R = [[0.96, -0.28], [0.28, 0.96]] -> R^T folds the lattice gradient back to world axes.
    let g = m.yz * 3.2;
    return vec3<f32>(m.x, 0.96 * g.x + 0.28 * g.y, -0.28 * g.x + 0.96 * g.y);
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
