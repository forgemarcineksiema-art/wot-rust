struct Camera {
    view_proj: mat4x4<f32>,
    tint_r: f32,
    tint_g: f32,
    tint_b: f32,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

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
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    out.clip = camera.view_proj * model * vec4<f32>(input.position, 1.0);
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
    let sun = normalize(vec3<f32>(0.45, 0.82, 0.35));
    let diffuse = max(dot(n, sun), 0.0);
    // Soft sky fill from above keeps shadowed faces readable rather than black.
    let sky_fill = 0.25 * (0.5 + 0.5 * n.y);
    let shade = 0.30 + diffuse * 0.78 + sky_fill;
    let tint = vec3<f32>(camera.tint_r, camera.tint_g, camera.tint_b);
    return vec4<f32>(input.color * shade * tint, 1.0);
}
