// The meadow's shared arithmetic (Jedna Trawa): the ONE place that decides how far grass
// geometry stands and how much of the meadow the GROUND has to carry on its own. Composed
// into BOTH the scene pass (which draws the tufts) and the terrain pass (which draws what
// they stand on) — the two must agree to the metre or their hand-off reads as a line.
// Composed after camera_common.wgsl (it reads the camera uniform) and noise_common.wgsl.

// How far a frame's grass bands stretch. Read from the projection's Y scale
// (ssao_params.w = cot(fov_y/2)), which the renderer recovers from the view-projection every
// frame — so this is the SAME number the CPU chunk cutoff uses, with no camera state to
// synchronise. Mirrors renderer_api::grass_zoom_band_scale; wgsl_layout locks the two ends.
const GRASS_ZOOM_REFERENCE_PROJ_Y: f32 = 1.921;
const GRASS_ZOOM_BAND_CAP: f32 = 4.0;

fn grass_zoom_band_scale() -> f32 {
    return clamp(camera.ssao_params.w / GRASS_ZOOM_REFERENCE_PROJ_Y, 1.0, GRASS_ZOOM_BAND_CAP);
}

// The far costume's collapse band, in metres from the eye.
const MEADOW_FAR_COLLAPSE_START_M: f32 = 260.0;
const MEADOW_FAR_COLLAPSE_END_M: f32 = 330.0;

// How much of itself the far meadow still shows at `distance` — 1 while the far tufts stand
// full height, 0 once they have folded into the ground.
fn meadow_far_stand(distance: f32) -> f32 {
    let zoom = grass_zoom_band_scale();
    return 1.0 - smoothstep(
        MEADOW_FAR_COLLAPSE_START_M * zoom,
        MEADOW_FAR_COLLAPSE_END_M * zoom,
        distance,
    );
}

// Costume C (the ground wearing the meadow). Grass is darker than the soil it grows from:
// blades shade each other and the gaps between them hold shadow, which is why a real meadow
// reads deeper than bare ground of the same material. Near the eye the ground shows only the
// shade BETWEEN standing tufts; where the far costume has folded away, the ground is all the
// meadow there is, so it takes the tufts' full share — the collapse becomes a dissolve into
// tone instead of a horizon where grass stops. Vegetation-weighted, so roads, rock and the
// riverbed keep their own colour. Mirrors renderer_api::meadow_ground_shade.
const MEADOW_SHADE_STANDING: f32 = 0.05;
const MEADOW_SHADE_COLLAPSED: f32 = 0.17;

fn meadow_ground_shade(vegetation: f32, distance: f32) -> f32 {
    let carried = mix(MEADOW_SHADE_COLLAPSED, MEADOW_SHADE_STANDING, meadow_far_stand(distance));
    return 1.0 - carried * clamp(vegetation, 0.0, 1.0);
}

// --- Wind 2.0 (Jedna Trawa P7) ---------------------------------------------------------
//
// The field is pushed by ONE wind, and it is the wind the sky already has: the storm front's
// heading (cloud2_params.z) that drives the cloud sheet. What moves the clouds lays the
// grass — no second wind to keep in sync, and a gusty sky reads as a gusty field.
//
// A gust is a FRONT, not a hum: a low-frequency field advected ALONG the wind, stretched
// across it, so waves visibly roll over the meadow instead of every blade humming in place.

/// Baseline breeze the field always carries, and how much a storm front adds on top.
const MEADOW_WIND_BASE: f32 = 0.42;
const MEADOW_WIND_STORM_GAIN: f32 = 1.15;
/// The gust field: ~28 m along the wind, ~85 m across it (long streaks), rolling downwind.
const MEADOW_GUST_ALONG: f32 = 0.036;
const MEADOW_GUST_ACROSS: f32 = 0.012;
const MEADOW_GUST_SPEED: f32 = 3.4;
/// Deflection per metre of blade height, and the flutter riding on top of it.
const MEADOW_BEND_PER_HEIGHT: f32 = 0.42;
const MEADOW_FLUTTER: f32 = 0.055;

// --- The tank in the meadow (Jedna Trawa P9) ------------------------------------------
//
// Forty tonnes drive through a field and the field answers. The hull presses grass FLAT and
// pushes it outward from where it stands — the same arc as the wind (pinned at the root,
// dropping with the square of the deflection), but the direction is the machine's, not the
// weather's, and it wins: nothing sways under a tank.
//
// This is presentation, not gameplay. Grass never hid anything (the 0.6 m honesty cap), so
// flattening it reveals nothing either — and because every client derives it from the same
// replicated tank positions, every player sees the same field. Nobody's GPU buys them
// information.

