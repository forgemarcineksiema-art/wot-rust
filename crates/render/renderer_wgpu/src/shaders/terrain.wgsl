// Terrain Material 2.0 ground pass (docs/art-direction-policy.md rules 2/5): the heightfield's
// albedo comes from four material layers (lush grass / dry straw / worn dirt / broken rock)
// weighted per-pixel by the baked splat map, and its lighting normal leans into the baked macro
// normal map (~1 m relief the 5 m grid cannot carry — raking evening light reads every hummock).
// Everything else — wetness, puddles, cloud shade, shadow/AO, specular, fog — is the scene
// pass's exact model, so ground and statics stay ONE picture. The submerged riverbed keeps its
// baked depth tint via the vertex tint lane (no splat equivalent exists for looking through
// water). Composed after camera_common.wgsl, lighting_common.wgsl and shadow_common.wgsl.
//
// The per-pixel diet (Inny Poziom Q7): the ground is the biggest share of `scene_pass`, and
// `scene_pass` is three quarters of the frame, so what this pass does per FRAGMENT is the
// frame. The field quilt is a ~50–100 m plot structure — it is evaluated per VERTEX (5 m grid)
// and interpolated; the strata noise is paid only on steep fragments, the puddle pool only when
// the look fills puddles, and every procedural octave (ground grain, micro crumb, furrow
// wave) is filtered by the fragment's PIXEL FOOTPRINT (T3/O1): an octave leaves once it is
// under four pixels per period, so a grazing eye never samples a lattice below Nyquist — the
// moiré ripples on flat meadow were exactly that. Near the eye the picture is the same; the
// fragment just stops paying for structure it cannot show.

struct TerrainMaterials {
    // rgb = layer albedo, w = detail amplitude; R/G/B/A splat channel order.
    layers: array<vec4<f32>, 4>,
    // Per-layer specular lane, same channel order.
    layer_gloss: vec4<f32>,
    // xy = ground extent in metres (UV = world.xz / extent), z = macro normal strength,
    // w = field-quilt strength.
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
    // The field quilt, evaluated per vertex (Q7): x = the plot's dry lean (already scaled by
    // the quilt strength), y = the plot's lightness drift, z = the furrow mask before the
    // vegetation share, w = unused.
    @location(5) quilt: vec4<f32>,
    // The plot's plough direction (unit, hull-independent), interpolated across the plot.
    @location(6) furrow_dir: vec2<f32>,
};

// Ziemia 2.0, pasmo makro: worked land is FIELDS, not one lawn. A low-frequency noise pair
// names a ~50-100 m plot, every plot holds its own tone (lusher, drier, lighter, darker) and
// the borders blend over a few metres like real headlands. Teren B1: each worked plot ploughs
// in its own direction, quantized to 8 lanes over a half-turn. Structure, not an octave — and
// since Q7 evaluated once per vertex, not once per pixel: the plot is 50 m wide and the grid
// 5 m, so the interpolation cannot show a seam the octave did not already blur.
fn field_quilt(world_xz: vec2<f32>) -> vec4<f32> {
    let field_strength = materials.params.w;
    if (field_strength <= 0.001) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let cells = value_noise(world_xz / 90.0) * 4.0
        + value_noise(world_xz / 34.0 + vec2<f32>(17.3, 41.7)) * 0.6;
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
    let lean = (dry - 0.5) * field_strength;
    let light_drift = (light - 0.5) * 0.34 * field_strength;
    let plough_lane = detail_hash(vec2<f32>(cell * 5.7 + 13.1, cell * 2.3 + 7.9));
    let plough_angle = floor(plough_lane * 8.0) * 0.39269908;
    let furrow_mask = field_strength * (1.0 - border);
    return vec4<f32>(lean, light_drift, furrow_mask, plough_angle);
}

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
    let quilt = field_quilt(world.xz);
    out.quilt = vec4<f32>(quilt.x, quilt.y, quilt.z, 0.0);
    out.furrow_dir = vec2<f32>(cos(quilt.w), sin(quilt.w));
    return out;
}

