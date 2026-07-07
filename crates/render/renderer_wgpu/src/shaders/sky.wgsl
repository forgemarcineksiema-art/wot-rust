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
    // x = presentation seconds (tick-domain — see gpu_layout.rs), y = rain intensity,
    // z = world wetness, w reserved.
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

// Gentle display grade after the tone curve (mirrors scene.wgsl's display_grade verbatim) so the sky
// grades into the same picture as the lit world and the horizon meets the fogged terrain seamlessly.
fn display_grade(c: vec3<f32>) -> vec3<f32> {
    let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    let saturated = mix(vec3<f32>(luma), c, 1.18);
    let contrasted = (saturated - vec3<f32>(0.5)) * 1.10 + vec3<f32>(0.5);
    return clamp(contrasted, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Filmic ACES-lite tone curve (Narkowicz fit); mirrors scene.wgsl/vehicle.wgsl so the sky is graded
// on the same curve as the lit world.
fn tonemap_aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    let mapped = clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
    return display_grade(mapped);
}

// --- Procedural clouds ----------------------------------------------------------------------
// A drifting FBM sheet on a virtual cloud plane, so the dome stops being a flat two-stop wash. The
// noise is anchored to the ray direction (world-stable — it does not swim as the camera turns) and
// crawls only by the presentation clock, so a still frame is still and a moving one drifts slowly.

fn cloud_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn cloud_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = cloud_hash(i);
    let b = cloud_hash(i + vec2<f32>(1.0, 0.0));
    let c = cloud_hash(i + vec2<f32>(0.0, 1.0));
    let d = cloud_hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn cloud_fbm(p: vec2<f32>) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var freq = 1.0;
    for (var i = 0; i < 5; i = i + 1) {
        sum = sum + amp * cloud_noise(p * freq);
        freq = freq * 2.0;
        amp = amp * 0.5;
    }
    return sum;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    // Unproject the far-plane NDC point to world space and take the direction from the camera: the
    // per-pixel view ray. inv_view_proj is singular only for a degenerate camera (all-zeros), which
    // normalize below turns into a harmless constant.
    let world = camera.inv_view_proj * vec4<f32>(input.ndc, 1.0, 1.0);
    let dir = normalize(world.xyz / world.w - camera.camera_pos);

    // Zenith->horizon gradient by the ray's up fraction. A gentle power keeps the deeper zenith blue
    // reaching down toward the eye line, so the dome is not one flat pale band; the paler haze still
    // hugs the ground line where real aerial haze thickens.
    let up = clamp(dir.y, 0.0, 1.0);
    var color = mix(camera.sky_horizon_rgb, camera.sky_zenith_rgb, pow(up, 0.42));

    let sun = normalize(camera.key_direction);
    let d = max(dot(dir, sun), 0.0);
    let above = smoothstep(-0.02, 0.06, dir.y);

    // Clouds: project the ray onto a plane above the eye (uv foreshortens toward the horizon like
    // real cloud cover) and drift it by the clock. A domain warp — offsetting the sample by a
    // low-frequency noise of itself — breaks the grid alignment of raw value noise into billowed,
    // natural banks instead of angular blobs. Coverage is soft-thresholded so open blue shows between.
    let drift = camera.time_params.x * 0.004;
    let uv = dir.xz / (dir.y + 0.45) * 0.8 + vec2<f32>(drift, drift * 0.6);
    let warp = vec2<f32>(cloud_fbm(uv * 0.5), cloud_fbm(uv * 0.5 + vec2<f32>(5.2, 1.3)));
    let coverage = cloud_fbm(uv + warp * 0.7);
    // Fade the sheet out at the horizon (where it would alias into a busy band); a broad, soft
    // mid-sky belt of cloud so the dome stops reading as one flat pale wash.
    let band = smoothstep(0.04, 0.32, dir.y);
    let cloud = smoothstep(0.40, 0.72, coverage) * band;
    // Lit toward the sun (warm bright tops), a cooler shadowed base on the far side; the spread gives
    // the banks body rather than reading as flat white paint against the pale sky.
    let sun_side = clamp(dot(dir, sun) * 0.5 + 0.5, 0.0, 1.0);
    let cloud_col = mix(vec3<f32>(0.64, 0.68, 0.76), vec3<f32>(1.30, 1.22, 1.06), sun_side);
    color = mix(color, cloud_col, cloud * 0.9);

    // Sun: a tight bright disc plus a soft surrounding haze, along the key light direction. Drawn
    // after the clouds and only above the horizon, so a low sun does not bleed a second glow into the
    // ground band and the disc burns through thin cloud.
    let disc = pow(d, 900.0) * 6.0;
    let halo = pow(d, 9.0) * 0.20;
    color += camera.key_rgb * (disc + halo) * above;

    return vec4<f32>(tonemap_aces(color), 1.0);
}
