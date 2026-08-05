// Lit scene pass (terrain, buildings, props, simple meshes). Composed after camera_common.wgsl,
// lighting_common.wgsl and shadow_common.wgsl — the camera uniform, the lighting model, the
// display transform and the shadow/SSAO lookups all live there, shared with the vehicle pass.

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) tint_weight: f32,
    @location(9) gloss: f32,
    @location(10) surface: f32,
    @location(11) sway: f32,
    // The UV lane (Imported Flora 2.0, FL-1): rides mechanically at [0, 0] for all
    // procedural content; the textured foliage fragment path opts in with FL-2.
    @location(12) uv: vec2<f32>,
    // The bounce lane (Hala 2.0, T1): baked indirect radiance premultiplied by the vertex's
    // albedo, plus any emissive output. Zeros everywhere except GI-baked interiors.
    @location(13) bounce: vec3<f32>,
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
    @location(4) surface: f32,
    @location(5) uv: vec2<f32>,
    @location(6) bounce: vec3<f32>,
};

// The foliage atlas (Imported Flora 2.0, FL-2): every scene fragment samples it at the UV
// lane. Procedural content carries uv (0, 0), where the atlas is opaque white — multiply by
// 1.0 and alpha 1.0, a bit-exact no-op — while imported foliage texels colour the leaf and
// cut its silhouette.
@group(1) @binding(0) var foliage_atlas: texture_2d<f32>;
@group(1) @binding(1) var foliage_sampler: sampler;

// The costume hand-off (Jedna Trawa P4): near tufts fold down EXACTLY where far tufts
// stand up, per place. The radius is not a circle but a coastline — world-anchored noise
// undulates it ±half the spread over ~14 m features, so the eye never finds a ring line.
// ONE function serves both costumes: the near ring takes `stand`, the far meadow takes its
// complement, and their sum is 1 by construction. The band tops out at 47 m, safely inside
// the 48 m shader-ring contract the CPU cache's anti-streaming lock is written against.
const GRASS_HANDOFF_BASE_M: f32 = 33.0;
const GRASS_HANDOFF_SPREAD_M: f32 = 11.0;
const GRASS_HANDOFF_HALF_BAND_M: f32 = 3.0;

// The magnification and the far collapse band live in `meadow_common.wgsl`, which the
// TERRAIN pass composes too — the ground has to know exactly where these tufts fold away in
// order to take over their share of the meadow (costume C).
//
// The near↔far hand-off does NOT stretch with the scope: the near ring's population is a
// CPU cache of a fixed world radius, and its instance count grows with the square of any
// stretch. What the scope buys is DEPTH — the far meadow's collapse moves out (below), so
// the magnified view keeps a meadow instead of looking at bare ground.
fn grass_handoff_stand(anchor: vec2<f32>, eye: vec2<f32>) -> f32 {
    let d = length(eye - anchor);
    let r = GRASS_HANDOFF_BASE_M + value_noise(anchor / 7.0) * GRASS_HANDOFF_SPREAD_M;
    return 1.0 - smoothstep(r - GRASS_HANDOFF_HALF_BAND_M, r + GRASS_HANDOFF_HALF_BAND_M, d);
}

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    var world = model * vec4<f32>(input.position, 1.0);
    let root = model[3].xyz;
    let model_scale = length(model[1].xyz);
    // A far tuft (costume B) STANDS only in its band: it grows in exactly where the near
    // ring folds (the shared hand-off above; per-vertex anchor differs from the tuft's root
    // by at most its half-width, far inside the band) and folds back into the ground before
    // its chunk culls (260-330 m) — both ends read as the meadow thinning, never as a pop.
    // The sway lane doubles as the vertex's height over its root (sway = height * 0.3).
    var stand = 1.0;
    if (abs(input.surface - 5.0) < 0.5) {
        let d = length(camera.camera_pos.xz - world.xz);
        stand = (1.0 - grass_handoff_stand(world.xz, camera.camera_pos.xz))
            * meadow_far_stand(d);
        world.y -= (input.sway / 0.3) * (1.0 - stand);
    }
    // Near blades are a stable, deterministic population. Only the OUTER presentation edge
    // depends on the camera: fold every vertex (including roots) into the instance root at
    // the shared hand-off coastline. There is no camera-driven birth/reveal inside the
    // playable ring, so driving cannot make individual tufts load in around the tank.
    if (abs(input.surface - 6.0) < 0.5) {
        stand = grass_handoff_stand(root.xz, camera.camera_pos.xz);
        world = vec4<f32>(root + (world.xyz - root) * stand, world.w);
    }
    // The wind lane (D4, lit by the grass field): vertices that opted in — blade tips, leaf
    // edges, card tops — ride the field's wind. Two drifting world-space waves, so
    // neighbouring blades gust TOGETHER instead of jittering independently; roots carry
    // sway 0 and stay planted. A collapsed card stops feeling the wind with its stand.
    if (input.sway > 0.0) {
        let t = camera.time_params.x;
        let gust = (sin(dot(world.xz, vec2<f32>(0.31, 0.17)) + t * 1.6)
            + 0.45 * sin(dot(world.xz, vec2<f32>(0.83, -0.51)) + t * 2.7));
        // `sway` is mesh-local. Convert it to world metres with the instance scale, then
        // suppress it as the card/tuft folds down. Without this conversion a tiny faded tuft
        // could still move by the full-size displacement and flash as a long needle.
        let scaled_sway = input.sway * model_scale * stand;
        world = vec4<f32>(
            world.x + gust * scaled_sway * 0.24,
            world.y - abs(gust) * scaled_sway * 0.05,
            world.z + gust * scaled_sway * 0.15,
            world.w,
        );
    }
    out.clip = camera.view_proj * world;
    out.world_pos = world.xyz;
    out.normal = (model * vec4<f32>(input.normal, 0.0)).xyz;
    // Team colour is a per-instance tint, applied only where the vertex opted in (armour);
    // detail materials (barrel, tracks, rubber) carry tint_weight 0 and keep their base colour.
    let tint = mix(vec3<f32>(1.0, 1.0, 1.0), input.tint.rgb, input.tint_weight);
    out.color = input.color * tint;
    out.gloss = input.gloss;
    out.surface = input.surface;
    out.uv = input.uv;
    // Baked light is light, not paint: the team tint never touches it.
    out.bounce = input.bounce;
    return out;
}

