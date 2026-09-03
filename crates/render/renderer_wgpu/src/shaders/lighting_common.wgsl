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
        // Outside the pool the slot contributes exactly zero, so leave before the facing term
        // and the multiply: with the shot lights live (Inny Poziom S1) nearly every fragment
        // of a 1080p frame is outside every pool, and this early-out is what keeps six live
        // flashes affordable on the MX330. Inside, the arithmetic is the same as before.
        if (d * d >= pr.w * pr.w) {
            continue;
        }
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

// Canopy light (surface_role::FOLIAGE): leaves are thin scatterers, not opaque walls, so the
// key wears a WRAPPED falloff — light reaches around a card the way it filters through a real
// crown — plus a small transmission lobe when the sun stands behind the card. Both terms stay
// inside the cast shadow, so a crown in a building's shade gains nothing. The asset's baked
// vertex AO (COLOR_0) carries the deep-interior darkening; this keeps the SURFACE falloff soft.
fn foliage_radiance(world_pos: vec3<f32>, n: vec3<f32>, shadow_in: f32, ao: f32) -> vec3<f32> {
    let key_dir = normalize(camera.key_direction);
    // A crown's shadow on its own leaves is POROUS: the shadow map sees an opaque lid of
    // cards, a real canopy lets a good share of the sun through between and through the
    // blades. Leaves receive the map at 60 % (Leaves 2.0) — the side of a crown under a high
    // sun is a lit green wall, not the black underside of a lid.
    let shadow = 0.4 + 0.6 * shadow_in;
    let wrap = 0.6;
    let n_dot_key = dot(n, key_dir);
    let key = clamp((n_dot_key + wrap) / (1.0 + wrap), 0.0, 1.0) * shadow;
    // Transmission is what lights the side of a crown that faces AWAY from the sun: the
    // light comes through the leaves (a backlit crown glows, it does not go black), and it
    // reaches a leaf its own crown shadows, which the shadow map cannot see — so the lobe is
    // strong and keeps a floor in shade (Leaves 2.0, with the crown normals).
    let transmit = max(-n_dot_key, 0.0) * 0.55 * (0.45 + 0.55 * shadow);
    let fill = max(dot(n, normalize(camera.fill_direction)), 0.0) * ao;
    let rim = max(dot(n, normalize(camera.rim_direction)), 0.0);
    // A leaf mass gathers more sky than an opaque surface of the same normal: light passes
    // between and through the blades.
    return hemi_ambient(n) * ao * 1.2
        + camera.key_rgb * (key + transmit)
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
    // scattering) — the fog AMOUNT above is untouched by either. The blend weights sum to 1.0
    // on purpose: scatter WARMS the haze toward the key, it never makes the air BRIGHTER than
    // the sky it belongs to (the old 0.4/0.8 pair produced an over-white HDR haze — the whole
    // sun-side distance graded to milk). CPU-mirrored by SceneLighting::fog_sun_haze_reference
    // and locked in look_locks.
    let toward_sun =
        max(dot(to_fragment / max(distance, 1.0e-4), normalize(camera.key_direction)), 0.0);
    let sun_amount =
        (pow(toward_sun, 8.0) * 0.75 + pow(toward_sun, 64.0) * 0.55) * camera.sky_params.y;
    let sun_haze = camera.sky_horizon_rgb * 0.55 + camera.key_rgb * 0.45;
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
    // Contrast as a real S-curve WITH A TOE, not a straight line through mid grey. The old
    // `(x - 0.5) * contrast + 0.5` carried slope `contrast` everywhere, so everything below
    // `0.5 - 0.5/contrast` — 0.054 at the shipped 1.12 — was driven negative and clamped to pure
    // black. That band is exactly where a hull's shaded flank lives: measured on the backlit
    // review frame, the vehicle's median pixel arrived here at 0.068 and left at 0.016.
    //
    // `smoothstep` is the ordinary S: slope 1.5 at mid grey, slope 0 at both ends. Blending
    // toward it by `k` keeps the profile's `contrast` meaning exactly what it always meant — the
    // slope at mid grey, which is `1 + k/2` — while the toe COMPRESSES the darks instead of
    // clipping them. The lit end does not move (0.556 stays 0.556); the shade gains its
    // structure back. Mirrored on the CPU by `SceneLighting::grade_reference`.
    let k = clamp((camera.grade_params.w - 1.0) * 2.0, 0.0, 1.0);
    let contrasted = mix(saturated, smoothstep(vec3<f32>(0.0), vec3<f32>(1.0), saturated), k);
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

// The exact piecewise sRGB transfer pair (not a gamma approximation). Two consumers, one copy:
// the post pass dithers on the real 8-bit quantization grid, and the FXAA pass — which works on
// the ENCODED picture, where luma is perceptual — decodes its result back to display-linear for
// the hardware encode of the sRGB target.
fn srgb_encode(c: vec3<f32>) -> vec3<f32> {
    let low = c * 12.92;
    let high = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, c <= vec3<f32>(0.0031308));
}

fn srgb_decode(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}
