// The central post pass — the ONE place the picture is formed (art-direction rule 7). The whole
// world (sky, terrain, vehicles, water, FX, rain) renders linear HDR radiance into an
// Rgba16Float target; this fullscreen pass applies the display transform (exposure -> ACES-lite
// -> profile grade, `lighting_common.wgsl`) and writes the final display-linear colour to the
// sRGB swapchain. Slots for bloom composite and vignette land here in later packages. The HUD
// draws AFTER this pass, un-graded — the UI is not part of the picture.
// Composed after camera_common.wgsl and lighting_common.wgsl (camera uniform, display transform).

@group(1) @binding(0) var hdr_input: texture_2d<f32>;
@group(1) @binding(1) var bloom_input: texture_2d<f32>;
@group(1) @binding(2) var bloom_sampler: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
};

// A single oversized triangle covering the framebuffer (the sky-pass pattern).
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    var out: VsOut;
    let x = f32(index / 2u) * 4.0 - 1.0;
    let y = f32(index % 2u) * 4.0 - 1.0;
    out.clip = vec4<f32>(x, y, 1.0, 1.0);
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    // The resolved HDR frame is exactly framebuffer-sized: load the texel under this fragment.
    let hdr = textureLoad(hdr_input, vec2<i32>(input.clip.xy), 0).rgb;
    // Threshold-free bloom: fold the blurred HDR ladder back over the sharp frame at the
    // profile's small weight BEFORE the curve — only genuinely hot energy visibly glows
    // (rule 6). fog_params.zw is the inverse render size; sky_params.z the profile weight.
    let uv = input.clip.xy * camera.fog_params.zw;
    let bloom = textureSampleLevel(bloom_input, bloom_sampler, uv, 0.0).rgb;
    var composed = hdr + bloom * camera.sky_params.z;
    // Crepuscular rays (haze_params.z, profile-gated): march a few taps from this pixel toward
    // the screen-space sun, accumulating only genuinely HDR energy (luma past 1.0 — the disc
    // and its halo), weighted down along the way. Painted light shafts through the evening
    // air; the honest cheap crepuscular, not a volumetric sim (rule 6: light is the bling).
    let ray_strength = camera.haze_params.z;
    if (ray_strength > 0.0) {
        let sun_world = camera.camera_pos + normalize(camera.key_direction) * 10000.0;
        let sun_clip = camera.view_proj * vec4<f32>(sun_world, 1.0);
        if (sun_clip.w > 0.0) {
            let sun_ndc = sun_clip.xy / sun_clip.w;
            let sun_uv = sun_ndc * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
            let frame = 1.0 / camera.fog_params.zw; // render size in pixels
            var accum = vec3<f32>(0.0);
            for (var i = 1; i <= 8; i = i + 1) {
                let t = f32(i) / 8.0;
                let sample_uv = mix(uv, sun_uv, t);
                if (sample_uv.x < 0.0 || sample_uv.x > 1.0
                    || sample_uv.y < 0.0 || sample_uv.y > 1.0) {
                    continue;
                }
                let px = vec2<i32>(sample_uv * frame);
                let sample_hdr = textureLoad(hdr_input, px, 0).rgb;
                let luma = dot(sample_hdr, vec3<f32>(0.2126, 0.7152, 0.0722));
                let hot = max(luma - 1.0, 0.0);
                accum += sample_hdr * (hot / max(luma, 1.0e-4)) * (1.0 - t * 0.7);
            }
            composed += accum / 8.0 * ray_strength;
        }
    }
    var out = tonemap_aces(composed);
    // Profile vignette after the grade (sky_params.w, bible-capped at 0.15): a gentle pull at
    // the frame's corners that seats the picture without ever reading as a lens.
    let ndc = uv * 2.0 - vec2<f32>(1.0);
    let vignette = 1.0 - camera.sky_params.w * smoothstep(0.55, 1.45, length(ndc));
    return vec4<f32>(out * vignette, 1.0);
}
