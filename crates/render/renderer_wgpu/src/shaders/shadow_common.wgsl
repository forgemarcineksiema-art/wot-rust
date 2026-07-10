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

// Screen-space AO from the blurred SSAO target, addressed by framebuffer pixel. Strength 0 (the
// capability fallback) returns fully open.
fn screen_ao(frag: vec4<f32>) -> f32 {
    if (camera.ssao_params.z <= 0.0) {
        return 1.0;
    }
    let dims = vec2<f32>(textureDimensions(ssao_tex));
    return textureSampleLevel(ssao_tex, ssao_sampler, frag.xy / dims, 0.0).r;
}

// Focused sun-shadow lookup with a 3x3 PCF. Only the key light is occluded; strength 0 (the
// capability fallback) returns fully lit. shadow_params = (texel UV step, depth bias, strength,
// world normal offset). Uses the Level variant so it is valid outside uniform control flow.
fn sun_shadow(world_pos: vec3<f32>, n: vec3<f32>) -> f32 {
    let strength = camera.shadow_params.z;
    if (strength <= 0.0) {
        return 1.0;
    }
    let biased = world_pos + n * camera.shadow_params.w;
    let clip = camera.light_view_proj * vec4<f32>(biased, 1.0);
    let ndc = clip.xyz / clip.w;
    let uv = ndc.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || ndc.z > 1.0 || ndc.z < 0.0) {
        return 1.0;
    }
    let texel = camera.shadow_params.x;
    let reference = ndc.z - camera.shadow_params.y;
    var sum = 0.0;
    for (var i = -1; i <= 1; i = i + 1) {
        for (var j = -1; j <= 1; j = j + 1) {
            let off = vec2<f32>(f32(i), f32(j)) * texel;
            sum = sum + textureSampleCompareLevel(shadow_map, shadow_sampler, uv + off, reference);
        }
    }
    return mix(1.0, sum / 9.0, strength);
}
