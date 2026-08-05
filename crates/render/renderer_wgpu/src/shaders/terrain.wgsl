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

// The ground grain, its light-catch gradient, the puddle field and the cloud shade all live
// in the shared fragments (noise_common.wgsl, shadow_common.wgsl) — the terrain and the
// statics standing on it must read ONE implementation or their grains drift apart.

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let geometric_n = normalize(input.normal);
    let uv = input.world_pos.xz / materials.params.xy;

    // Layer weights from the splat map, renormalized against filtering drift.
    var w = textureSample(splat_map, ground_sampler, uv);
    w = w / max(w.r + w.g + w.b + w.a, 1.0e-4);

    // Ziemia 2.0, pasmo makro: worked land is FIELDS, not one lawn. A low-frequency noise
    // pair names a ~50-100 m plot, every plot holds its own tone (lusher, drier, lighter,
    // darker) and the borders blend over a few metres like real headlands. Only the two
    // vegetation layers move - dirt roads and rock cut through the quilt unchanged.
    var field_light = 1.0;
    // Teren B1 (bought back through the redemption gate): each worked plot ploughs in its
    // own direction. The lanes are declared here and evaluated after the eye distance is
    // in hand; a plot names its furrow direction from its OWN hash — never a world axis.
    var furrow_dir = vec2<f32>(1.0, 0.0);
    var furrow_mask = 0.0;
    let field_strength = materials.params.w;
    if (field_strength > 0.001) {
        let cells = value_noise(input.world_pos.xz / 90.0) * 4.0
            + value_noise(input.world_pos.xz / 34.0 + vec2<f32>(17.3, 41.7)) * 0.6;
        let cell = floor(cells);
        let border = smoothstep(0.82, 1.0, fract(cells));
        // Two independent lanes per plot: how DRY it stands and how LIGHT it reads — a dark
        // dry stubble and a pale lush meadow are both real land.
        let dry = mix(
            detail_hash(vec2<f32>(cell, cell * 1.7)),
            detail_hash(vec2<f32>(cell + 1.0, (cell + 1.0) * 1.7)),
            border,
        );
        let light = mix(
            detail_hash(vec2<f32>(cell * 3.1 + 7.0, cell)),
            detail_hash(vec2<f32>((cell + 1.0) * 3.1 + 7.0, cell + 1.0)),
            border,
        );
        // Some plots stand dry, some lush: redistribute grass <-> straw per plot...
        let lean = (dry - 0.5) * field_strength;
        let to_straw = max(lean, 0.0) * 0.9 * w.r;
        let to_grass = max(-lean, 0.0) * 0.9 * w.g;
        w = vec4<f32>(w.r - to_straw + to_grass, w.g + to_straw - to_grass, w.b, w.a);
        // ...and drift the plot's lightness on the vegetation share only.
        field_light = 1.0 + (light - 0.5) * 0.34 * field_strength * (w.r + w.g);
        // The plot's cultivation direction: quantized to 8 lanes over a half-turn
        // (furrows are bidirectional), from the plot hash — headlands (borders) and
        // unworked ground fade it out. Structure, not an octave: it rides the quilt's
        // plot identity the way the tone lanes above do.
        if (detail_bit(64u)) {
            let plough_lane = detail_hash(vec2<f32>(cell * 5.7 + 13.1, cell * 2.3 + 7.9));
            let plough_angle = floor(plough_lane * 8.0) * 0.39269908;
            furrow_dir = vec2<f32>(cos(plough_angle), sin(plough_angle));
            furrow_mask = field_strength * (1.0 - border) * (w.r + w.g);
        }
    }

    // The baked macro normal (~1 m relief) leaned into by the profile's strength; the detail
    // octaves then bend it further exactly like the scene pass.
    let packed = textureSample(macro_normal_map, ground_sampler, uv);
    let macro_n = normalize(packed.xyz * 2.0 - vec3<f32>(1.0));
    let base_n = normalize(mix(geometric_n, macro_n, materials.params.z));

    let wet = clamp(camera.time_params.z, 0.0, 1.0);
    let fill = clamp(camera.weather_params.z, 0.0, 1.0);
    let pool = puddle_pool(input.world_pos.xz, fill);
    let puddle = packed.a * fill * pool * 0.38;
    // The vertex lane carries the baked steepness/road/riverbed gloss; the chalk break adds
    // its own mineral sheen where its layer dominates.
    let layer_gloss = dot(w, materials.layer_gloss);
    let gloss = clamp(max(input.gloss, layer_gloss) + wet * 0.08 + puddle, 0.0, 1.0);

    // Detail: the scene pass's ground/strata mix, amplitude blended per layer. One shared
    // evaluation carries the albedo grain AND the analytic gradient the normal bend reads.
    let grain = ground_grain(input.world_pos.xz);
    let ground = grain.x;
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
    // Analytic (rides the ground_grain evaluation above): no extra lattice samples, and no
    // finite-difference faceting. The reduced tier (time_params.w, F2) keeps the albedo
    // grain and folds only its light-catching micro-relief.
    var bend = vec3<f32>(0.0);
    if (detail_bit(1u)) {
        bend = vec3<f32>(-grain.y, 0.0, -grain.z) * 0.12 * clamp(1.0 - gloss, 0.35, 1.0);
    }

    // Ziemia 2.0, pasmo mikro: a third, finer octave (~31 cm crumb — inside the art policy's
    // micro window of 0.3-0.6 m; the first cut ran ~20 cm and violated rule 5) near the
    // eye - the far field pays nothing and the near field stops reading as one woven carpet.
    var micro_shade = 1.0;
    let eye_dist = length(camera.camera_pos - input.world_pos);
    let near_amp = (1.0 - smoothstep(20.0, 55.0, eye_dist)) * select(0.0, 1.0, detail_bit(2u));
    if (near_amp > 0.004) {
        let micro = micro_grain(input.world_pos.xz);
        micro_shade = 1.0 + (micro.x - 0.5) * 0.11 * near_amp * amp;
        bend += vec3<f32>(-micro.y, 0.0, -micro.z) * 0.05 * near_amp;
    }
    // Teren B1: the furrow wave — ~1.25 m anisotropic stripes ACROSS the plough direction,
    // read as both an albedo ripple and a normal corrugation, gone by 150 m so the far
    // field never shimmers (rule 5's no-noise clause is the gate this feature answered).
    var furrow_shade = 1.0;
    if (furrow_mask > 0.001) {
        let across = dot(input.world_pos.xz, vec2<f32>(-furrow_dir.y, furrow_dir.x));
        let phase = across * 5.0265482;
        let reach = (1.0 - smoothstep(60.0, 150.0, eye_dist)) * furrow_mask;
        furrow_shade = 1.0 + sin(phase) * 0.06 * reach;
        bend += vec3<f32>(-furrow_dir.y, 0.0, furrow_dir.x) * cos(phase) * 0.10 * reach;
    }
    let n = normalize(base_n + bend);

    var albedo = materials.layers[0].rgb * w.r
        + materials.layers[1].rgb * w.g
        + materials.layers[2].rgb * w.b
        + materials.layers[3].rgb * w.a;
    albedo = albedo * detail_factor * field_light * micro_shade * furrow_shade;
    // Costume C (Jedna Trawa P5): the ground carries the meadow's own darkness — a little
    // where tufts still stand in front of it, all of it where the far costume has folded
    // away. The shared `meadow_far_stand` is the SAME curve the scene pass folds those
    // tufts on, so the collapse dissolves into tone instead of ending at a horizon where
    // grass stops. Vegetation-weighted from the splat both passes read, so roads, rock and
    // the riverbed keep their own colour — and because it rides a mipmapped texture rather
    // than a procedural octave, the far field gains no shimmer (rule 5).
    albedo = albedo * meadow_ground_shade(w.r + w.g, eye_dist);
    // The submerged riverbed: the baked depth tint wins by the vertex lane.
    albedo = mix(albedo, input.color, clamp(input.vertex_dominance, 0.0, 1.0));
    albedo = albedo * mix(1.0, 0.62, wet);

    let shadow =
        sun_shadow(input.world_pos, geometric_n, input.clip) * cloud_shadow(input.world_pos);
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