// --- Procedural material detail ------------------------------------------------------------
// The world's albedo used to be one flat vertex colour per surface; these functions break that
// fill with world-space value noise (stable — anchored to world coordinates, so nothing swims)
// and give steep faces a horizontal strata pattern so cliffs and cut banks read as rock beds,
// not smooth paint. Purely multiplicative around 1.0: palettes and lighting stay authored.
// The hash, the lattice and the ground octaves live in noise_common.wgsl — the terrain pass
// reads the SAME functions, so the ground and the statics on it carry one grain.

fn material_detail(world: vec3<f32>, n: vec3<f32>) -> f32 {
    // Interior looks (fog density 0 — the hangar; see `fog_params` docs) keep a near-flat
    // finish: painted panels and cast concrete carry no strata, and the outdoor patch noise
    // reads as damp stains smeared down a wall indoors. One gentle octave of paint-sheen
    // variation that also climbs the walls (the y term keeps a vertical panel from sampling
    // one constant noise row).
    if (camera.fog_params.x <= 0.0) {
        let p = world.xz * 1.3 + vec2<f32>(world.y * 0.7, world.y * 0.4);
        return 0.955 + value_noise(p) * 0.07;
    }
    // Two octaves (~2.5 m patches with ~0.6 m grain) on level ground...
    let ground = ground_grain(world.xz).x;
    // ...crossfaded into height-banded strata on steep faces (walls, cliffs, cut banks).
    let strata = value_noise(vec2<f32>(world.y * 2.2, (world.x + world.z) * 0.15));
    let steep = clamp(1.0 - n.y, 0.0, 1.0);
    let detail = mix(ground, strata, steep * 0.7);
    return 0.92 + detail * 0.16;
}

// --- Surface-role treatments (Materia Swiata 3) ---------------------------------------------
// The `surface` lane names WHICH procedural material a vertex wears (see renderer_api's
// surface_role table). Everything is world-anchored arithmetic on the wall's own plane
// coordinates - no UVs, no textures, nothing swims. Each treatment is multiplicative around
// 1.0 so the authored palette and the lighting stay in charge.

fn surface_treatment(role: f32, world: vec3<f32>, n: vec3<f32>) -> f32 {
    // Grass roles are geometry categories, not woody materials. One cheap world-space octave
    // keeps the meadow coherent without paying the generic three-octave ground treatment on
    // tens of thousands of card fragments. The old catch-all painted bark striations across
    // every card and blade and made the field read noisy at driving distance.
    if (role > 4.5) {
        return 0.94 + value_noise(octave_frame_fine(world.xz) * 1.7) * 0.12;
    }
    // The wall-plane frame: h runs along the face, world.y climbs it.
    let tangent = normalize(vec3<f32>(-n.z, 1.0e-4, n.x));
    let h = dot(world, tangent);
    if (role < 1.5) {
        // Plaster: fine grain over half-metre trowel blotches.
        let grain = value_noise(vec2<f32>(h, world.y) * 7.0);
        let blotch = value_noise(vec2<f32>(h * 1.4, world.y * 1.1));
        return 0.90 + grain * 0.08 + blotch * 0.08;
    }
    if (role < 2.5) {
        // Planks: ~0.2 m boards, each with its own tone, split by dark joints, streaked
        // with vertical grain.
        let board = floor(h / 0.2);
        let tone = detail_hash(vec2<f32>(board, board * 1.7)) * 0.18;
        let f = fract(h / 0.2);
        let edge_m = min(f, 1.0 - f) * 0.2;
        let joint = smoothstep(0.0, 0.014, edge_m);
        let grain = value_noise(vec2<f32>(h * 40.0, world.y * 1.3)) * 0.08;
        return (0.86 + tone + grain) * (0.72 + 0.28 * joint);
    }
    if (role < 3.5) {
        // Roof courses: rows climbing the slope, joints staggered every other row, one tone
        // per tile.
        let row = floor(world.y / 0.13);
        let offset = fract(row * 0.5) * 0.26;
        let col = floor((h + offset) / 0.26);
        let tone = detail_hash(vec2<f32>(col * 2.3 + 5.0, row)) * 0.16;
        let fy = fract(world.y / 0.13);
        let row_edge = min(fy, 1.0 - fy) * 0.13;
        let fx = fract((h + offset) / 0.26);
        let col_edge = min(fx, 1.0 - fx) * 0.26;
        let joint = smoothstep(0.0, 0.010, row_edge) * (0.85 + 0.15 * smoothstep(0.0, 0.012, col_edge));
        return (0.88 + tone) * (0.70 + 0.30 * joint);
    }
    // Bark: vertical striations with deeper grooves riding them.
    let striae = value_noise(vec2<f32>(h * 9.0, world.y * 0.7));
    let groove = value_noise(vec2<f32>(h * 18.0, world.y * 2.4));
    return 0.80 + striae * 0.24 + groove * 0.12;
}

