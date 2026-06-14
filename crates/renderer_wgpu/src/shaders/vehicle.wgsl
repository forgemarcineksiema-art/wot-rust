// PBR-lite vehicle shader. Separate from scene.wgsl: it consumes the richer VehicleVertex
// (tangent frame, uv, material id, tint mask) so vehicles get normal-mapped micro-detail,
// per-material albedo/roughness specular, and sun + sky-fill lighting, while terrain and simple
// meshes stay on the lighter scene pipeline.

struct Camera {
    view_proj: mat4x4<f32>,
    tint_r: f32,
    tint_g: f32,
    tint_b: f32,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

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
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    out.clip = camera.view_proj * model * vec4<f32>(input.position, 1.0);
    out.world_normal = (model * vec4<f32>(input.normal, 0.0)).xyz;
    out.world_tangent = (model * vec4<f32>(input.tangent.xyz, 0.0)).xyz;
    out.tangent_w = input.tangent.w;
    out.uv = input.uv;
    out.material_id = input.material_id;
    out.tint_mask = input.tint_mask;
    out.team_tint = input.tint.rgb;
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

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(input.world_normal);
    let t = normalize(input.world_tangent - n * dot(n, input.world_tangent));
    let b = cross(n, t) * input.tangent_w;
    let dn = normalize(textureSample(normal_map, vehicle_sampler, input.uv).xyz * 2.0 - vec3<f32>(1.0));
    let world_n = normalize(t * dn.x + b * dn.y + n * dn.z);

    let mat = material_params(input.material_id);
    // Armour takes the per-instance team tint; detail materials keep their absolute albedo.
    let tinted = mix(vec3<f32>(1.0, 1.0, 1.0), input.team_tint, input.tint_mask);
    let baked_albedo = textureSample(albedo_map, vehicle_sampler, input.uv).rgb;
    let albedo = mat.albedo * baked_albedo * tinted;
    let ao_rough = textureSample(ao_roughness_map, vehicle_sampler, input.uv).rgb;
    let ao = ao_rough.r;
    let cavity = textureSample(cavity_map, vehicle_sampler, input.uv).r;

    let sun_dir = normalize(vec3<f32>(0.45, 0.82, 0.35));
    let diffuse = max(dot(world_n, sun_dir), 0.0);
    let sky_fill = 0.25 * (0.5 + 0.5 * world_n.y);
    let lit = albedo * (0.22 + diffuse * 0.85 + sky_fill) * ao * cavity;

    // Roughness-driven specular: smoother materials get a tighter, brighter highlight. The view
    // direction is approximated (the lite model carries no camera position), enough to separate a
    // glossy barrel from matte rubber.
    let half_v = normalize(sun_dir + normalize(vec3<f32>(0.0, 0.4, 1.0)));
    let roughness = clamp(mat.roughness * (0.55 + ao_rough.g), 0.04, 1.0);
    let shininess = mix(4.0, 96.0, 1.0 - roughness);
    let spec = pow(max(dot(world_n, half_v), 0.0), shininess) * (1.0 - roughness) * 0.4;

    let scene_tint = vec3<f32>(camera.tint_r, camera.tint_g, camera.tint_b);
    return vec4<f32>((lit + vec3<f32>(spec, spec, spec)) * scene_tint, 1.0);
}
