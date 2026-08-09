// PBR-lite vehicle shader. Separate from scene.wgsl: it consumes the richer VehicleVertex
// (tangent frame, uv, material id, tint mask) so vehicles get normal-mapped micro-detail,
// per-material albedo/roughness specular, and sun + sky-fill lighting, while terrain and simple
// meshes stay on the lighter scene pipeline. Composed after camera_common.wgsl,
// lighting_common.wgsl and shadow_common.wgsl — the camera uniform, the lighting model, the
// display transform and the shadow/SSAO lookups all live there, shared with the scene pass.

// Material maps are role-aware texture arrays: one layer per material_id (rolled armour, cast
// armour, barrel steel, track metal, rubber). The shader selects the layer by the vertex material id.
@group(1) @binding(0)
var albedo_map: texture_2d_array<f32>;
@group(1) @binding(1)
var normal_map: texture_2d_array<f32>;
@group(1) @binding(2)
var ao_roughness_map: texture_2d_array<f32>;
@group(1) @binding(3)
var cavity_map: texture_2d_array<f32>;
@group(1) @binding(4)
var vehicle_sampler: sampler;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) material_id: u32,
    @location(5) tint_mask: f32,
    @location(6) model_0: vec4<f32>,
    @location(7) model_1: vec4<f32>,
    @location(8) model_2: vec4<f32>,
    @location(9) model_3: vec4<f32>,
    @location(10) tint: vec4<f32>,
    @location(11) mapping_mode: u32,
    // Baked per-vertex contact occlusion (geometry-bake surface_shade): 1.0 open, lower in seams.
    @location(12) shade: f32,
    @location(13) damage_index: u32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_tangent: vec3<f32>,
    @location(2) tangent_w: f32,
    @location(3) uv: vec2<f32>,
    @location(4) @interpolate(flat) material_id: u32,
    @location(5) tint_mask: f32,
    @location(6) team_tint: vec3<f32>,
    @location(7) world_pos: vec3<f32>,
    // Object-local position/normal: triplanar projects material coordinates from these, so the
    // texture stays anchored to the part as the hull rotates, the turret traverses and the gun
    // elevates (the model transform never reaches the material coordinates).
    @location(8) local_pos: vec3<f32>,
    @location(9) local_normal: vec3<f32>,
    @location(10) @interpolate(flat) mapping_mode: u32,
    @location(11) shade: f32,
    @location(12) @interpolate(flat) damage_index: u32,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    let world_position = model * vec4<f32>(input.position, 1.0);
    out.clip = camera.view_proj * world_position;
    out.world_pos = world_position.xyz;
    out.world_normal = (model * vec4<f32>(input.normal, 0.0)).xyz;
    out.world_tangent = (model * vec4<f32>(input.tangent.xyz, 0.0)).xyz;
    out.tangent_w = input.tangent.w;
    out.uv = input.uv;
    out.material_id = input.material_id;
    out.tint_mask = input.tint_mask;
    out.team_tint = input.tint.rgb;
    out.local_pos = input.position;
    out.local_normal = input.normal;
    out.mapping_mode = input.mapping_mode;
    out.shade = input.shade;
    out.damage_index = input.damage_index;
    return out;
}

// Per-material PBR-lite parameters: base albedo and roughness, keyed by material id
// (0 rolled armour, 1 cast armour, 2 barrel steel, 3 track metal, 4 rubber,
// 5 interior primer, 6 interior machinery, 7 ammunition, 8 exposed armor section).
struct Material {
    albedo: vec3<f32>,
    roughness: f32,
};

