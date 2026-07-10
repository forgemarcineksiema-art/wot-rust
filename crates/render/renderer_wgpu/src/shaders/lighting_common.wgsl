// Shared image formation: the lighting model, aerial perspective and the display transform that
// every lit pass (scene, vehicle, sky, water) grades through, so the terrain, the hulls, the dome
// and the river read as ONE picture. Composed after camera_common.wgsl (these functions read the
// `camera` uniform). Extracted from four hand-mirrored copies — edit image formation here, once.

// Hemispheric ambient: up-facing surfaces take the sky colour, down-facing surfaces the warmer
// ground bounce, blended by the normal's up fraction. This grounds a vehicle in its field instead
// of a flat constant that floods every face equally.
fn hemi_ambient(n: vec3<f32>) -> vec3<f32> {
    let t = clamp(n.y * 0.5 + 0.5, 0.0, 1.0);
    return mix(camera.ground_ambient_rgb, camera.ambient_rgb, t);
}

// Hemispheric ambient plus key/fill/rim directional terms. Directions point towards each light and
// are normalized here; an unlit (black) light contributes nothing.
fn light_radiance(n: vec3<f32>, shadow: f32) -> vec3<f32> {
    let key = max(dot(n, normalize(camera.key_direction)), 0.0) * shadow;
    let fill = max(dot(n, normalize(camera.fill_direction)), 0.0);
    let rim = max(dot(n, normalize(camera.rim_direction)), 0.0);
    return hemi_ambient(n)
        + camera.key_rgb * key
        + camera.fill_rgb * fill
        + camera.rim_rgb * rim;
}

// The analytic sky gradient every smooth material reflects (slate, wet stone, hull steel, the
// river): horizon to zenith by the ray's up fraction. The dome itself shades a softer power curve
// in sky.wgsl; reflections use this cheaper sqrt form everywhere so they all agree.
fn env_sky(dir: vec3<f32>) -> vec3<f32> {
    let up = clamp(dir.y, 0.0, 1.0);
    return mix(camera.sky_horizon_rgb, camera.sky_zenith_rgb, sqrt(up));
}

// Aerial perspective: fade a fragment's HDR radiance toward the horizon/sky colour by distance and
// height, so a 1000 m map reads with real depth instead of as cardboard cut-outs at range. Applied
// in linear HDR *before* the tone curve, and mirrored on the CPU by SceneLighting::fog_factor.
fn apply_fog(color: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let density = camera.fog_params.x;
    if (density <= 0.0) {
        return color;
    }
    let distance = length(camera.camera_pos - world_pos);
    let height_term = exp(-max(world_pos.y, 0.0) * camera.fog_params.y);
    let fog = clamp(1.0 - exp(-max(distance, 0.0) * density * height_term), 0.0, 1.0);
    return mix(color, camera.sky_horizon_rgb, fog);
}

// Gentle display grade after the tone curve: a saturation lift (the raw raster reads pale — grass
// and armour wash toward grey) and a mild contrast S around mid grey (deeper shadows, now that the
// whole world casts them).
fn display_grade(c: vec3<f32>) -> vec3<f32> {
    let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    let saturated = mix(vec3<f32>(luma), c, 1.18);
    let contrasted = (saturated - vec3<f32>(0.5)) * 1.10 + vec3<f32>(0.5);
    return clamp(contrasted, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Filmic ACES-lite tone curve (Narkowicz fit): maps HDR radiance to display range so a hot sun and
// specular roll off instead of clipping to white. The framebuffer is *UnormSrgb, so the hardware
// does the linear->sRGB encode; we output linear, tone-mapped colour and never a manual sRGB pow.
// Bare curve, no grade: the water pass tone-maps without the display grade (pre-existing look,
// preserved exactly — unifying it is a grading decision for the exposure phase, not this refactor).
fn aces_curve(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// The full display transform: ACES-lite curve, then the shared grade.
fn tonemap_aces(x: vec3<f32>) -> vec3<f32> {
    return display_grade(aces_curve(x));
}
