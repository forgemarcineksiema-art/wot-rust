// Shared sun-shadow and screen-space-AO sampling for the lit geometry passes (scene, vehicle).
// Composed after camera_common.wgsl; declares the group-2 environment bindings, so only pipelines
// whose layout carries the shadow/SSAO bind group may include this file (the sky, water and rain
// passes must not — their layouts have no group 2).

@group(2) @binding(0)
var shadow_map: texture_depth_2d;
@group(2) @binding(1)
var shadow_sampler: sampler_comparison;
@group(2) @binding(2)
var ssao_tex: texture_2d<f32>;
@group(2) @binding(3)
var ssao_sampler: sampler;
@group(2) @binding(4)
var shadow_map_far: texture_depth_2d;

// Screen-space AO from the blurred SSAO target, addressed by framebuffer pixel scaled by the
// inverse RENDER target size (fog_params.zw) — the AO chain may run at a reduced resolution
// (half on integrated GPUs), so the texture's own dimensions are not the frame's. Strength 0
// (the capability fallback) returns fully open. Applied to the indirect (ambient/fill/env)
// terms only; the key light is occluded by the shadow map, not by screen-space AO.
fn screen_ao(frag: vec4<f32>) -> f32 {
    if (camera.ssao_params.z <= 0.0) {
        return 1.0;
    }
    return textureSampleLevel(ssao_tex, ssao_sampler, frag.xy * camera.fog_params.zw, 0.0).r;
}

// Two-cascade sun-shadow lookup. Only the key light is occluded; strength 0 (the capability
// fallback) returns fully lit. Cascade selection is by containment, not split distance: a fragment
// whose near-box UV sits inside the margin samples the crisp near map with a 3×3 PCF; everything
// else falls through to the far cascade (terrain/static casters only, 2×2 PCF — its softness sits
// out where the aerial haze lives). cascade_params.z < 2 disables the far lookup, and its margin
// lane is 0 then, so a single cascade behaves exactly like the pre-cascade shadow.
// Uses the Level sample variants so the lookup is valid outside uniform control flow.
fn sun_shadow(world_pos: vec3<f32>, n: vec3<f32>) -> f32 {
    let strength = camera.shadow_params.z;
    if (strength <= 0.0) {
        return 1.0;
    }
    // Near cascade: shadow_params = (texel UV step, depth bias, strength, world normal offset).
    let biased = world_pos + n * camera.shadow_params.w;
    let clip = camera.light_view_proj * vec4<f32>(biased, 1.0);
    let ndc = clip.xyz / clip.w;
    let uv = ndc.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    let margin = camera.cascade_params.w;
    if (uv.x >= margin && uv.x <= 1.0 - margin && uv.y >= margin && uv.y <= 1.0 - margin
        && ndc.z >= 0.0 && ndc.z <= 1.0) {
        let texel = camera.shadow_params.x;
        let reference = ndc.z - camera.shadow_params.y;
        var sum = 0.0;
        // Full detail: 3×3 PCF. The reduced tier (time_params.w, F2) trims to the 2×2 core —
        // four taps instead of nine; the far cascade below is 2×2 on every tier already.
        var lo = -1;
        if (camera.time_params.w < 0.5) {
            lo = 0;
        }
        for (var i = lo; i <= 1; i = i + 1) {
            for (var j = lo; j <= 1; j = j + 1) {
                let off = vec2<f32>(f32(i), f32(j)) * texel;
                sum = sum
                    + textureSampleCompareLevel(shadow_map, shadow_sampler, uv + off, reference);
            }
        }
        return mix(1.0, sum / 9.0, strength);
    }
    if (camera.cascade_params.z < 2.0) {
        return 1.0;
    }
    // Far cascade: its own (coarser) texel-derived normal offset, the same NDC depth bias.
    let biased_far = world_pos + n * camera.cascade_params.y;
    let clip_far = camera.light_view_proj_far * vec4<f32>(biased_far, 1.0);
    let ndc_far = clip_far.xyz / clip_far.w;
    let uv_far = ndc_far.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    if (uv_far.x < 0.0 || uv_far.x > 1.0 || uv_far.y < 0.0 || uv_far.y > 1.0
        || ndc_far.z < 0.0 || ndc_far.z > 1.0) {
        return 1.0;
    }
    let texel_far = camera.cascade_params.x;
    // The NDC depth bias is worth bias * depth_span metres of forgiveness. The far box is 3.7x
    // deeper than it was born (300 m -> 1100 m, to contain every occluder on a 1000 m map so
    // nothing pancakes to depth 0 and blankets the field in false shadow); scaling the shared
    // bias back down keeps the far cascade's WORLD-space forgiveness at its calibrated ~0.5 m
    // — a 1.2 m plate still casts, grazing terrain still doesn't acne. Keep in lockstep with
    // SunShadowParams::far_cascade (renderer_api/sun_shadow.rs).
    let reference_far = ndc_far.z - camera.shadow_params.y * (300.0 / 1100.0);
    var sum_far = 0.0;
    for (var i = 0; i < 2; i = i + 1) {
        for (var j = 0; j < 2; j = j + 1) {
            let off = (vec2<f32>(f32(i), f32(j)) - vec2<f32>(0.5, 0.5)) * texel_far;
            sum_far = sum_far
                + textureSampleCompareLevel(
                    shadow_map_far, shadow_sampler, uv_far + off, reference_far);
        }
    }
    return mix(1.0, sum_far / 4.0, strength);
}