// The ground grain, its light-catch gradient, the puddle field and the cloud shade all live
// in the shared fragments (noise_common.wgsl, shadow_common.wgsl) — the terrain and the
// statics standing on it must read ONE implementation or their grains drift apart.

// The hard ceiling on the ground grain, in metres. The grain's REAL fade is by pixel
// footprint (`ground_grain_filtered`, T3/O1 — an octave leaves once it is under four pixels
// per period, whatever the lens); this distance cap only keeps the far apron from ever
// evaluating a lattice, and fades so the far field never pops.
const GRAIN_REACH_START_M: f32 = 300.0;
const GRAIN_REACH_END_M: f32 = 450.0;
// The furrow wave's period in metres (phase = across * 2π / period).
const FURROW_PERIOD_M: f32 = 1.25;

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let geometric_n = normalize(input.normal);
    let uv = input.world_pos.xz / materials.params.xy;
    let eye_dist = length(camera.camera_pos - input.world_pos);
    // The metres of ground this fragment covers: the longer of the two screen-axis
    // derivatives of the world position (the depth axis dominates at a grazing eye). Every
    // procedural octave below is filtered by it — the analytic twin of a texture's mip chain.
    let footprint = max(length(dpdx(input.world_pos.xz)), length(dpdy(input.world_pos.xz)));

    // Layer weights from the splat map, renormalized against filtering drift.
    var w = textureSample(splat_map, ground_sampler, uv);
    w = w / max(w.r + w.g + w.b + w.a, 1.0e-4);

    // The field quilt from the vertex stage: some plots stand dry, some lush — redistribute
    // grass <-> straw per plot, and drift the plot's lightness on the vegetation share only.
    let lean = input.quilt.x;
    let to_straw = max(lean, 0.0) * 0.9 * w.r;
    let to_grass = max(-lean, 0.0) * 0.9 * w.g;
    w = vec4<f32>(w.r - to_straw + to_grass, w.g + to_straw - to_grass, w.b, w.a);
    let field_light = 1.0 + input.quilt.y * (w.r + w.g);
    // The furrow lanes: headlands (borders) and unworked ground fade them out.
    var furrow_mask = 0.0;
    if (detail_bit(64u)) {
        furrow_mask = input.quilt.z * (w.r + w.g);
    }
    // Two plots ploughing in opposite lanes interpolate through a zero vector at their seam:
    // the seam takes the world axis, and the mask fades with the vector so no furrow reads there.
    let furrow_len = length(input.furrow_dir);
    let furrow_dir = select(vec2<f32>(1.0, 0.0), input.furrow_dir / max(furrow_len, 1.0e-3), furrow_len > 1.0e-3);
    furrow_mask = furrow_mask * clamp(furrow_len, 0.0, 1.0);

    // The baked macro normal (~1 m relief) leaned into by the profile's strength; the detail
    // octaves then bend it further exactly like the scene pass. The lean fades to ZERO at the
    // authored splat's edge: the apron mesh runs 1500 m past the border on the same pipeline,
    // and the clamped edge texel used to be stretched over all of it — the backdrop hills had
    // their shading normal pulled 65% toward one flat sample and lit like flat ground (the
    // form-less bright band at the horizon). Outside the map the geometry speaks for itself.
    let packed = textureSample(macro_normal_map, ground_sampler, uv);
    let macro_n = normalize(packed.xyz * 2.0 - vec3<f32>(1.0));
    let inside_splat = smoothstep(0.0, 0.02, uv.x) * (1.0 - smoothstep(0.98, 1.0, uv.x))
        * smoothstep(0.0, 0.02, uv.y) * (1.0 - smoothstep(0.98, 1.0, uv.y));
    let base_n = normalize(mix(geometric_n, macro_n, materials.params.z * inside_splat));

    let wet = clamp(camera.time_params.z, 0.0, 1.0);
    let fill = clamp(camera.weather_params.z, 0.0, 1.0);
    // The puddle field is a two-noise pool: paid only when the look fills puddles at all.
    var puddle = 0.0;
    if (fill > 0.001) {
        puddle = packed.a * fill * puddle_pool(input.world_pos.xz, fill) * 0.38;
    }
    // The vertex lane carries the baked steepness/road/riverbed gloss; the chalk break adds
    // its own mineral sheen where its layer dominates.
    let layer_gloss = dot(w, materials.layer_gloss);
    let gloss = clamp(max(input.gloss, layer_gloss) + wet * 0.08 + puddle, 0.0, 1.0);

    // Detail: the scene pass's ground/strata mix, amplitude blended per layer. One shared
    // evaluation carries the albedo grain AND the analytic gradient the normal bend reads —
    // and only inside the grain's reach; past it the ground is the splat and the macro relief.
    let amp = dot(w, vec4<f32>(
        materials.layers[0].w,
        materials.layers[1].w,
        materials.layers[2].w,
        materials.layers[3].w,
    ));
    let grain_reach = 1.0 - smoothstep(GRAIN_REACH_START_M, GRAIN_REACH_END_M, eye_dist);
    var grain = vec3<f32>(0.5, 0.0, 0.0);
    if (grain_reach > 0.004) {
        grain = ground_grain_filtered(input.world_pos.xz, footprint);
    }
    let ground = grain.x;
    let steep = clamp(1.0 - base_n.y, 0.0, 1.0);
    // The strata noise reads only on steep ground; a flat fragment pays nothing for it.
    var strata = ground;
    if (steep > 0.02) {
        strata = value_noise(vec2<f32>(
            input.world_pos.y * 2.2,
            (input.world_pos.x + input.world_pos.z) * 0.15,
        ));
    }
    let detail_mix = mix(ground, strata, steep * 0.7);
    let detail_factor = 1.0 + (detail_mix * 0.16 - 0.08) * amp * grain_reach;

    // The detail-noise gradient bent into the normal (the scene pass's grain-catches-light).
    // Analytic (rides the ground_grain evaluation above): no extra lattice samples, and no
    // finite-difference faceting. The reduced tier (time_params.w, F2) keeps the albedo
    // grain and folds only its light-catching micro-relief.
    var bend = vec3<f32>(0.0);
    if (detail_bit(1u)) {
        bend = vec3<f32>(-grain.y, 0.0, -grain.z) * 0.12 * clamp(1.0 - gloss, 0.35, 1.0)
            * grain_reach;
    }

    // Ziemia 2.0, pasmo mikro: a third, finer octave (~31 cm crumb — inside the art policy's
    // micro window of 0.3-0.6 m; the first cut ran ~20 cm and violated rule 5) near the
    // eye - the far field pays nothing and the near field stops reading as one woven carpet.
    // The 20–55 m band is the ceiling; the footprint filter inside `micro_grain_filtered`
    // ends the 31 cm crumb where it drops under four pixels per period (~20 m from a tank's
    // eye at the reference lens, farther in the scope).
    var micro_shade = 1.0;
    let near_amp = (1.0 - smoothstep(20.0, 55.0, eye_dist)) * select(0.0, 1.0, detail_bit(2u));
    if (near_amp > 0.004) {
        let micro = micro_grain_filtered(input.world_pos.xz, footprint);
        micro_shade = 1.0 + (micro.x - 0.5) * 0.11 * near_amp * amp;
        bend += vec3<f32>(-micro.y, 0.0, -micro.z) * 0.05 * near_amp;
    }
    // Teren B1: the furrow wave — ~1.25 m anisotropic stripes ACROSS the plough direction,
    // read as both an albedo ripple and a normal corrugation, gone by 150 m so the far
    // field never shimmers (rule 5's no-noise clause is the gate this feature answered).
    // A sine is an octave too: it takes the same footprint filter as the grain.
    var furrow_shade = 1.0;
    if (furrow_mask > 0.001 && eye_dist < 150.0) {
        let across = dot(input.world_pos.xz, vec2<f32>(-furrow_dir.y, furrow_dir.x));
        let phase = across * 5.0265482;
        let reach = (1.0 - smoothstep(60.0, 150.0, eye_dist)) * furrow_mask
            * octave_reach(FURROW_PERIOD_M, footprint);
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
