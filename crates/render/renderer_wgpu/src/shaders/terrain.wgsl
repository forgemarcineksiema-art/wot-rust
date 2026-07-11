// Terrain Material 2.0 ground pass (docs/art-direction-policy.md rules 2/5): the heightfield's
// albedo comes from four material layers (lush grass / dry straw / worn dirt / broken rock)
// weighted per-pixel by the baked splat map, and its lighting normal leans into the baked macro
// normal map (~1 m relief the 5 m grid cannot carry — raking evening light reads every hummock).
// Everything else — wetness, puddles, cloud shade, shadow/AO, specular, fog — is the scene
// pass's exact model, so ground and statics stay ONE picture. The submerged riverbed keeps its
// baked depth tint via the vertex tint lane (no splat equivalent exists for looking through
// water). Composed after camera_common.wgsl, lighting_common.wgsl and shadow_common.wgsl.

struct TerrainMaterials {
    // rgb = layer albedo, w = detail amplitude; R/G/B/A splat channel order.
    layers: array<vec4<f32>, 4>,
    // Per-layer specular lane, same channel order.
    layer_gloss: vec4<f32>,
    // xy = ground extent in metres (UV = world.xz / extent), z = macro normal strength.
    params: vec4<f32>,
};

@group(1) @binding(0) var splat_map: texture_2d<f32>;
@group(1) @binding(1) var macro_normal_map: texture_2d<f32>;
@group(1) @binding(2) var ground_sampler: sampler;
@group(1) @binding(3) var<uniform> materials: TerrainMaterials;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) tint_weight: f32,
    @location(9) gloss: f32,
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
    @location(3) gloss: f32,
    // How strongly the baked vertex colour overrides the splat albedo (the submerged riverbed).
    @location(4) vertex_dominance: f32,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    let world = model * vec4<f32>(input.position, 1.0);
    out.clip = camera.view_proj * world;
    out.world_pos = world.xyz;
    out.normal = (model * vec4<f32>(input.normal, 0.0)).xyz;
    out.color = input.color;
    out.gloss = input.gloss;
    out.vertex_dominance = input.tint_weight;
    return out;
}

// The scene pass's two-octave detail noise (same hash, same scales) so ground grain matches
// the statics standing on it.
fn detail_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = detail_hash(i);
    let b = detail_hash(i + vec2<f32>(1.0, 0.0));
    let c = detail_hash(i + vec2<f32>(0.0, 1.0));
    let d = detail_hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn cloud_shadow(world: vec3<f32>) -> f32 {
    let strength = camera.sky_params.x;
    if (strength <= 0.0) {
        return 1.0;
    }
    let drift = camera.time_params.x * camera.cloud_params.w;
    let uv = world.xz * (0.8 / 400.0) * camera.cloud_params.y + vec2<f32>(drift, drift * 0.6);
    let coverage = (value_noise(uv) * 0.6 + value_noise(uv * 2.0) * 0.4) + camera.cloud_params.x;
    let cloud = smoothstep(0.40, 0.72, coverage);
    return 1.0 - cloud * strength;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let geometric_n = normalize(input.normal);
    let uv = input.world_pos.xz / materials.params.xy;

    // Layer weights from the splat map, renormalized against filtering drift.
    var w = textureSample(splat_map, ground_sampler, uv);
    w = w / max(w.r + w.g + w.b + w.a, 1.0e-4);

    // The baked macro normal (~1 m relief) leaned into by the profile's strength; the detail
    // octaves then bend it further exactly like the scene pass.
    let packed = textureSample(macro_normal_map, ground_sampler, uv).xyz;
    let macro_n = normalize(packed * 2.0 - vec3<f32>(1.0));
    let base_n = normalize(mix(geometric_n, macro_n, materials.params.z));

    let wet = clamp(camera.time_params.z, 0.0, 1.0);
    let pool = smoothstep(0.58, 0.82, value_noise(input.world_pos.xz * 0.16));
    let puddle = smoothstep(0.985, 0.999, base_n.y) * wet * pool * 0.5;
    // The vertex lane carries the baked steepness/road/riverbed gloss; the chalk break adds
    // its own mineral sheen where its layer dominates.
    let layer_gloss = dot(w, materials.layer_gloss);
    let gloss = clamp(max(input.gloss, layer_gloss) + wet * 0.08 + puddle, 0.0, 1.0);

    // Detail: the scene pass's ground/strata mix, amplitude blended per layer.
    let ground = value_noise(input.world_pos.xz * 0.4) * 0.6
        + value_noise(input.world_pos.xz * 1.7) * 0.4;
    let strata = value_noise(vec2<f32>(
        input.world_pos.y * 2.2,
        (input.world_pos.x + input.world_pos.z) * 0.15,
    ));
    let steep = clamp(1.0 - base_n.y, 0.0, 1.0);
    let detail_mix = mix(ground, strata, steep * 0.7);
    let amp = dot(w, vec4<f32>(
        materials.layers[0].w,
        materials.layers[1].w,
        materials.layers[2].w,
        materials.layers[3].w,
    ));
    let detail_factor = 1.0 + (detail_mix * 0.16 - 0.08) * amp;

    // The detail-noise gradient bent into the normal (the scene pass's grain-catches-light).
    let e = 0.35;
    let here = value_noise(input.world_pos.xz * 1.7);
    let dx = value_noise((input.world_pos.xz + vec2<f32>(e, 0.0)) * 1.7) - here;
    let dz = value_noise((input.world_pos.xz + vec2<f32>(0.0, e)) * 1.7) - here;
    let bend = vec3<f32>(-dx, 0.0, -dz) * (0.12 / e) * clamp(1.0 - gloss, 0.35, 1.0);
    let n = normalize(base_n + bend);

    var albedo = materials.layers[0].rgb * w.r
        + materials.layers[1].rgb * w.g
        + materials.layers[2].rgb * w.b
        + materials.layers[3].rgb * w.a;
    albedo = albedo * detail_factor;
    // The submerged riverbed: the baked depth tint wins by the vertex lane.
    albedo = mix(albedo, input.color, clamp(input.vertex_dominance, 0.0, 1.0));
    albedo = albedo * mix(1.0, 0.62, wet);

    let shadow = sun_shadow(input.world_pos, geometric_n) * cloud_shadow(input.world_pos);
    let ao = screen_ao(input.clip);
    var lit = albedo * light_radiance(input.world_pos, n, shadow, ao);
    if (gloss > 0.001) {
        let view = normalize(camera.camera_pos - input.world_pos);
        let key = normalize(camera.key_direction);
        let halfway = normalize(key + view);
        let shininess = mix(16.0, 96.0, gloss);
        let lobe = pow(max(dot(n, halfway), 0.0), shininess);
        let fresnel = 0.25 + 0.75 * pow(1.0 - max(dot(n, view), 0.0), 5.0);
        let reflected = reflect(-view, n);
        lit += camera.key_rgb * lobe * gloss * shadow
            + env_sky(reflected) * gloss * gloss * fresnel * ao;
    }
    // Linear HDR out: the display transform lives in the central post pass (rule 7).
    return vec4<f32>(apply_fog(lit, input.world_pos), 1.0);
}
