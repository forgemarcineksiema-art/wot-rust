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
    // Cloud layer from the lighting profile: x = coverage bias, y = pattern scale, z = opacity,
    // w = drift speed (UV per presentation second).
    cloud_params: vec4<f32>,
    // Sky/air extras: x = terrain cloud-shadow strength (0 disables), y = sun-directional
    // scatter in the aerial perspective, zw reserved.
    sky_params: vec4<f32>,
    // Local fill pools (worklamps, pane glow — see renderer_api::LocalLight): xyz = world
    // position, w = radius (0 disables the slot). All-off on every outdoor profile.
    light_pos_radius: array<vec4<f32>, 6>,
    // Pool colours: xyz = linear rgb, w = intensity multiplier.
    light_rgb_intensity: array<vec4<f32>, 6>,
    // Two-layer air: x = valley haze density, y = valley fade-out height (m, 0 disables),
    // z = crepuscular-ray strength (post pass), w reserved.
    haze_params: vec4<f32>,
    // Cloud layer 2: x = high-sheet opacity, y = high-sheet scale, z = storm-front heading
    // (radians), w = storm-front strength (0 disables).
    cloud2_params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct ArmorDamageHeader {
    start: u32,
    count: u32,
    padding_0: u32,
    padding_1: u32,
};

struct ArmorAperture {
    center_major: vec4<f32>,
    normal_minor: vec4<f32>,
    tangent_rotation: vec4<f32>,
    // x irregularity, yz deterministic phases, w plane half-depth.
    shape: vec4<f32>,
    // x glow intensity now (CPU-cooled), y glow tightness, zw reserved.
    thermal: vec4<f32>,
};

@group(0) @binding(1)
var<storage, read> armor_damage_headers: array<ArmorDamageHeader>;
@group(0) @binding(2)
var<storage, read> armor_apertures: array<ArmorAperture>;

/// True only for fragments on the pierced plate and inside its deterministic irregular contour.
/// Shared by color, camera-depth and shadow passes so no pass silently closes the opening.
fn armor_fragment_is_cut(world_pos: vec3<f32>, damage_index: u32) -> bool {
    if (damage_index == 0u) {
        return false;
    }
    let header = armor_damage_headers[damage_index];
    var slot = 0u;
    loop {
        if (slot >= header.count) {
            break;
        }
        let aperture = armor_apertures[header.start + slot];
        let normal = normalize(aperture.normal_minor.xyz);
        let tangent = normalize(aperture.tangent_rotation.xyz);
        let bitangent = normalize(cross(normal, tangent));
        let delta = world_pos - aperture.center_major.xyz;
        if (abs(dot(delta, normal)) <= aperture.shape.w) {
            let local = vec2<f32>(dot(delta, tangent), dot(delta, bitangent));
            let angle = atan2(local.y, local.x);
            let rough = 1.0 + aperture.shape.x
                * (sin(angle * 3.0 + aperture.shape.y) * 0.62
                    + sin(angle * 5.0 + aperture.shape.z) * 0.38);
            let rotation = aperture.tangent_rotation.w;
            let sin_r = sin(rotation);
            let cos_r = cos(rotation);
            let rotated = vec2<f32>(
                local.x * cos_r + local.y * sin_r,
                -local.x * sin_r + local.y * cos_r,
            );
            let metric = vec2<f32>(
                rotated.x / max(aperture.center_major.w * rough, 0.005),
                rotated.y / max(aperture.normal_minor.w * rough, 0.005),
            );
            if (dot(metric, metric) <= 1.0) {
                return true;
            }
        }
        slot += 1u;
    }
    return false;
}
