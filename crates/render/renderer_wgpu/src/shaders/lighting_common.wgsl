// Shared image formation: the lighting model and aerial perspective the lit passes (scene,
// vehicle, sky, water) shade with, plus the display transform (exposure -> ACES-lite -> grade)
// that the central post pass applies ONCE to the resolved HDR frame — so the terrain, the hulls,
// the dome and the river read as ONE picture (art-direction rule 7). Composed after
// camera_common.wgsl (these functions read the `camera` uniform).

// Hemispheric ambient: up-facing surfaces take the sky colour, down-facing surfaces the warmer
// ground bounce, blended by the normal's up fraction. This grounds a vehicle in its field instead
// of a flat constant that floods every face equally.
fn hemi_ambient(n: vec3<f32>) -> vec3<f32> {
    let t = clamp(n.y * 0.5 + 0.5, 0.0, 1.0);
    return mix(camera.ground_ambient_rgb, camera.ambient_rgb, t);
}

// Hemispheric ambient plus key/fill/rim directional terms. Directions point towards each light and
// are normalized here; an unlit (black) light contributes nothing. `ao` (screen-space AO)
// attenuates the INDIRECT terms only — ambient and fill, the light that reaches a crease by
// bouncing. The sun key is already occluded by the shadow map; multiplying it by screen AO too
// double-darkened every sunlit crease and dirtied the whole key-lit field. The rim is a grazing
// silhouette accent and stays out of both.
// Unshadowed local fill pools (worklamps, pane glow): a smooth squared falloff to each light's
// radius, with a soft-wrap facing term so the pool reads as BOUNCED worklight filling a bay, not
// a CG spot with a hard terminator. Indirect-like light, so the caller's screen AO applies (via
// light_radiance) and grounds the pools in creases. A zero radius disables the slot; every
// outdoor profile ships all-off arrays, making this a no-op on the battlefield.
fn local_pools(world_pos: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    var sum = vec3<f32>(0.0, 0.0, 0.0);
    for (var k = 0u; k < 6u; k = k + 1u) {
        let pr = camera.light_pos_radius[k];
        if (pr.w <= 0.0) {
            continue;
        }
        let to_light = pr.xyz - world_pos;
        let d = length(to_light);
        let t = clamp(1.0 - (d * d) / (pr.w * pr.w), 0.0, 1.0);
        let facing = 0.25 + 0.75 * max(dot(n, to_light / max(d, 1.0e-4)), 0.0);
        let ci = camera.light_rgb_intensity[k];
        sum += ci.rgb * ci.w * t * t * facing;
    }
    return sum;
}

fn light_radiance(world_pos: vec3<f32>, n: vec3<f32>, shadow: f32, ao: f32) -> vec3<f32> {
    let key = max(dot(n, normalize(camera.key_direction)), 0.0) * shadow;
    let fill = max(dot(n, normalize(camera.fill_direction)), 0.0) * ao;
    let rim = max(dot(n, normalize(camera.rim_direction)), 0.0);
    return hemi_ambient(n) * ao
        + camera.key_rgb * key
        + camera.fill_rgb * fill
        + camera.rim_rgb * rim
        + local_pools(world_pos, n) * ao;
}

// The analytic sky gradient every smooth material reflects (slate, wet stone, hull steel, the
// river): horizon to zenith by the ray's up fraction. The dome itself shades a softer power curve
// in sky.wgsl; reflections use this cheaper sqrt form everywhere so they all agree.
fn env_sky(dir: vec3<f32>) -> vec3<f32> {
    let up = clamp(dir.y, 0.0, 1.0);
    return mix(camera.sky_horizon_rgb, camera.sky_zenith_rgb, sqrt(up));
}

// Aerial perspective: fade a fragment's HDR radiance toward the horizon haze by distance and
// height, so a 1000 m map reads with real depth instead of as cardboard cut-outs at range. Looked
// at TOWARD the sun, the haze warms toward the key colour (sun-directional scatter — the classic
// backlit-air cue), scaled by the profile's sky_params.y. Colour only: the fog AMOUNT (density,
// height falloff — and with it the 400 m spotting-fairness bound) is exactly the pre-scatter
// model, mirrored on the CPU by SceneLighting::fog_factor. Applied in linear HDR before the tone
// curve.
fn apply_fog(color: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let density = max(camera.fog_params.x, 0.0);
    // The second air layer: valley haze pooled below its fade-out height, quadratic falloff.
    // CPU-mirrored by SceneLighting::fog_factor; the 400 m fairness sweep bounds the SUM.
    var valley = 0.0;
    if (camera.haze_params.y > 0.0) {
        let pooled = clamp(1.0 - max(world_pos.y, 0.0) / camera.haze_params.y, 0.0, 1.0);
        valley = camera.haze_params.x * pooled * pooled;
    }
    if (density <= 0.0 && valley <= 0.0) {
        return color;
    }
    let to_fragment = world_pos - camera.camera_pos;
    let distance = length(to_fragment);
    let height_term = exp(-max(world_pos.y, 0.0) * camera.fog_params.y);
    let fog = clamp(
        1.0 - exp(-max(distance, 0.0) * (density * height_term + valley)), 0.0, 1.0);
    // Sun-directional scatter, colour only: a broad lobe for the general sun-side warmth plus
    // a tight second lobe that makes the air GLOW right around the low sun (forward
    // scattering) — the fog AMOUNT above is untouched by either.
    let toward_sun =
        max(dot(to_fragment / max(distance, 1.0e-4), normalize(camera.key_direction)), 0.0);
    let sun_amount =
        (pow(toward_sun, 8.0) * 0.75 + pow(toward_sun, 64.0) * 0.55) * camera.sky_params.y;
    let sun_haze = camera.sky_horizon_rgb * 0.4 + camera.key_rgb * 0.8;
    let haze = mix(camera.sky_horizon_rgb, sun_haze, min(sun_amount, 1.0));
    return mix(color, haze, fog);
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
// white. Called ONLY by the central post pass (post.wgsl) — the world shaders output linear HDR
// into the Rgba16Float chain and never tone-map themselves (art-direction rule 7). The final
// framebuffer is *UnormSrgb, so the hardware does the linear->sRGB encode; we output linear,
// tone-mapped colour and never a manual sRGB pow.
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