fn material_params(id: u32) -> Material {
    var m: Material;
    if (id == 0u) {
        m.albedo = vec3<f32>(0.42, 0.44, 0.46);
        m.roughness = 0.55;
    } else if (id == 1u) {
        m.albedo = vec3<f32>(0.46, 0.47, 0.48);
        m.roughness = 0.72;
    } else if (id == 2u) {
        m.albedo = vec3<f32>(0.14, 0.15, 0.16);
        // The exterior gun tube is painted service steel, not polished bare metal.
        m.roughness = 0.45;
    } else if (id == 3u) {
        // Worn track steel: a stop lighter than raw plate — link faces, guide horns and pad
        // wear must read as shapes, not merge into one black band under the fenders.
        m.albedo = vec3<f32>(0.17, 0.17, 0.18);
        m.roughness = 0.60;
    } else if (id == 4u) {
        // Roadwheel rubber: dark but not void — the spoke/tire boundary stays visible.
        m.albedo = vec3<f32>(0.10, 0.10, 0.11);
        m.roughness = 0.90;
    } else if (id == 5u) {
        m.albedo = vec3<f32>(0.46, 0.52, 0.39);
        m.roughness = 0.82;
    } else if (id == 6u) {
        m.albedo = vec3<f32>(0.17, 0.17, 0.15);
        m.roughness = 0.82;
    } else if (id == 7u) {
        m.albedo = vec3<f32>(0.43, 0.31, 0.13);
        m.roughness = 0.65;
    } else if (id == 8u) {
        // Torn armor is warmer than a gun tube but never a black void. High roughness keeps the
        // irregular section legible without turning it into a polished copper ring.
        m.albedo = vec3<f32>(0.34, 0.285, 0.22);
        m.roughness = 0.78;
    } else if (id == 9u) {
        // Proofed canvas: the mantlet cover, tarpaulins, the cleaning kit's rolls. Matte, faded
        // duck cloth with no specular lobe worth speaking of. It used to fall through to torn
        // armour, which is exactly the mistake the role exists to prevent: one material for two
        // things is one of them rendered wrong.
        m.albedo = vec3<f32>(0.40, 0.37, 0.29);
        m.roughness = 0.95;
    } else if (id == 10u) {
        // Glass: the headlight lens, vision-block prisms. The one thing on a tank that is meant
        // to catch the sun, so it is the only role here with a genuinely tight lobe.
        m.albedo = vec3<f32>(0.55, 0.60, 0.62);
        m.roughness = 0.08;
    } else if (id == 11u) {
        // Seasoned timber: the unditching log. Warm, fibrous, unpainted — nothing like steel.
        m.albedo = vec3<f32>(0.33, 0.25, 0.16);
        m.roughness = 0.88;
    } else {
        // Unknown role. Reached only if `material_role_id` grows past what this function answers,
        // which `vehicle_material_ids.rs` makes a build failure — so this is a visible flag, not a
        // silent stand-in. Magenta is the point.
        m.albedo = vec3<f32>(1.0, 0.0, 1.0);
        m.roughness = 0.5;
    }
    return m;
}

/// Whether a world point on the plate plane lies inside the aperture — the sun-admission test
/// for interior lighting. Shares the ONE contour evaluation (`aperture_contour_metric`) with
/// the mesh cut and the soot/glow marks, so light enters exactly the hole that is drawn and
/// that collision samples.
fn aperture_contains_world(aperture: ArmorAperture, world_pos: vec3<f32>) -> bool {
    let metric = aperture_contour_metric(aperture, aperture_plane_local(aperture, world_pos));
    return dot(metric, metric) <= 1.0;
}

/// Aperture-only illumination for interior materials. Up to four dominant holes contribute a sky
/// solid-angle fill; direct sun is admitted only when its ray intersects the actual contour.
fn armor_interior_radiance(
    world_pos: vec3<f32>,
    surface_normal: vec3<f32>,
    damage_index: u32,
) -> vec3<f32> {
    if (damage_index == 0u) {
        return vec3<f32>(0.0);
    }
    let header = armor_damage_headers[damage_index];
    var radiance = vec3<f32>(0.0);
    var slot = 0u;
    loop {
        if (slot >= min(header.count, 4u)) {
            break;
        }
        let aperture = armor_apertures[header.start + slot];
        let plate_normal = normalize(aperture.normal_minor.xyz);
        let to_hole = aperture.center_major.xyz - world_pos;
        let depth = dot(to_hole, plate_normal);
        if (depth > 0.01) {
            let area = 3.14159265 * aperture.center_major.w * aperture.normal_minor.w;
            let solid_angle = clamp(area / (dot(to_hole, to_hole) + area), 0.0, 0.45);
            // A first diffuse bounce keeps a small opening readable after exposure without adding
            // any light through intact armor. The sqrt term approximates nearby painted surfaces
            // spreading sky energy beyond the aperture's direct projected solid angle.
            let bounced_sky = solid_angle * 3.0 + sqrt(solid_angle) * 1.6;
            radiance += camera.ambient_rgb * bounced_sky;

            let sun = normalize(camera.key_direction);
            let toward_outside = dot(sun, plate_normal);
            if (toward_outside > 0.02) {
                let ray_t = depth / toward_outside;
                let plane_hit = world_pos + sun * ray_t;
                if (ray_t > 0.0 && aperture_contains_world(aperture, plane_hit)) {
                    let diffuse = max(dot(surface_normal, sun), 0.0);
                    radiance += camera.key_rgb * (0.18 + diffuse * 0.82);
                }
            }
        }
        slot += 1u;
    }
    return radiance;
}

