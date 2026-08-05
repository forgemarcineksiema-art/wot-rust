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
