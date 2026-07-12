// The river surface (Water 2.0): a static mesh on the still-water plane, animated in-shader from
// the tick-domain presentation clock (camera.time_params.x — never render-frame dt). Waves now
// ADVECT along the baked per-vertex current instead of three static crossing sines, so the river
// reads as flowing; the shallow banks carry scrolling foam flecks. The reflection needs no
// offscreen target: the sky is an analytic gradient + sun disc, so the reflected ray re-evaluates
// the same formula sky.wgsl shades the dome with (refraction — a grab of the scene behind the
// surface — is the deferred A6b step). Depth-tested against the world but never writing depth,
// alpha-blended over the riverbed drawn beneath it. Composed after camera_common.wgsl and
// lighting_common.wgsl (camera uniform, env_sky gradient).

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) depth_m: f32,
    @location(2) flow: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) depth_m: f32,
    @location(2) flow: vec2<f32>,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    out.world = input.position;
    out.depth_m = input.depth_m;
    out.flow = input.flow;
    out.clip = camera.view_proj * vec4<f32>(input.position, 1.0);
    return out;
}

fn water_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn water_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = water_hash(i);
    let b = water_hash(i + vec2<f32>(1.0, 0.0));
    let c = water_hash(i + vec2<f32>(0.0, 1.0));
    let d = water_hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
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

// The muted river body: a turbid grey-green in the shallows deepening to a murky steel-green —
// deliberately NOT a tropical/pool blue. The chroma is kept low so the Bystra reads as a
// Central-European river under an oil-painting sky (art-direction bible rule 2).
fn river_body(depth_m: f32) -> vec3<f32> {
    let shallow = vec3<f32>(0.19, 0.27, 0.23);
    let deep = vec3<f32>(0.06, 0.12, 0.13);
    return mix(shallow, deep, clamp(depth_m / 2.2, 0.0, 1.0));
}

// Pull the surface chroma partway toward its own luma so neither the body nor the reflected sky
// reads as candy-blue — the single "desaturate the water" knob.
fn river_mute(color: vec3<f32>) -> vec3<f32> {
    let luma = dot(color, vec3<f32>(0.299, 0.587, 0.114));
    return mix(vec3<f32>(luma), color, 0.74);
}

// Bank foam: painterly speckle over the thin shore film — two noise octaves advected downstream,
// thresholded so it reads as flecks not a solid rim. Widened (reaches ~0.55 m) and strengthened
// so a steep bank still shows a lace of foam instead of a bare waterline.
fn river_foam(
    color: vec3<f32>,
    p: vec2<f32>,
    flow: vec2<f32>,
    time: f32,
    depth_m: f32,
) -> vec3<f32> {
    let foam_zone = smoothstep(0.55, 0.02, depth_m);
    if (foam_zone <= 0.0) {
        return color;
    }
    let drift = flow * time * 0.35;
    let f1 = water_noise(p * 0.9 - drift);
    let f2 = water_noise(p * 2.3 - drift * 1.7);
    let speckle = smoothstep(0.42, 0.78, f1 * 0.6 + f2 * 0.4);
    let foam = speckle * foam_zone;
    return mix(color, vec3<f32>(0.93, 0.95, 0.96), foam * 0.95);
}

// Shore fade: a wider ramp (over ~0.65 m) so film-thin water at the banks dissolves gently
// instead of ending on a hard saw edge; the very edge starts near-transparent.
fn river_shore_alpha(depth_m: f32) -> f32 {
    return clamp(depth_m / 0.65, 0.0, 1.0) * 0.9 + 0.04;
}

// Refraction (A6b, discrete tier only): a grab of the resolved opaque HDR scene BEHIND the
// surface, sampled by screen UV and bent by the wave normal so the riverbed wobbles under the
// water. Bound ONLY by the refraction pipeline (entry `fs_refract`); the analytic `fs_main`
// never references group 1, so its pipeline needs no such binding.
@group(1) @binding(0) var scene_grab: texture_2d<f32>;
@group(1) @binding(1) var scene_grab_sampler: sampler;
struct RefractionParams {
    // Max screen-UV offset the wave tilt drives. At 0 the refraction collapses to the analytic body.
    strength: f32,
    pad0: f32,
    pad1: f32,
    pad2: f32,
};
@group(1) @binding(2) var<uniform> refraction: RefractionParams;

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let time = camera.time_params.x;
    let p = input.world.xz;

    // The current: the baked downstream direction, and the cross-stream axis. Still water
    // (flow ~ 0) falls back to a fixed frame so a lake still ripples gently.
    var flow = input.flow;
    if (length(flow) < 1.0e-3) {
        flow = vec2<f32>(0.0, 1.0);
    } else {
        flow = normalize(flow);
    }
    let cross_axis = vec2<f32>(-flow.y, flow.x);

    // Waves ADVECT downstream: each train's phase carries `-time * speed` ALONG the flow, so
    // crests travel with the current instead of standing in place. A long primary train down
    // the channel, a finer secondary at a slight angle, and a cross chop for texture.
    let along = dot(p, flow);
    let side = dot(p, cross_axis);
    // Slope of the wave height along the two axes -> a perturbed up-normal.
    let d_along = cos(along * 0.16 - time * 1.1) * 0.16 * 0.9
        + cos((along * 0.31 + side * 0.12) - time * 1.7) * 0.31 * 0.5;
    let d_side = cos((along * 0.31 + side * 0.12) - time * 1.7) * 0.12 * 0.5
        + cos(side * 0.24 - time * 0.6) * 0.24 * 0.7;
    let slope = flow * d_along + cross_axis * d_side;
    let normal = normalize(vec3<f32>(-slope.x * 0.11, 1.0, -slope.y * 0.11));

    let view = normalize(input.world - camera.camera_pos);
    let reflected = reflect(view, normal);
    // Keep the reflected ray above the horizon: a downward reflection would sample the gradient
    // below its definition and read as a hole in the river.
    let sky = sky_color(normalize(vec3<f32>(reflected.x, abs(reflected.y), reflected.z)));

    // The muted river body under the surface, deepening with real depth.
    let body = river_body(input.depth_m);

    // Fresnel: grazing looks are mirror, straight-down looks read the body under the surface. The
    // mirror is cast in a near-neutral cool tint (no blue boost) so it holds the sky without
    // tipping the river into pool-blue.
    let facing = clamp(dot(-view, normal), 0.0, 1.0);
    let fresnel = 0.08 + 0.78 * pow(1.0 - facing, 5.0);
    var color = mix(body, sky * vec3<f32>(0.90, 0.93, 0.95), fresnel);

    color = river_foam(color, p, flow, time, input.depth_m);
    color = river_mute(color);

    // Aerial perspective, mirroring scene.wgsl: distant water melts toward the horizon color.
    let distance = length(input.world - camera.camera_pos);
    let fog = 1.0 - exp(-camera.fog_params.x * distance);
    color = mix(color, camera.sky_horizon_rgb, clamp(fog, 0.0, 1.0));

    let alpha = river_shore_alpha(input.depth_m);
    // Linear HDR out: the river grades through the shared display transform in the post pass.
    return vec4<f32>(color, alpha);
}