// Cheap 3D value noise for material micro-variation, sampled in OBJECT-local space so the
// grain rides the part (hull rotation, turret traverse) instead of swimming through it.
fn v_hash(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3<f32>(127.1, 311.7, 74.7))) * 43758.5453);
}

fn v_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let n000 = v_hash(i);
    let n100 = v_hash(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = v_hash(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = v_hash(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = v_hash(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = v_hash(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = v_hash(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = v_hash(i + vec3<f32>(1.0, 1.0, 1.0));
    let nx00 = mix(n000, n100, u.x);
    let nx10 = mix(n010, n110, u.x);
    let nx01 = mix(n001, n101, u.x);
    let nx11 = mix(n011, n111, u.x);
    return mix(mix(nx00, nx10, u.y), mix(nx01, nx11, u.y), u.z);
}

// Object-local texels-per-metre for triplanar projection. Matches the client's parametric UV_SCALE
// so the two mapping modes read at a consistent material density across one vehicle.
const TRIPLANAR_SCALE: f32 = 0.5;

// Blend weights from the (object-local) normal: a small exponent narrows the projection seams
// without producing abrupt material bands.
fn triplanar_weights(n: vec3<f32>) -> vec3<f32> {
    let w = pow(abs(n), vec3<f32>(4.0, 4.0, 4.0));
    let s = w.x + w.y + w.z;
    return w / max(s, 1.0e-5);
}

// Sample a role layer of an array map by its three object-local axis projections, blended by the
// normal weights. `layer` is the material id.
fn triplanar_sample(
    tex: texture_2d_array<f32>,
    layer: i32,
    p: vec3<f32>,
    w: vec3<f32>,
) -> vec4<f32> {
    let sx = textureSample(tex, vehicle_sampler, p.zy * TRIPLANAR_SCALE, layer);
    let sy = textureSample(tex, vehicle_sampler, p.xz * TRIPLANAR_SCALE, layer);
    let sz = textureSample(tex, vehicle_sampler, p.xy * TRIPLANAR_SCALE, layer);
    return sx * w.x + sy * w.y + sz * w.z;
}

// A stable world tangent orthogonal to `n`, used to apply a blended detail normal for triplanar
// surfaces (the sampling coordinates stay object-local, so the material does not swim).
fn stable_tangent(n: vec3<f32>) -> vec3<f32> {
    let seed = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(n.x) < 0.9);
    return normalize(seed - n * dot(seed, n));
}

// Which texture-array layer a material role samples. Twelve roles, eight layers.
//
// This was `min(material_id, 4u)`, which is a clamp and not a mapping: it sent the interior three,
// torn steel, canvas, glass and timber all to the RUBBER layer. `client::material_layer_id` is the
// same table in Rust and `client/tests/vehicle_material_ids.rs` holds the two to each other, so a
// role added on one side without the other is a failing test rather than a part wearing a tyre.
fn material_layer(id: u32) -> i32 {
    if (id == 0u) { return 0; }          // RolledArmor
    else if (id == 1u) { return 1; }     // CastArmor
    else if (id == 2u) { return 2; }     // BarrelSteel
    else if (id == 3u) { return 3; }     // TrackMetal
    else if (id == 4u) { return 4; }     // Rubber
    else if (id == 5u) { return 0; }     // InteriorPrimer -> painted sheet
    else if (id == 6u) { return 2; }     // InteriorMachinery -> machined steel
    else if (id == 7u) { return 2; }     // Ammunition -> machined steel
    else if (id == 8u) { return 0; }     // ExposedSteel -> torn rolled plate
    else if (id == 9u) { return 5; }     // Canvas
    else if (id == 10u) { return 6; }    // Glass
    else if (id == 11u) { return 7; }    // Timber
    return 0;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    if (armor_fragment_is_cut(input.world_pos, input.damage_index)) {
        discard;
    }
    let n = normalize(input.world_normal);
    let layer = material_layer(input.material_id);
    var world_n: vec3<f32>;
    var baked_albedo: vec3<f32>;
    var ao_rough: vec3<f32>;
    var cavity: f32;

    if (input.mapping_mode == 0u) {
        // Parametric: authored UV0 with a tangent-space normal map (as before).
        let t = normalize(input.world_tangent - n * dot(n, input.world_tangent));
        let b = cross(n, t) * input.tangent_w;
        let dn = normalize(textureSample(normal_map, vehicle_sampler, input.uv, layer).xyz * 2.0 - vec3<f32>(1.0));
        world_n = normalize(t * dn.x + b * dn.y + n * dn.z);
        baked_albedo = textureSample(albedo_map, vehicle_sampler, input.uv, layer).rgb;
        ao_rough = textureSample(ao_roughness_map, vehicle_sampler, input.uv, layer).rgb;
        cavity = textureSample(cavity_map, vehicle_sampler, input.uv, layer).r;
    } else {
        // Triplanar: project from object-local coordinates so the material stays anchored to the
        // part under hull rotation, turret traverse and gun elevation.
        let w = triplanar_weights(normalize(input.local_normal));
        baked_albedo = triplanar_sample(albedo_map, layer, input.local_pos, w).rgb;
        ao_rough = triplanar_sample(ao_roughness_map, layer, input.local_pos, w).rgb;
        cavity = triplanar_sample(cavity_map, layer, input.local_pos, w).r;
        let t = stable_tangent(n);
        let b = cross(n, t);
        let dn = normalize(triplanar_sample(normal_map, layer, input.local_pos, w).xyz * 2.0 - vec3<f32>(1.0));
        world_n = normalize(t * dn.x + b * dn.y + n * dn.z);
    }

    let mat = material_params(input.material_id);
    // Armour takes the per-instance team tint; detail materials keep their absolute albedo.
    let tinted = mix(vec3<f32>(1.0, 1.0, 1.0), input.team_tint, input.tint_mask);
    // Wetness (camera.time_params.z): rain-soaked paint and steel darken and tighten their
    // finish, exactly like the terrain does. Presentation only — set by the weather look.
    let wet = clamp(camera.time_params.z, 0.0, 1.0);

    // Material micro-variation: broad patches of repainted/faded finish over a finer service
    // grain. Object-local, so a parked tank and a moving one carry the same wear.
    let grain = v_noise(input.local_pos * 2.6) * 0.65 + v_noise(input.local_pos * 9.0) * 0.35;
    let albedo_var = mix(0.92, 1.08, grain);
    // A burnt-out wreck is charcoal, not lacquer: the wreck tint's near-black luma (well under
    // any live paint — the darkest camo+dirt tint stays above ~0.33) drives the WHOLE vehicle
    // matte and kills the sky mirror, so charring never reads as gloss paint. Not gated by
    // tint_mask: the wreck's running gear chars and sheds its dust with the hull.
    let tint_luma = dot(input.team_tint, vec3<f32>(0.299, 0.587, 0.114));
    let burnt = 1.0 - smoothstep(0.16, 0.30, tint_luma);

    // Service weathering: the running gear (track metal + rubber) wears the field's dry-earth
    // dust, broken up by the broad noise so it reads as use, not paint. Kept off the armour
    // and barrel — their part-local origins sit at the ring/trunnion, so a height band there
    // would dust the turret roof — and burnt steel sheds its dust with everything else.
    let gear = select(
        0.0,
        1.0,
        input.material_id == 3u || input.material_id == 4u,
    );
    let dust = gear
        * (0.45 + 0.55 * v_noise(input.local_pos * 1.7 + vec3<f32>(31.7, 7.3, 19.1)))
        * (1.0 - burnt);
    let dust_tone = vec3<f32>(0.36, 0.32, 0.25);

    var albedo = mat.albedo * baked_albedo * tinted * albedo_var * mix(1.0, 0.85, wet);
    let fractured_steel = input.material_id == 8u;
    let heat_band = 1.0 - smoothstep(0.52, 0.86, input.shade);
    let fracture_tone = mix(vec3<f32>(0.34, 0.29, 0.23), vec3<f32>(0.19, 0.12, 0.075), heat_band);
    albedo = select(albedo, fracture_tone * albedo_var, fractured_steel);
    albedo = mix(albedo, dust_tone * albedo_var, dust * 0.30 * (1.0 - wet * 0.5));
    let ao = ao_rough.r;

    // The sun a hull sees is the sun the GROUND under it sees: the shadow map and the wandering
    // cloud sheet both. Vehicles used to skip the cloud term that terrain.wgsl and scene.wgsl
    // apply, so a bank of shade swept the field and left every tank standing in it fully lit —
    // a bright cut-out pasted on darkened ground.
    let shadow =
        sun_shadow(input.world_pos, world_n, input.clip) * cloud_shadow(input.world_pos);
    // Baked contact occlusion (geometry-bake surface_shade): authored per-vertex, it dampens
    // every term so the turret-ring seam, running-gear recess and grille wells read as real
    // cavities. SCREEN-space AO is different — it rides inside light_radiance on the indirect
    // terms only, so a sunlit hull keeps its full key.
    let contact = clamp(input.shade, 0.0, 1.0);
    let screen = screen_ao(input.clip);

    // THREE ESTIMATES OF ONE QUANTITY, so the most occluded wins instead of all three
    // multiplying. `ao` (material bake), `cavity` (material bake) and `contact` (geometry
    // bake) each answer the same physical question — how enclosed is this point — from a
    // different source. Multiplying them compounded the same occlusion three times over.
    //
    // Measured on `prokhorovka_contact_backlit`, the locked frame for the side the sun never
    // touches: neutralising any ONE of them moved the subject median by +0.003 to +0.008 (and
    // screen AO by +0.017), while neutralising all of them together moved it from 0.002 to
    // 0.030 — fifteenfold. The compounding, not any single term, was what left the hull flank,
    // tracks and road wheels as one black mass with no readable form.
    //
    // `min` rather than an average: a seam that any bake calls enclosed IS enclosed, so the
    // cavities the artists authored keep their full depth. What disappears is only the
    // accidental product of three independent bakes agreeing.
    let surface_occlusion = min(ao, min(cavity, contact));
    // ...and screen AO is a FOURTH estimate of that same quantity, so it joins by `min` too.
    // Multiplying it in on top of the three bakes was the very compounding the paragraph above
    // rejects, surviving one level up: the bakes were reconciled with each other and the product
    // was then taken against SSAO anyway. On a shaded flank both terms sit near 0.7, so the
    // indirect light a hull's dark side lives on was being halved instead of dimmed once.
    let indirect_occlusion = min(surface_occlusion, screen);
    // The direct sun keeps the baked occlusion (a crease IS darker under a raking key, and the
    // seams were authored to read that way); the indirect terms take the reconciled one. The rim
    // stays out of both, which is what lighting_common promises it does — it is a silhouette
    // accent, and the silhouette is the thing this frame exists to keep readable.
    var lit = albedo
        * light_radiance(
            input.world_pos,
            world_n,
            shadow * surface_occlusion,
            indirect_occlusion,
        );
    // The hero probe (Hala 3.0 B2): the garage hall's bounced light, blended by the ambient
    // cube — the vehicle-side twin of the scene mesh's baked bounce lane, closing G5 (the
    // room has had GI since Hala 2.0; the hero standing in it received none). All-zero on
    // battle frames, where this line adds exactly nothing. Occluded like the ambient: cavity
    // AO applies, the sun shadow does not (bounce arrives from everywhere).
    lit += albedo * hero_probe_irradiance(world_n) * indirect_occlusion;
    // INTERIOR roles only (5 primer, 6 machinery, 7 ammunition). Canvas, glass and timber are
    // exterior fittings that happen to sit above them in the id order, and an unbounded `>= 5`
    // lit a tarpaulin as though it were inside the fighting compartment.
    if (input.material_id >= 5u && input.material_id <= 7u) {
        lit += albedo
            * armor_interior_radiance(input.world_pos, world_n, input.damage_index)
            * surface_occlusion;
    }

    // Roughness-driven specular off the key light, evaluated against the real world-space view
    // direction (camera position - fragment position) so the highlight tracks the camera, not a
    // fixed approximation. Smoother materials get a tighter, brighter highlight.
    let view_dir = normalize(camera.camera_pos - input.world_pos);
    let key_dir = normalize(camera.key_direction);
    let half_v = normalize(key_dir + view_dir);
    // Micro-variation and dust roughen the finish; rain tightens it; charring caps it matte.
    let role_roughness = select(mat.roughness, 0.78, fractured_steel);
    let rough_base = role_roughness * (0.55 + ao_rough.g) * mix(1.0, 0.55, wet);
    let roughness = clamp(
        mix(rough_base + (grain - 0.5) * 0.20 + dust * 0.22, 0.95, burnt),
        0.04,
        1.0,
    );
    let shininess = mix(4.0, 96.0, 1.0 - roughness);
    // Amplitude falls with the CUBE of smoothness, not linearly. A rough surface does not just
    // spread its lobe, it dims it — and the linear law left matte service paint with a specular
    // amplitude of 0.168 at roughness 0.58, about four times a dielectric's ~0.04 reflectance.
    // On big smooth cast shapes that read as broad hot sheets of sky and is a large part of what
    // "the light is too strong" actually is. The cube collapses paint to ~0.044 while LIFTING
    // genuinely smooth roles: headlight glass at roughness 0.1 goes 0.36 -> 0.44, so the one
    // thing on a tank meant to catch the sun still catches it.
    let spec = pow(max(dot(world_n, half_v), 0.0), shininess) * pow(1.0 - roughness, 3.0) * 0.6;
    // The specular is the key light's highlight, so it is occluded by the same shadow.
    let spec_color = camera.key_rgb * spec * shadow * contact;

    // Analytic-sky environment reflection: the smoother the finish, the more the hull mirrors
    // the sky along the reflected view ray — strongest at grazing angles (Fresnel), dampened in
    // baked cavities so recesses don't glow. Rain cuts roughness, so a soaked tank reflects.
    let smoothness = 1.0 - roughness;
    // Painted vehicle surfaces are dielectric at normal incidence. Starting Fresnel at 0.25 made
    // a dark barrel mostly mirror the blue sky; the physical ~0.04 F0 keeps the reflection as a
    // readable grazing highlight instead of replacing the material colour.
    let fresnel = 0.04 + 0.96 * pow(1.0 - max(dot(world_n, view_dir), 0.0), 5.0);
    // The sky reflection is indirect light, so it takes the screen AO the key terms skip.
    let interior_env = select(1.0, 0.10, input.material_id >= 5u && input.material_id <= 7u);
    let fracture_env = select(1.0, 0.32, fractured_steel);
    let env = env_sky(reflect(-view_dir, world_n))
        * smoothness * smoothness * fresnel * indirect_occlusion
        * (1.0 - burnt * 0.85) * interior_env * fracture_env;

    // Perforation marks. Scorch is the permanent soot (the whole visible penetration on a
    // legacy hull that is never opened; the burnt lip around the real hole on the T-54) and
    // eats radiance and specular alike. Glow is the thermal rim: energy speaking, cooling away
    // CPU-side — deep orange heat, never a permanent white edge.
    let thermal = armor_aperture_thermal(input.world_pos, input.damage_index);
    let glow_rgb = vec3<f32>(1.0, 0.30, 0.06) * thermal.x * 2.2;
    let scorched = (lit + spec_color + env) * (1.0 - thermal.y * 0.78);

    // Linear HDR out: the display transform lives in the central post pass (rule 7).
    return vec4<f32>(apply_fog(scorched + glow_rgb, input.world_pos), 1.0);
}
