// FXAA over the FORMED picture — the one anti-aliasing every player gets. The one-look policy
// ships 1x MSAA on every adapter (the minimum spec cannot afford multisampling), which until
// now meant the shipped game had NO anti-aliasing at all; and in the dev-only rich profile the
// hardware MSAA resolve averages samples in linear HDR BEFORE the tone curve, so high-contrast
// edges collapse back to ~1 px staircases anyway. Filtering the ENCODED LDR picture — after
// exposure, ACES, grade and dither — is the textbook-correct place: luma here is perceptual,
// so the edge blend weights match what the eye sees.
//
// The algorithm is the classic Lottes fast-quality FXAA: a 5-tap luma cross gates the work
// (flat pixels pass through untouched — the dither grain survives), the diagonal luma gradient
// picks the blur direction, and a 2-tap/4-tap pair walks it, falling back to the short blend
// when the long one overshoots the local luma range. The HUD draws after this pass and is
// never softened.
//
// Reads the LDR intermediate (`post.wgsl` writes sRGB-ENCODED bytes into a plain Unorm
// texture, so samples arrive perceptual, exactly as FXAA wants); writes display-linear to the
// sRGB output target via `srgb_decode` (lighting_common.wgsl).

@group(1) @binding(0) var ldr_input: texture_2d<f32>;
@group(1) @binding(1) var ldr_sampler: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
};

// A single oversized triangle covering the framebuffer (the post-pass pattern).
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    var out: VsOut;
    let x = f32(index / 2u) * 4.0 - 1.0;
    let y = f32(index % 2u) * 4.0 - 1.0;
    out.clip = vec4<f32>(x, y, 1.0, 1.0);
    return out;
}

fn fxaa_luma(rgb: vec3<f32>) -> f32 {
    return dot(rgb, vec3<f32>(0.299, 0.587, 0.114));
}

// Contrast gates: below these the pixel is flat and passes through untouched.
const EDGE_THRESHOLD: f32 = 0.125;
const EDGE_THRESHOLD_MIN: f32 = 0.0312;
const SUBPIX_SHIFT: f32 = 0.25;

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let inv = camera.fog_params.zw;
    let uv = input.clip.xy * inv;
    // The 5-tap luma cross lands on EXACT texels, so it loads them directly instead of paying
    // the sampler's bilinear filter for a weight of 1.0. Measured on the battle probe: the
    // filtered version of this cross cost ~2.6 ms of a 5.0 ms pass at 1920x1080 — a whole
    // sixth of the min-spec frame budget spent interpolating between a texel and itself.
    // (Only the four BLEND taps below sit at fractional offsets and genuinely need filtering.)
    let px = vec2<i32>(input.clip.xy);
    let size = vec2<i32>(textureDimensions(ldr_input)) - vec2<i32>(1);
    let rgb_m = textureLoad(ldr_input, clamp(px, vec2<i32>(0), size), 0).rgb;
    let luma_m = fxaa_luma(rgb_m);
    let luma_nw =
        fxaa_luma(textureLoad(ldr_input, clamp(px + vec2<i32>(-1, -1), vec2<i32>(0), size), 0).rgb);
    let luma_ne =
        fxaa_luma(textureLoad(ldr_input, clamp(px + vec2<i32>(1, -1), vec2<i32>(0), size), 0).rgb);
    let luma_sw =
        fxaa_luma(textureLoad(ldr_input, clamp(px + vec2<i32>(-1, 1), vec2<i32>(0), size), 0).rgb);
    let luma_se =
        fxaa_luma(textureLoad(ldr_input, clamp(px + vec2<i32>(1, 1), vec2<i32>(0), size), 0).rgb);
    let luma_min = min(luma_m, min(min(luma_nw, luma_ne), min(luma_sw, luma_se)));
    let luma_max = max(luma_m, max(max(luma_nw, luma_ne), max(luma_sw, luma_se)));

    if (luma_max - luma_min < max(EDGE_THRESHOLD_MIN, luma_max * EDGE_THRESHOLD)) {
        return vec4<f32>(srgb_decode(rgb_m), 1.0);
    }

    var dir = vec2<f32>(
        -((luma_nw + luma_ne) - (luma_sw + luma_se)),
        ((luma_nw + luma_sw) - (luma_ne + luma_se)),
    );
    let dir_reduce = max((luma_nw + luma_ne + luma_sw + luma_se) * 0.25 * SUBPIX_SHIFT, 1.0 / 128.0);
    let rcp_dir_min = 1.0 / (min(abs(dir.x), abs(dir.y)) + dir_reduce);
    dir = clamp(dir * rcp_dir_min, vec2<f32>(-8.0), vec2<f32>(8.0)) * inv;

    let rgb_a = 0.5
        * (textureSampleLevel(ldr_input, ldr_sampler, uv + dir * (1.0 / 3.0 - 0.5), 0.0).rgb
            + textureSampleLevel(ldr_input, ldr_sampler, uv + dir * (2.0 / 3.0 - 0.5), 0.0).rgb);
    let rgb_b = rgb_a * 0.5
        + 0.25
            * (textureSampleLevel(ldr_input, ldr_sampler, uv + dir * -0.5, 0.0).rgb
                + textureSampleLevel(ldr_input, ldr_sampler, uv + dir * 0.5, 0.0).rgb);
    let luma_b = fxaa_luma(rgb_b);

    var result = rgb_b;
    if (luma_b < luma_min || luma_b > luma_max) {
        result = rgb_a;
    }
    return vec4<f32>(srgb_decode(result), 1.0);
}
