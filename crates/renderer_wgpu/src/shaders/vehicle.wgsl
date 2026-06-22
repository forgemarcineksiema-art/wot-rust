// PBR-lite vehicle shader. Separate from scene.wgsl: it consumes the richer VehicleVertex
// (tangent frame, uv, material id, tint mask) so vehicles get normal-mapped micro-detail,
// per-material albedo/roughness specular, and sun + sky-fill lighting, while terrain and simple
// meshes stay on the lighter scene pipeline.

struct Camera {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    ambient_rgb: vec3<f32>,
    key_direction: vec3<f32>,
    key_rgb: vec3<f32>,
    fill_direction: vec3<f32>,
    fill_rgb: vec3<f32>,
    rim_direction: vec3<f32>,
    rim_rgb: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

// Calibrated three-point lighting: ambient plus key/fill/rim directional terms (directions point
// towards each light, normalized here). Shared in spirit with scene.wgsl's scene_radiance.
fn light_radiance(n: vec3<f32>) -> vec3<f32> {
    let key = max(dot(n, normalize(camera.key_direction)), 0.0);
    let fill = max(dot(n, normalize(camera.fill_direction)), 0.0);
    let rim = max(dot(n, normalize(camera.rim_direction)), 0.0);
    return camera.ambient_rgb
        + camera.key_rgb * key
        + camera.fill_rgb * fill
        + camera.rim_rgb * rim;
}

@group(1) @binding(0)
var albedo_map: texture_2d<f32>;
@group(1) @binding(1)
var normal_map: texture_2d<f32>;
@group(1) @binding(2)
var ao_roughness_map: texture_2d<f32>;
@group(1) @binding(3)
var cavity_map: texture_2d<f32>;
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
    return out;
}

// Per-material PBR-lite parameters: base albedo and roughness, keyed by material id
// (0 rolled armour, 1 cast armour, 2 barrel steel, 3 track metal, 4 rubber).
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
        m.roughness = 0.30;
    } else if (id == 3u) {
        m.albedo = vec3<f32>(0.10, 0.10, 0.11);
        m.roughness = 0.60;
    } else {
        m.albedo = vec3<f32>(0.045, 0.045, 0.05);
        m.roughness = 0.90;
    }
    return m;
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

// Sample a map by its three object-local axis projections and blend by the normal weights.
fn triplanar_sample(
    tex: texture_2d<f32>,
    p: vec3<f32>,
    w: vec3<f32>,
) -> vec4<f32> {
    let sx = textureSample(tex, vehicle_sampler, p.zy * TRIPLANAR_SCALE);
    let sy = textureSample(tex, vehicle_sampler, p.xz * TRIPLANAR_SCALE);
    let sz = textureSample(tex, vehicle_sampler, p.xy * TRIPLANAR_SCALE);
    return sx * w.x + sy * w.y + sz * w.z;
}

// A stable world tangent orthogonal to `n`, used to apply a blended detail normal for triplanar
// surfaces (the sampling coordinates stay object-local, so the material does not swim).
fn stable_tangent(n: vec3<f32>) -> vec3<f32> {
    let seed = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(n.x) < 0.9);
    return normalize(seed - n * dot(seed, n));
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(input.world_normal);
    var world_n: vec3<f32>;
    var baked_albedo: vec3<f32>;
    var ao_rough: vec3<f32>;
    var cavity: f32;

    if (input.mapping_mode == 0u) {
        // Parametric: authored UV0 with a tangent-space normal map (as before).
        let t = normalize(input.world_tangent - n * dot(n, input.world_tangent));
        let b = cross(n, t) * input.tangent_w;
        let dn = normalize(textureSample(normal_map, vehicle_sampler, input.uv).xyz * 2.0 - vec3<f32>(1.0));
        world_n = normalize(t * dn.x + b * dn.y + n * dn.z);
        baked_albedo = textureSample(albedo_map, vehicle_sampler, input.uv).rgb;
        ao_rough = textureSample(ao_roughness_map, vehicle_sampler, input.uv).rgb;
        cavity = textureSample(cavity_map, vehicle_sampler, input.uv).r;
    } else {
        // Triplanar: project from object-local coordinates so the material stays anchored to the
        // part under hull rotation, turret traverse and gun elevation.
        let w = triplanar_weights(normalize(input.local_normal));
        baked_albedo = triplanar_sample(albedo_map, input.local_pos, w).rgb;
        ao_rough = triplanar_sample(ao_roughness_map, input.local_pos, w).rgb;
        cavity = triplanar_sample(cavity_map, input.local_pos, w).r;
        let t = stable_tangent(n);
        let b = cross(n, t);
        let dn = normalize(triplanar_sample(normal_map, input.local_pos, w).xyz * 2.0 - vec3<f32>(1.0));
        world_n = normalize(t * dn.x + b * dn.y + n * dn.z);
    }

    let mat = material_params(input.material_id);
    // Armour takes the per-instance team tint; detail materials keep their absolute albedo.
    let tinted = mix(vec3<f32>(1.0, 1.0, 1.0), input.team_tint, input.tint_mask);
    let albedo = mat.albedo * baked_albedo * tinted;
    let ao = ao_rough.r;

    let lit = albedo * light_radiance(world_n) * ao * cavity;

    // Roughness-driven specular off the key light, evaluated against the real world-space view
    // direction (camera position - fragment position) so the highlight tracks the camera, not a
    // fixed approximation. Smoother materials get a tighter, brighter highlight.
    let view_dir = normalize(camera.camera_pos - input.world_pos);
    let key_dir = normalize(camera.key_direction);
    let half_v = normalize(key_dir + view_dir);
    let roughness = clamp(mat.roughness * (0.55 + ao_rough.g), 0.04, 1.0);
    let shininess = mix(4.0, 96.0, 1.0 - roughness);
    let spec = pow(max(dot(world_n, half_v), 0.0), shininess) * (1.0 - roughness) * 0.4;
    let spec_color = camera.key_rgb * spec;

    return vec4<f32>(lit + spec_color, 1.0);
}
