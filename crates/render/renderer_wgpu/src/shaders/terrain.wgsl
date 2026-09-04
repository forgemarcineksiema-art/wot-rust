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
// the look fills puddles.
//
// Teren 2.0 (T3, O1's ground half, Q7): the detail is a MATERIAL, not a lattice. Each splat
// layer has its own baked, tiling, mipmapped detail tile (`renderer_api::ground_detail` —
// tangent normal + shade + height; grass clumps, straw stubble, dirt clods, rock plates) and
// the mid-field's colour variation comes from a baked macro tone tile tapped at two scales.
// A texture with a mip chain is filtered by the hardware at every distance and through every
// lens — no per-fragment footprint fade, no octave that can alias — and the four layers blend
// by HEIGHT at their splat borders, so a dirt edge is where clumps stop poking through, not a
// filtered line. The procedural `noise_common.wgsl` grain stays for the statics' generic
// ground in the scene pass; the terrain fragment evaluates no lattice octave any more except
// the strata noise on steep ground.

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
// Teren 2.0: the detail material (four layers, splat order) and the macro tone tile, on a
// repeat + anisotropic sampler. Constants mirror `renderer_api::ground_detail`.
@group(1) @binding(4) var detail_tiles: texture_2d_array<f32>;
@group(1) @binding(5) var macro_tile: texture_2d<f32>;
@group(1) @binding(6) var detail_sampler: sampler;

const GROUND_TILE_PERIOD_M: f32 = 10.0;
const GROUND_MACRO_PERIOD_M: f32 = 160.0;
const GROUND_MACRO_FAR_RATIO: f32 = 3.83;
// How much a tile's shade lane (0.5 = flat) moves the layer albedo at full layer amplitude.
const DETAIL_SHADE_AMP: f32 = 0.40;
// The tile's tangent normal is bent into the lighting normal at this share of its slope
// (the bake's relief is metres-true; this is the art's say over how hard the grain catches
// light, the same knob the old grain had at 0.12 of a lattice gradient).
const DETAIL_BEND: f32 = 0.85;
// The macro tone tile's amplitude: ±12 % lightness at the extremes of its lanes.
const MACRO_TONE_AMP: f32 = 0.12;
// The height blend's sharpness: the weight of a layer is its splat weight times
// (height + HEIGHT_BLEND_FLOOR)^HEIGHT_BLEND_POWER, renormalized. Where one layer holds the
// whole splat nothing changes; at a border the taller material shows through.
const HEIGHT_BLEND_FLOOR: f32 = 0.15;
const HEIGHT_BLEND_POWER: f32 = 3.0;

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
    // the quilt strength), y = the plot's lightness drift. Both are TONES — smooth scalars a
    // 5 m interpolation cannot kink. Nothing directional rides this lane any more: the
    // "Teren B1" furrow sine did (a per-vertex plough angle interpolated across each
    // triangle), and it printed the owner's zigzag waves on every meadow of every map — see
    // the register's T6. Worked land comes back as a material, not as a sine.
    @location(5) quilt: vec2<f32>,
};