// The refraction variant (discrete tier): identical wave/sky/fresnel math to `fs_main`, but the
// under-surface body is the ACTUAL scene behind the water — a grab of the resolved opaque HDR,
// sampled by screen UV and bent by the wave tilt — so a wading hull's legs and the riverbed
// wobble under the current instead of reading as a flat painted body.
@fragment
fn fs_refract(input: VsOut) -> @location(0) vec4<f32> {
    let time = camera.time_params.x;
    let p = input.world.xz;

    var flow = input.flow;
    if (length(flow) < 1.0e-3) {
        flow = vec2<f32>(0.0, 1.0);
    } else {
        flow = normalize(flow);
    }
    let cross_axis = vec2<f32>(-flow.y, flow.x);

    let along = dot(p, flow);
    let side = dot(p, cross_axis);
    let d_along = cos(along * 0.16 - time * 1.1) * 0.16 * 0.9
        + cos((along * 0.31 + side * 0.12) - time * 1.7) * 0.31 * 0.5;
    let d_side = cos((along * 0.31 + side * 0.12) - time * 1.7) * 0.12 * 0.5
        + cos(side * 0.24 - time * 0.6) * 0.24 * 0.7;
    let slope = flow * d_along + cross_axis * d_side;
    let normal = normalize(vec3<f32>(-slope.x * 0.11, 1.0, -slope.y * 0.11));

    let view = normalize(input.world - camera.camera_pos);
    let reflected = reflect(view, normal);
    let sky = sky_color(normalize(vec3<f32>(reflected.x, abs(reflected.y), reflected.z)));

    // The muted river body is the deep-water floor the refraction eases toward with depth.
    let body_base = river_body(input.depth_m);

    // Grab the opaque scene behind this fragment. `input.clip.xy` is the framebuffer pixel; the
    // grab is the full-res resolved opaque HDR, so screen UV = pixel / grab size. Bend the lookup
    // by the wave tilt so the bed distorts with the ripples. A sub-pixel screen-hash jitter breaks
    // up the moiré that the regular wave pattern otherwise beats against the blocky bed texels.
    let dims = vec2<f32>(textureDimensions(scene_grab));
    let screen_uv = input.clip.xy / dims;
    let bend = vec2<f32>(normal.x, normal.z) * refraction.strength;
    let jitter = vec2<f32>(
        water_hash(input.clip.xy) - 0.5,
        water_hash(input.clip.xy + vec2<f32>(37.2, 17.1)) - 0.5,
    ) * (2.0 / dims);
    let ruv = clamp(screen_uv + bend + jitter, vec2<f32>(0.002), vec2<f32>(0.998));
    let refracted = textureSampleLevel(scene_grab, scene_grab_sampler, ruv, 0.0).rgb;
    // Water tints and darkens what shows through it, deepening with depth (absorption).
    let absorb = clamp(input.depth_m / 2.2, 0.0, 1.0);
    let tint = mix(vec3<f32>(0.72, 0.82, 0.72), vec3<f32>(0.10, 0.22, 0.22), absorb);
    let seen_through = refracted * tint;
    // Shallow water is clear (the bed shows through); deep water hides it behind the body colour.
    let clarity = clamp(1.0 - input.depth_m / 1.6, 0.0, 1.0);
    let body = mix(body_base, seen_through, clarity);

    let facing = clamp(dot(-view, normal), 0.0, 1.0);
    let fresnel = 0.08 + 0.78 * pow(1.0 - facing, 5.0);
    var color = mix(body, sky * vec3<f32>(0.90, 0.93, 0.95), fresnel);

    color = river_foam(color, p, flow, time, input.depth_m);
    color = river_mute(color);

    let distance = length(input.world - camera.camera_pos);
    let fog = 1.0 - exp(-camera.fog_params.x * distance);
    color = mix(color, camera.sky_horizon_rgb, clamp(fog, 0.0, 1.0));

    // Refracted water reads a touch clearer, so the surface itself is slightly more opaque.
    let alpha = clamp(input.depth_m / 0.55, 0.0, 1.0) * 0.94 + 0.05;
    return vec4<f32>(color, alpha);
}
