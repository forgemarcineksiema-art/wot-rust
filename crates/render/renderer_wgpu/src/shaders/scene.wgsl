struct Camera {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    ambient_rgb: vec3<f32>,
    ground_ambient_rgb: vec3<f32>,
    key_direction: vec3<f32>,
    key_rgb: vec3<f32>,
    fill_direction: vec3<f32>,
    fill_rgb: vec3<f32>,
    rim_direction: vec3<f32>,
    rim_rgb: vec3<f32>,
    light_view_proj: mat4x4<f32>,
    shadow_params: vec4<f32>,
    ssao_params: vec4<f32>,
    sky_zenith_rgb: vec3<f32>,
    sky_horizon_rgb: vec3<f32>,
    fog_params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

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

// Hemispheric ambient: up-facing surfaces take the sky colour, down-facing surfaces the warmer
// ground bounce, blended by the normal's up fraction. This grounds a vehicle in its field instead
// of the old flat constant that flooded every face equally.
fn hemi_ambient(n: vec3<f32>) -> vec3<f32> {
    let t = clamp(n.y * 0.5 + 0.5, 0.0, 1.0);
    return mix(camera.ground_ambient_rgb, camera.ambient_rgb, t);
}

// Hemispheric ambient plus key/fill/rim directional terms. Directions point towards each light and
// are normalized here; an unlit (black) light contributes nothing.
fn scene_radiance(n: vec3<f32>, shadow: f32) -> vec3<f32> {
    let key = max(dot(n, normalize(camera.key_direction)), 0.0) * shadow;
    let fill = max(dot(n, normalize(camera.fill_direction)), 0.0);
    let rim = max(dot(n, normalize(camera.rim_direction)), 0.0);
    return hemi_ambient(n)
        + camera.key_rgb * key
        + camera.fill_rgb * fill
        + camera.rim_rgb * rim;
}

// Aerial perspective: fade a fragment's HDR radiance toward the horizon/sky colour by distance and
// height, so a 1000 m map reads with real depth instead of as cardboard cut-outs at range. Applied
// in linear HDR *before* the tone curve, and mirrored on the CPU by SceneLighting::fog_factor.
// fog_params = (density, height falloff, 0, 0); density 0 disables it (interior looks).
fn apply_fog(color: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let density = camera.fog_params.x;
    if (density <= 0.0) {
        return color;
    }
    let distance = length(camera.camera_pos - world_pos);
    let height_term = exp(-max(world_pos.y, 0.0) * camera.fog_params.y);
    let fog = clamp(1.0 - exp(-max(distance, 0.0) * density * height_term), 0.0, 1.0);
    return mix(color, camera.sky_horizon_rgb, fog);
}

// Filmic ACES-lite tone curve (Narkowicz fit): maps HDR radiance to display range so a hot sun and
// specular roll off instead of clipping to white. The framebuffer is *UnormSrgb, so the hardware
// does the linear->sRGB encode; we output linear, tone-mapped colour and never a manual sRGB pow.
fn tonemap_aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) tint_weight: f32,
    @location(4) model_0: vec4<f32>,
    @location(5) model_1: vec4<f32>,
    @location(6) model_2: vec4<f32>,
    @location(7) model_3: vec4<f32>,
    @location(8) tint: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) world_pos: vec3<f32>,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    let world = model * vec4<f32>(input.position, 1.0);
    out.clip = camera.view_proj * world;
    out.world_pos = world.xyz;
    out.normal = (model * vec4<f32>(input.normal, 0.0)).xyz;
    // Team colour is a per-instance tint, applied only where the vertex opted in (armour);
    // detail materials (barrel, tracks, rubber) carry tint_weight 0 and keep their base colour.
    let tint = mix(vec3<f32>(1.0, 1.0, 1.0), input.tint.rgb, input.tint_weight);
    out.color = input.color * tint;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(input.normal);
    let shadow = sun_shadow(input.world_pos, n);
    let ao = screen_ao(input.clip);
    let lit = input.color * scene_radiance(n, shadow) * ao;
    return vec4<f32>(tonemap_aces(apply_fog(lit, input.world_pos)), 1.0);
}
