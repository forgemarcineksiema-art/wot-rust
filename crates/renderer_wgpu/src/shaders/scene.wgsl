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

// Calibrated three-point lighting: ambient plus key/fill/rim directional terms. Directions point
// towards each light and are normalized here; an unlit (black) light contributes nothing.
fn scene_radiance(n: vec3<f32>) -> vec3<f32> {
    let key = max(dot(n, normalize(camera.key_direction)), 0.0);
    let fill = max(dot(n, normalize(camera.fill_direction)), 0.0);
    let rim = max(dot(n, normalize(camera.rim_direction)), 0.0);
    return camera.ambient_rgb
        + camera.key_rgb * key
        + camera.fill_rgb * fill
        + camera.rim_rgb * rim;
}

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
    return vec4<f32>(input.color * scene_radiance(n), 1.0);
}
