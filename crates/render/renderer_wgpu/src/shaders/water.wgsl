// The river surface: a static mesh on the still-water plane, animated entirely in-shader from
// the tick-domain presentation clock (camera.time_params.x — never render-frame dt). The
// reflection needs no offscreen target: the sky is an analytic gradient + sun disc, so the
// reflected ray simply re-evaluates the same formula sky.wgsl shades the dome with. Depth-tested
// against the world but never writing depth, alpha-blended over the riverbed drawn beneath it.
// Composed after camera_common.wgsl and lighting_common.wgsl (camera uniform, env_sky gradient,
// ACES curve). NOTE: the surface tone-maps through the bare aces_curve, without display_grade —
// the pre-existing water look, preserved by the shared-WGSL refactor; revisit with the exposure
// phase, which turns the grade into profile data.

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) depth_m: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) depth_m: f32,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    out.world = input.position;
    out.depth_m = input.depth_m;
    out.clip = camera.view_proj * vec4<f32>(input.position, 1.0);
    return out;
}

// The shared env_sky gradient plus the water's own sun: the reflection IS the sky the dome and
// the hulls agree on, not an approximation of it.
fn sky_color(dir: vec3<f32>) -> vec3<f32> {
    var color = env_sky(dir);
    let sun = normalize(camera.key_direction);
    let d = max(dot(dir, sun), 0.0);
    let above = smoothstep(-0.02, 0.06, dir.y);
    // The water's sun: the same halo, a gentler disc — a mirror-sharp 900-power spot aliases
    // into fireflies on a rippled surface.
    let disc = pow(d, 400.0) * 3.0;
    let halo = pow(d, 9.0) * 0.18;
    color += camera.key_rgb * (disc + halo) * above;
    return color;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let time = camera.time_params.x;
    // Two crossing ripple trains perturb the up normal: small amplitude, incommensurate wave
    // lengths and speeds so the pattern never visibly loops. GAMEPLAY water stays a flat plane;
    // this is presentation only.
    let p = input.world.xz;
    let ripple_a = sin(dot(p, vec2<f32>(0.23, 0.11)) + time * 0.9);
    let ripple_b = sin(dot(p, vec2<f32>(-0.13, 0.27)) + time * 1.3);
    let ripple_c = sin(dot(p, vec2<f32>(0.05, -0.07)) + time * 0.5);
    let normal = normalize(vec3<f32>(
        (ripple_a + ripple_c * 0.6) * 0.035,
        1.0,
        (ripple_b - ripple_c * 0.4) * 0.035,
    ));

    let view = normalize(input.world - camera.camera_pos);
    let reflected = reflect(view, normal);
    // Keep the reflected ray above the horizon: a downward reflection would sample the gradient
    // below its definition and read as a hole in the river.
    let sky = sky_color(normalize(vec3<f32>(reflected.x, abs(reflected.y), reflected.z)));

    // The water body: a shallow glassy green deepening into the channel blue by real depth.
    let shallow = vec3<f32>(0.16, 0.30, 0.30);
    let deep = vec3<f32>(0.03, 0.10, 0.18);
    let body = mix(shallow, deep, clamp(input.depth_m / 2.2, 0.0, 1.0));

    // Fresnel: grazing looks are mirror, straight-down looks read the body under the surface.
    let facing = clamp(dot(-view, normal), 0.0, 1.0);
    let fresnel = 0.08 + 0.92 * pow(1.0 - facing, 5.0);
    var color = mix(body, sky, fresnel);

    // Aerial perspective, mirroring scene.wgsl: distant water melts toward the horizon color.
    let distance = length(input.world - camera.camera_pos);
    let fog = 1.0 - exp(-camera.fog_params.x * distance);
    color = mix(color, camera.sky_horizon_rgb, clamp(fog, 0.0, 1.0));

    // Shore fade: film-thin water at the banks dissolves instead of ending in a hard saw edge.
    let alpha = clamp(input.depth_m / 0.4, 0.0, 1.0) * 0.88 + 0.06;
    return vec4<f32>(aces_curve(color), alpha);
}
