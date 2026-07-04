// Depth-only occluder pass for the focused sun shadow map. Renders vehicle occluders from the sun's
// point of view into a depth texture; the scene and vehicle shaders sample it to shade the key
// light. Only the position and the per-instance model matrix are read — no colour, no fragment.

struct Camera {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    ambient_rgb: vec3<f32>,
    ground_ambient_rgb: vec3<f32>,
    key_direction: vec3<f32>,
    key_rgb: vec3<f32>,
    fill_direction: vec3<f32>,
    fill_rgb: vec3<f32>,
    rim_direction: vec3<f32>,
    rim_rgb: vec3<f32>,
    light_view_proj: mat4x4<f32>,
    shadow_params: vec4<f32>,
    ssao_params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(6) model_0: vec4<f32>,
    @location(7) model_1: vec4<f32>,
    @location(8) model_2: vec4<f32>,
    @location(9) model_3: vec4<f32>,
};

@vertex
fn vs_main(input: VsIn) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    return camera.light_view_proj * model * vec4<f32>(input.position, 1.0);
}

// Camera depth prepass for SSAO: same inputs, but transformed by the CAMERA view-projection into
// the screen-sized depth texture the SSAO pass reads.
@vertex
fn vs_prepass(input: VsIn) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    return camera.view_proj * model * vec4<f32>(input.position, 1.0);
}
