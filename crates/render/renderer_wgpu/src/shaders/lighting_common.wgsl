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

// Profile-driven display grade after the tone curve (grade_params, mirrored on the CPU by
// SceneLighting::grade_reference): a black-point pull so shade reads as shade instead of the
// ACES-lite lifted near-black, a saturation lift around per-pixel luma (the raw raster reads
// pale), and a contrast S around mid grey. All data, not shader constants — each look owns its
// grade.
fn display_grade(c: vec3<f32>) -> vec3<f32> {
    let black = camera.grade_params.y;
    let pulled = clamp((c - vec3<f32>(black)) / (1.0 - black), vec3<f32>(0.0), vec3<f32>(1.0));
    let luma = dot(pulled, vec3<f32>(0.2126, 0.7152, 0.0722));
    let saturated = mix(vec3<f32>(luma), pulled, camera.grade_params.z);
    let contrasted = (saturated - vec3<f32>(0.5)) * camera.grade_params.w + vec3<f32>(0.5);
    return clamp(contrasted, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Filmic ACES-lite tone curve (Narkowicz fit) with the profile's exposure applied in linear HDR
// first: maps radiance to display range so a hot sun and specular roll off instead of clipping to
// white. The framebuffer is *UnormSrgb, so the hardware does the linear->sRGB encode; we output
// linear, tone-mapped colour and never a manual sRGB pow. Curve + exposure only, no grade: the
// water pass tone-maps through this without the display grade (its pre-existing look).
fn aces_curve(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    let exposed = x * camera.grade_params.x;
    return clamp(
        (exposed * (a * exposed + b)) / (exposed * (c * exposed + d) + e),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
}

// The full display transform: exposure + ACES-lite curve, then the profile grade.
fn tonemap_aces(x: vec3<f32>) -> vec3<f32> {
    return display_grade(aces_curve(x));
}