// Ziemia 2.0, pasmo makro: worked land is FIELDS, not one lawn. A low-frequency noise pair
// names a ~50-100 m plot, every plot holds its own tone (lusher, drier, lighter, darker) and
// the borders blend over a few metres like real headlands. Structure, not an octave — and
// since Q7 evaluated once per vertex, not once per pixel: the plot is 50 m wide and the grid
// 5 m, so the interpolation cannot show a seam the octave did not already blur.
fn field_quilt(world_xz: vec2<f32>) -> vec2<f32> {
    let field_strength = materials.params.w;
    if (field_strength <= 0.001) {
        return vec2<f32>(0.0, 0.0);
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
    return vec2<f32>(lean, light_drift);
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
    out.quilt = field_quilt(world.xz);
    return out;
}

// The puddle field and the cloud shade live in the shared fragments (noise_common.wgsl,
// shadow_common.wgsl) — the terrain and the statics standing on it must read ONE
// implementation or their looks drift apart.

// One layer's detail tap. The taps sit in NON-uniform control flow (a layer with no weight
// here is not sampled — two taps on a typical fragment, not four), so the mip level comes
// from explicit gradients of the tile coordinate rather than implicit derivatives.
fn detail_tap(tile_uv: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>, layer: i32) -> vec4<f32> {
    return textureSampleGrad(detail_tiles, detail_sampler, tile_uv, layer, ddx, ddy);
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let geometric_n = normalize(input.normal);
    let uv = input.world_pos.xz / materials.params.xy;
    let eye_dist = length(camera.camera_pos - input.world_pos);

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

    // Teren 2.0: the detail material. One tap per layer that is present; the gradients are
    // taken once, outside the branches. A fragment without the tiles' detail bit reads a
    // flat tile (0.5 lanes) and pays no tap.
    let tile_uv = input.world_pos.xz / GROUND_TILE_PERIOD_M;
    let tile_ddx = dpdx(tile_uv);
    let tile_ddy = dpdy(tile_uv);
    var tiles: array<vec4<f32>, 4>;
    let flat_tile = vec4<f32>(0.5, 0.5, 0.5, 0.5);
    for (var i = 0; i < 4; i = i + 1) {
        tiles[i] = flat_tile;
        if (w[i] > 0.01 && detail_bit(2u)) {
            tiles[i] = detail_tap(tile_uv, tile_ddx, tile_ddy, i);
        }
    }
    // The height blend: at a splat border the taller material shows through. `w` keeps the
    // splat's own shares for everything that is about WHAT the ground is (vegetation share,
    // meadow shade); `wb` is how the materials MIX where they meet.
    var wb = w;
    for (var i = 0; i < 4; i = i + 1) {
        wb[i] = w[i] * pow(tiles[i].a + HEIGHT_BLEND_FLOOR, HEIGHT_BLEND_POWER);
    }
    wb = wb / max(wb.r + wb.g + wb.b + wb.a, 1.0e-5);
    let detail = tiles[0] * wb.r + tiles[1] * wb.g + tiles[2] * wb.b + tiles[3] * wb.a;

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

    // The material's shade, amplitude blended per layer (the layer's authored `detail`), and
    // the strata noise on steep ground in place of it — a flat fragment pays nothing for it.
    let amp = dot(wb, vec4<f32>(
        materials.layers[0].w,
        materials.layers[1].w,
        materials.layers[2].w,
        materials.layers[3].w,
    ));
    let steep = clamp(1.0 - base_n.y, 0.0, 1.0);
    var shade = detail.b;
    if (steep > 0.02) {
        let strata = value_noise(vec2<f32>(
            input.world_pos.y * 2.2,
            (input.world_pos.x + input.world_pos.z) * 0.15,
        ));
        shade = mix(shade, strata, steep * 0.7);
    }
    let detail_factor = 1.0 + (shade - 0.5) * DETAIL_SHADE_AMP * amp;

    // The tile's tangent normal bent into the lighting normal (the grain catches light). The
    // tile's xz slopes map straight onto world x/z — the bake is world-aligned and isotropic
    // — and the reduced tier (time_params.w, F2) keeps the shade and folds only this relief.
    var bend = vec3<f32>(0.0);
    if (detail_bit(1u)) {
        let tangent = detail.rg * 2.0 - vec2<f32>(1.0, 1.0);
        bend = vec3<f32>(tangent.x, 0.0, tangent.y) * DETAIL_BEND * amp
            * clamp(1.0 - gloss, 0.35, 1.0);
    }
    // No furrow sine here any more (T6): a 1.25 m stripe field on the vegetation share was
    // ploughing every meadow, and its per-vertex direction kinked at every 5 m triangle.
    let n = normalize(base_n + bend);

    // T3: the macro tone — two taps of one tile, the second rotated and 3.83x larger, so the
    // 160 m tile never shows its repeat inside a map; colour variation in the 15–120 m band
    // that the splat (1 m, four flat layers) and the field quilt (50–100 m plots) leave out.
    let macro_near = textureSample(macro_tile, detail_sampler,
        input.world_pos.xz / GROUND_MACRO_PERIOD_M).rgb;
    let macro_far = textureSample(macro_tile, detail_sampler,
        octave_frame_fine(input.world_pos.xz) / (GROUND_MACRO_PERIOD_M * GROUND_MACRO_FAR_RATIO)).rgb;
    let macro_tone = vec3<f32>(1.0)
        + ((macro_near - 0.5) * 0.6 + (macro_far - 0.5) * 0.4) * 2.0 * MACRO_TONE_AMP;

    var albedo = materials.layers[0].rgb * wb.r
        + materials.layers[1].rgb * wb.g
        + materials.layers[2].rgb * wb.b
        + materials.layers[3].rgb * wb.a;
    albedo = albedo * detail_factor * field_light * macro_tone;
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
