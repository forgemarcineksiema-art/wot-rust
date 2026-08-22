// Screen-space ambient occlusion from the camera depth prepass, plus its box blur.
//
// fs_ssao: for each pixel, linearize the prepass depth and take a golden-spiral of taps at a
// world-scaled pixel radius; nearer samples occlude with a distance-falloff range check. Writes
// the AO factor (1 open, darker in creases) into an R8 target. fs_blur: a 3x3 box that removes the
// spiral's noise pattern. Both are gated by camera.ssao_params.z (0 = capability fallback).
// Composed after camera_common.wgsl (the shared camera uniform declaration; this pass reads
// ssao_params as x = near plane, y = far plane, z = strength, w = projection Y scale P[1][1]).

@group(1) @binding(0)
var prepass_depth: texture_depth_2d;

@group(2) @binding(0)
var blur_src: texture_2d<f32>;

// A fullscreen triangle from the vertex index alone — no vertex buffer.
@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(index) / 2) * 4.0 - 1.0;
    let y = f32(i32(index) % 2) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

// View-space distance from a 0..1 reverse-projected depth (standard perspective, wgpu depth range).
fn linear_depth(d: f32) -> f32 {
    let near = camera.ssao_params.x;
    let far = camera.ssao_params.y;
    return near * far / (far + d * (near - far));
}

const TAPS: i32 = 12;
const RADIUS_M: f32 = 0.55;
const BIAS_M: f32 = 0.035;
const RANGE_M: f32 = 0.9;

@fragment
fn fs_ssao(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let strength = camera.ssao_params.z;
    let dims = vec2<i32>(textureDimensions(prepass_depth));
    let pix = vec2<i32>(frag.xy);
    let d = textureLoad(prepass_depth, pix, 0);
    if (strength <= 0.0 || d >= 1.0) {
        return vec4<f32>(1.0);
    }
    let lin = linear_depth(d);
    // Interiors take a sparser spiral (Hala v4 P3): eight taps instead of twelve, at the
    // UNCHANGED radius — the footprint is the room's ambient shade mass, tuned by eye at the
    // #554 relight, and a first cut that halved it visibly lightened the corners the mood
    // lives in, so the radius stays and only the sample count pays. The 3x3 blur was already
    // smearing twelve-tap noise; it smears eight-tap noise the same way. Interior = the same
    // fog test every interior branch keys on (fog_params.x <= 0); outdoors the count is the
    // shipped constant and the math is bit-exact.
    let taps = select(TAPS, 8, camera.fog_params.x <= 0.0);
    // World radius projected to pixels at this depth; clamped so the spiral neither vanishes in
    // the distance nor spans the whole screen up close.
    let radius_px = clamp(
        RADIUS_M * camera.ssao_params.w * f32(dims.y) / (2.0 * lin),
        2.0,
        48.0,
    );
    // Interleaved gradient noise rotates the spiral per pixel, trading banding for blurable noise.
    let noise = fract(52.9829189 * fract(dot(frag.xy, vec2<f32>(0.06711056, 0.00583715))));
    var occlusion = 0.0;
    for (var i = 0; i < taps; i = i + 1) {
        let angle = (f32(i) + noise) * 2.39996; // golden angle
        let reach = radius_px * sqrt((f32(i) + 0.5) / f32(taps));
        let offset = vec2<f32>(cos(angle), sin(angle)) * reach;
        let tap = clamp(pix + vec2<i32>(offset), vec2<i32>(0), dims - vec2<i32>(1));
        let sample_lin = linear_depth(textureLoad(prepass_depth, tap, 0));
        let closer_by = lin - sample_lin;
        // A sample occludes when it sits meaningfully in front, fading out past the AO range so
        // distant foreground silhouettes don't halo everything behind them.
        occlusion += clamp((closer_by - BIAS_M) / 0.08, 0.0, 1.0)
            * clamp(1.0 - (closer_by - BIAS_M) / RANGE_M, 0.0, 1.0);
    }
    let ao = 1.0 - strength * 0.85 * (occlusion / f32(taps));
    return vec4<f32>(clamp(ao, 0.0, 1.0));
}

@fragment
fn fs_blur(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<i32>(textureDimensions(blur_src));
    let pix = vec2<i32>(frag.xy);
    var sum = 0.0;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let tap = clamp(pix + vec2<i32>(dx, dy), vec2<i32>(0), dims - vec2<i32>(1));
            sum = sum + textureLoad(blur_src, tap, 0).r;
        }
    }
    return vec4<f32>(sum / 9.0);
}
