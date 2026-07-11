// Dual-Kawase bloom ladder for the central post pass (art-direction rule 6: light is the only
// bling). Threshold-free: the DOWNSAMPLE averages the raw linear-HDR frame, so only genuinely
// hot sources (the sun disc, tracers, fires, specular glints — values far past 1.0) survive
// the repeated averaging with enough energy to read as glow after the small composite weight.
// The UPSAMPLE tent-filters each mip back onto the finer one additively; the final composite
// weight lives in the lighting profile (`sky_params.z`) and is applied in post.wgsl.
// No camera uniform — each pass reads only its source mip.

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    var out: VsOut;
    let x = f32(index / 2u) * 4.0 - 1.0;
    let y = f32(index % 2u) * 4.0 - 1.0;
    out.clip = vec4<f32>(x, y, 1.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);
    return out;
}

// Kawase downsample: centre-weighted 5-tap diamond over the SOURCE texel grid — a cheap,
// stable low-pass whose repeated application approximates a wide gaussian.
@fragment
fn fs_down(input: VsOut) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(src));
    let o = texel; // half-pixel diamond in source space (we render at half the source size)
    var sum = textureSampleLevel(src, src_sampler, input.uv, 0.0).rgb * 4.0;
    sum += textureSampleLevel(src, src_sampler, input.uv + vec2<f32>(-o.x, -o.y), 0.0).rgb;
    sum += textureSampleLevel(src, src_sampler, input.uv + vec2<f32>(o.x, -o.y), 0.0).rgb;
    sum += textureSampleLevel(src, src_sampler, input.uv + vec2<f32>(-o.x, o.y), 0.0).rgb;
    sum += textureSampleLevel(src, src_sampler, input.uv + vec2<f32>(o.x, o.y), 0.0).rgb;
    return vec4<f32>(sum / 8.0, 1.0);
}

// Kawase upsample: 8-tap tent over the coarser mip, ADDED onto the finer one (the pipeline's
// additive blend), each level averaged so the ladder conserves energy instead of stacking it.
@fragment
fn fs_up(input: VsOut) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(src));
    let o = texel;
    var sum = vec3<f32>(0.0);
    sum += textureSampleLevel(src, src_sampler, input.uv + vec2<f32>(-o.x, 0.0), 0.0).rgb;
    sum += textureSampleLevel(src, src_sampler, input.uv + vec2<f32>(o.x, 0.0), 0.0).rgb;
    sum += textureSampleLevel(src, src_sampler, input.uv + vec2<f32>(0.0, -o.y), 0.0).rgb;
    sum += textureSampleLevel(src, src_sampler, input.uv + vec2<f32>(0.0, o.y), 0.0).rgb;
    sum += textureSampleLevel(src, src_sampler, input.uv + vec2<f32>(-o.x, -o.y), 0.0).rgb * 0.5;
    sum += textureSampleLevel(src, src_sampler, input.uv + vec2<f32>(o.x, -o.y), 0.0).rgb * 0.5;
    sum += textureSampleLevel(src, src_sampler, input.uv + vec2<f32>(-o.x, o.y), 0.0).rgb * 0.5;
    sum += textureSampleLevel(src, src_sampler, input.uv + vec2<f32>(o.x, o.y), 0.0).rgb * 0.5;
    // 6 effective taps; halve on the way up so the accumulated ladder stays energy-bounded.
    return vec4<f32>(sum / 12.0, 1.0);
}