/// How hard the meadow is pressed at `distance` from a crusher's centre: 1 under the hull,
/// easing to 0 at the radius. Mirrors renderer_api::grass_crush_strength.
fn meadow_crush(distance: f32, radius: f32) -> f32 {
    if (radius <= 0.0) {
        return 0.0;
    }
    let t = clamp(1.0 - distance / radius, 0.0, 1.0);
    return t * t;
}

/// The strongest press on this root, and the direction to shove the blades — away from the
/// hull that is standing on them.
fn meadow_crush_at(root_xz: vec2<f32>) -> vec3<f32> {
    var best = 0.0;
    var push = vec2<f32>(0.0, 0.0);
    for (var i = 0u; i < 6u; i = i + 1u) {
        let slot = camera.crusher_pos_radius[i];
        if (slot.w <= 0.0) {
            continue;
        }
        let away = root_xz - slot.xz;
        let strength = meadow_crush(length(away), slot.w);
        if (strength > best) {
            best = strength;
            // Dead centre under the hull there is no outward direction to speak of; the
            // grass there is simply flattened, so a zero push is the honest answer.
            push = normalize(away + vec2<f32>(1.0e-4, 0.0));
        }
    }
    return vec3<f32>(push.x, best, push.y);
}

/// The world-space displacement of one grass vertex.
///
/// `reach` is the vertex's wind lane already scaled into world metres and faded with its
/// costume (mesh-local sway × instance scale × stand): it carries BOTH how high up the blade
/// this vertex sits and how stiff its species is, so one formula serves carpet and seed head.
///
/// The tip does not slide sideways — it swings on an arc pinned at the root, so the drop is
/// QUADRATIC in the deflection (the chord of a bent blade of that length). A blade lies down
/// under a gust instead of skating downwind, which is the whole difference between grass and
/// a flag.
fn meadow_wind_offset(world_xz: vec2<f32>, root_xz: vec2<f32>, reach: f32, t: f32) -> vec3<f32> {
    if (reach <= 0.0) {
        return vec3<f32>(0.0);
    }
    // A hull standing on this tuft overrules the weather: the crushed share of the blade
    // is bent by the machine, and only what is left of it still feels the wind.
    let crush = meadow_crush_at(root_xz);
    if (crush.y > 0.0) {
        let flattened = reach * crush.y;
        let bend = flattened * MEADOW_CRUSH_SPREAD;
        let drop = bend * bend / max(reach * 2.0, 0.02);
        let free = meadow_wind_offset_free(world_xz, root_xz, reach * (1.0 - crush.y), t);
        return vec3<f32>(crush.x * bend, -drop, crush.z * bend) + free;
    }
    return meadow_wind_offset_free(world_xz, root_xz, reach, t);
}

/// How far a fully crushed blade is shoved outward, per metre of its height. Above 1.0 the
/// blade would be driven past flat and start climbing out the far side.
const MEADOW_CRUSH_SPREAD: f32 = 0.95;

/// The wind alone, with no machine standing on the tuft.
fn meadow_wind_offset_free(
    world_xz: vec2<f32>,
    root_xz: vec2<f32>,
    reach: f32,
    t: f32,
) -> vec3<f32> {
    if (reach <= 0.0) {
        return vec3<f32>(0.0);
    }
    let heading = camera.cloud2_params.z;
    let wind = vec2<f32>(cos(heading), sin(heading));
    let across = vec2<f32>(-wind.y, wind.x);
    let along_m = dot(world_xz, wind);
    // The gust front: one low-frequency sample, advected downwind.
    let front = value_noise(vec2<f32>(
        along_m * MEADOW_GUST_ALONG - t * MEADOW_GUST_ALONG * MEADOW_GUST_SPEED,
        dot(world_xz, across) * MEADOW_GUST_ACROSS,
    ));
    let strength = MEADOW_WIND_BASE
        + MEADOW_WIND_STORM_GAIN * clamp(camera.cloud2_params.w, 0.0, 1.0);
    // Per-tuft phase from its ROOT: neighbours share the gust but not the tremor, so the
    // field breathes together without moving as one rigid sheet. A cheap trig hash, not a
    // second lattice sample — this runs on every grass vertex in the frame, and the tremor
    // only needs to be decorrelated, not smooth.
    let phase = fract(sin(dot(root_xz, vec2<f32>(12.9898, 78.233))) * 43758.5453) * 6.2831853;
    let flutter = sin(t * 7.3 + phase) * MEADOW_FLUTTER;
    let deflection = (front * strength + flutter) * reach * MEADOW_BEND_PER_HEIGHT;
    // Arc, not slide: drop = deflection² / (2 × blade length), the chord of the bend.
    let drop = deflection * deflection / max(reach * 2.0, 0.02);
    return vec3<f32>(wind.x * deflection, -drop, wind.y * deflection);
}
