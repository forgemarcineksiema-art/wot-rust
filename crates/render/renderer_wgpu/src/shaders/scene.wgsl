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
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    var world = model * vec4<f32>(input.position, 1.0);
    // The wind lane (D4, lit by the grass field): vertices that opted in — blade tips, leaf
    // edges — ride the field's wind. Two drifting world-space waves, so neighbouring blades
    // gust TOGETHER instead of jittering independently; roots carry sway 0 and stay planted.
    if (input.sway > 0.0) {
        let t = camera.time_params.x;
        let gust = sin(dot(world.xz, vec2<f32>(0.31, 0.17)) + t * 1.6)
            + 0.45 * sin(dot(world.xz, vec2<f32>(0.83, -0.51)) + t * 2.7);
        world = vec4<f32>(
            world.x + gust * input.sway * 0.24,
            world.y - abs(gust) * input.sway * 0.05,
            world.z + gust * input.sway * 0.15,
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
    return out;
}

// --- Procedural material detail ------------------------------------------------------------
// The world's albedo used to be one flat vertex colour per surface; these functions break that
// fill with world-space value noise (stable — anchored to world coordinates, so nothing swims)
// and give steep faces a horizontal strata pattern so cliffs and cut banks read as rock beds,
// not smooth paint. Purely multiplicative around 1.0: palettes and lighting stay authored.

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
    let ground = value_noise(world.xz * 0.4) * 0.6 + value_noise(world.xz * 1.7) * 0.4;
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

// Cloud shade wandering the field: the terrain's sun is modulated by a 2-octave slice of the
// same value noise the sky's cloud sheet drifts with — matched scale (a ~400 m virtual cloud
// height maps the dome's UV onto world metres) and the same clock, so the ground shade moves
// with the banks overhead. Coherent in motion and scale, not pixel-exact (the dome is a ray
// projection, this is world-XZ) — right for a 2D sheet at infinity. Strength (sky_params.x) is
// profile data gated per tier; 0 skips it. Terrain only — a tank is too small for cloud shade
// to read as anything but a dirty hull.
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

// The albedo noise's analytic gradient bent into the normal, so the grain CATCHES LIGHT
// instead of only darkening the paint. Glossier surfaces perturb less — polish is smooth.
fn detail_normal(world: vec3<f32>, n: vec3<f32>, gloss: f32) -> vec3<f32> {
    // Interiors: machined and painted surfaces stay true to their authored normal. The
    // reduced tier (time_params.w, F2) also keeps the authored normal — the three-sample
    // gradient is the priciest part of the material grain for the least visible return.
    if (camera.fog_params.x <= 0.0 || camera.time_params.w < 0.5) {
        return n;
    }
    let e = 0.35;
    let here = value_noise(world.xz * 1.7);
    let dx = value_noise((world.xz + vec2<f32>(e, 0.0)) * 1.7) - here;
    let dz = value_noise((world.xz + vec2<f32>(0.0, e)) * 1.7) - here;
    let bend = vec3<f32>(-dx, 0.0, -dz) * (0.12 / e) * clamp(1.0 - gloss, 0.35, 1.0);
    return normalize(n + bend);
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let geometric_n = normalize(input.normal);
    // Wetness (camera.time_params.z): rain darkens every material, sharpens its finish, and
    // pools mirror-flat sheen on level ground. Presentation only — set by the weather look.
    let wet = clamp(camera.time_params.z, 0.0, 1.0);
    // Rain sheen lives in the puddles, and puddles are PATCHES pooled in the noise's hollows
    // — not a sheet mirror over every flat metre (the old broad gloss painted the whole
    // ground with the overcast sky and read as a mint wash). Soaked ground between them is
    // simply dark.
    let pool = smoothstep(0.58, 0.82, value_noise(input.world_pos.xz * 0.16));
    let puddle = smoothstep(0.985, 0.999, geometric_n.y) * wet * pool * 0.5;
    let gloss = clamp(input.gloss + wet * 0.08 + puddle, 0.0, 1.0);

    let n = detail_normal(input.world_pos, geometric_n, gloss);
    // Cloud shade rides the same channel as the cast shadow: it occludes the key (and the key's
    // specular below) without touching the ambient/fill.
    let shadow = sun_shadow(input.world_pos, geometric_n) * cloud_shadow(input.world_pos);
    let ao = screen_ao(input.clip);
    // A named surface wears its own treatment; everything else keeps the generic detail.
    var detail = material_detail(input.world_pos, geometric_n);
    if (input.surface > 0.5) {
        detail = surface_treatment(input.surface, input.world_pos, geometric_n);
    }
    var albedo = input.color * detail;
    albedo *= mix(1.0, 0.62, wet);

    // Screen AO rides inside light_radiance on the indirect terms only — a sunlit crease keeps
    // its full key while its ambient/fill correctly dampens.
    var lit = albedo * light_radiance(input.world_pos, n, shadow, ao);
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
