// The one shared Camera uniform declaration, composed (Rust-side — WGSL has no #include) into
// every pass that binds the scene camera buffer at group 0, binding 0. The layout mirrors
// CameraUniform in gpu_layout.rs byte-for-byte and is locked by the wgsl_layout tests; growing
// the uniform means editing exactly this struct and that Rust struct, nothing else.
//
// Passes that read only a few leading fields (shadow depth, SSAO) still declare the full struct:
// the buffer they bind is the full CameraUniform, and one shared declaration beats seven
// hand-maintained prefixes.

struct Camera {
    view_proj: mat4x4<f32>,
    // Inverse view-projection: the sky pass unprojects per-pixel view rays with it.
    inv_view_proj: mat4x4<f32>,
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
    // The far shadow cascade's light view-projection (terrain/static casters past the near box).
    light_view_proj_far: mat4x4<f32>,
    // x = shadow texel UV step, y = depth bias, z = strength (0 disables), w = world normal offset.
    shadow_params: vec4<f32>,
    // Far-cascade controls: x = far texel UV step, y = far world normal offset,
    // z = cascade count (< 2 disables the far lookup), w = near-box containment margin (UV).
    cascade_params: vec4<f32>,
    // x = near plane, y = far plane, z = strength (0 disables), w = projection Y scale (P[1][1]).
    ssao_params: vec4<f32>,
    sky_zenith_rgb: vec3<f32>,
    sky_horizon_rgb: vec3<f32>,
    // x = fog density (0 disables — interior looks), y = height falloff,
    // zw = inverse render-target size (screen-pixel -> UV for reduced-resolution screen targets).
    fog_params: vec4<f32>,
    // x = presentation seconds (tick-domain — see gpu_layout.rs), y = rain intensity,
    // z = world wetness, w reserved.
    time_params: vec4<f32>,
    // Display grade from the lighting profile: x = exposure (pre-curve HDR multiplier),
    // y = black point, z = saturation, w = contrast.
    grade_params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;
