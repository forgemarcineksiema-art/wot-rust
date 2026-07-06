// Gradient-sky background pass. Draws a single fullscreen triangle behind the geometry (depth
// compare Always, no depth write) and shades each pixel from the view ray direction: a zenith->
// horizon gradient plus a soft sun disc/haze along the key light. The horizon colour matches the
// aerial-perspective fog colour (SceneLighting::sky_horizon_rgb) so distant terrain melts into the
// visible sky instead of meeting a flat clear colour. See docs/atmosphere-policy.md phase 2.

struct Camera {
    view_proj: mat4x4<f32>,
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
    shadow_params: vec4<f32>,
    ssao_params: vec4<f32>,
    sky_zenith_rgb: vec3<f32>,
    sky_horizon_rgb: vec3<f32>,
    fog_params: vec4<f32>,
    // x = presentation seconds (tick-domain — see gpu_layout.rs), yzw reserved.
    time_params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    // Clip-space XY of this pixel (NDC), unprojected in the fragment stage into a world ray.
    @location(0) ndc: vec2<f32>,
};

// A single oversized triangle covering the framebuffer: vertices (-1,-1), (-1,3), (3,-1). Placed at
// the far plane (z=1) — depth compare Always means the value is irrelevant, but it keeps the sky
// behind any geometry that later draws with a real depth test.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    var out: VsOut;
    let x = f32(index / 2u) * 4.0 - 1.0;
    let y = f32(index % 2u) * 4.0 - 1.0;
    out.ndc = vec2<f32>(x, y);
    out.clip = vec4<f32>(x, y, 1.0, 1.0);
    return out;
}

// Filmic ACES-lite tone curve (Narkowicz fit); mirrors scene.wgsl/vehicle.wgsl so the sky is graded
// on the same curve as the lit world and the horizon meets the fogged terrain seamlessly.
fn tonemap_aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    // Unproject the far-plane NDC point to world space and take the direction from the camera: the
    // per-pixel view ray. inv_view_proj is singular only for a degenerate camera (all-zeros), which
    // normalize below turns into a harmless constant.
    let world = camera.inv_view_proj * vec4<f32>(input.ndc, 1.0, 1.0);
    let dir = normalize(world.xyz / world.w - camera.camera_pos);

    // Zenith->horizon gradient by the ray's up fraction. The sqrt lifts the horizon band so the
    // paler haze hugs the ground line rather than washing the whole dome.
    let up = clamp(dir.y, 0.0, 1.0);
    var color = mix(camera.sky_horizon_rgb, camera.sky_zenith_rgb, sqrt(up));

    // Sun: a tight bright disc plus a soft surrounding haze, along the key light direction. Only
    // above the horizon so a low sun does not bleed a second glow into the ground band.
    let sun = normalize(camera.key_direction);
    let d = max(dot(dir, sun), 0.0);
    let above = smoothstep(-0.02, 0.06, dir.y);
    let disc = pow(d, 900.0) * 6.0;
    let halo = pow(d, 9.0) * 0.18;
    color += camera.key_rgb * (disc + halo) * above;

    return vec4<f32>(tonemap_aces(color), 1.0);
}