// Cloud shade lives in shadow_common.wgsl (the baked coverage texture at group 2) — one
// implementation for the terrain and the generic-ground path, matched to the dome's scale
// and clock. Terrain and statics only — a tank is too small for cloud shade to read as
// anything but a dirty hull.

// The albedo noise's analytic gradient bent into the normal, so the grain CATCHES LIGHT
// instead of only darkening the paint. Glossier surfaces perturb less — polish is smooth.
fn detail_normal(world: vec3<f32>, n: vec3<f32>, gloss: f32) -> vec3<f32> {
    // Interiors: machined and painted surfaces stay true to their authored normal. The
    // reduced tier (time_params.w, F2) also keeps the authored normal — the gradient
    // is the priciest part of the material grain for the least visible return.
    if (camera.fog_params.x <= 0.0 || !detail_bit(4u)) {
        return n;
    }
    let grain = ground_grain(world.xz);
    let bend = vec3<f32>(-grain.y, 0.0, -grain.z) * 0.12 * clamp(1.0 - gloss, 0.35, 1.0);
    return normalize(n + bend);
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    // The foliage cutout (FL-2): sample first so a cut texel skips the whole shade. On the
    // default one-texel white atlas this is a cached fetch, alpha 1.0, colour 1.0 — no-op.
    let foliage = textureSample(foliage_atlas, foliage_sampler, input.uv);
    if (foliage.a < 0.5) {
        discard;
    }
    let geometric_n = normalize(input.normal);
    // Wetness (camera.time_params.z): rain darkens every material, sharpens its finish, and
    // pools mirror-flat sheen on level ground. Presentation only — set by the weather look.
    let wet = clamp(camera.time_params.z, 0.0, 1.0);
    let fill = clamp(camera.weather_params.z, 0.0, 1.0);
    // Rain sheen lives in the puddles, and puddles are PATCHES pooled in the noise's hollows
    // — not a sheet mirror over every flat metre (the old broad gloss painted the whole
    // ground with the overcast sky and read as a mint wash). Soaked ground between them is
    // simply dark.
    let pool = puddle_pool(input.world_pos.xz, fill);
    let puddle = smoothstep(0.94, 0.997, geometric_n.y) * fill * pool * 0.38;
    let gloss = clamp(input.gloss + wet * 0.08 + puddle, 0.0, 1.0);

    let n = detail_normal(input.world_pos, geometric_n, gloss);
    // Cloud shade rides the same channel as the cast shadow: it occludes the key (and the key's
    // specular below) without touching the ambient/fill.
    let shadow =
        sun_shadow(input.world_pos, geometric_n, input.clip) * cloud_shadow(input.world_pos);
    let ao = screen_ao(input.clip);
    // A named surface wears its own treatment; everything else keeps the generic detail.
    var detail = material_detail(input.world_pos, geometric_n);
    if (input.surface > 0.5) {
        detail = surface_treatment(input.surface, input.world_pos, geometric_n);
    }
    var albedo = input.color * detail * foliage.rgb;
    albedo *= mix(1.0, 0.62, wet);

    // Screen AO rides inside light_radiance on the indirect terms only — a sunlit crease keeps
    // its full key while its ambient/fill correctly dampens.
    var lit = albedo * light_radiance(input.world_pos, n, shadow, ao);
    // Baked indirect radiance (Hala 2.0 GI bake): already premultiplied by albedo and already
    // geometrically occluded by the bake's own ray casts, so it joins as a plain add — screen
    // AO on top would double-count the very occlusion the rays measured. Zeros outdoors.
    lit += input.bounce;
    // Specular: a Blinn lobe on the key light plus the analytic-sky reflection, both scaled by
    // the material lane. Matte (gloss 0) surfaces skip this entirely — the historical look.
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
