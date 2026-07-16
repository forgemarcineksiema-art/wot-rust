// Gradient-sky background pass. Draws a single fullscreen triangle behind the geometry (depth
// compare Always, no depth write) and shades each pixel from the view ray direction: a zenith->
// horizon gradient plus a soft sun disc/haze along the key light. The horizon colour matches the
// aerial-perspective fog colour (SceneLighting::sky_horizon_rgb) so distant terrain melts into the
// visible sky instead of meeting a flat clear colour. See docs/atmosphere-policy.md phase 2.
// Composed after camera_common.wgsl and lighting_common.wgsl (camera uniform, display transform).

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
    // Full detail runs five octaves; the reduced tier (time_params.w, F2) folds to three —
    // the two finest octaves shape sub-degree wisps a 20-30 FPS laptop never resolves.
    var octaves = 5;
    if (!detail_bit(8u)) {
        octaves = 3;
    }
    for (var i = 0; i < octaves; i = i + 1) {
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
    // natural banks instead of angular blobs. Coverage is soft-thresholded so open blue shows
    // between. Scale, drift, coverage bias and opacity come from the profile (cloud_params), so
    // each look owns its sky: the rain profile biases the same FBM into an overcast lid, dawn
    // thins it to high sheets.
    let drift = camera.time_params.x * camera.cloud_params.w;
    let uv = dir.xz / (dir.y + 0.45) * 0.8 * camera.cloud_params.y + vec2<f32>(drift, drift * 0.6);
    let warp = vec2<f32>(cloud_fbm(uv * 0.5), cloud_fbm(uv * 0.5 + vec2<f32>(5.2, 1.3)));
    // Cumulus body: the warped FBM plus a RIDGED octave pair — 1-|2n-1| billows the tops into
    // rounded heads instead of soft mush (clouds 2.0). The storm front (cloud2_params.zw)
    // closes coverage along its heading: a dark wall advancing from one horizon, pure profile
    // data (the rain squalls carry one; a clear day carries none).
    let ridged = 1.0 - abs(2.0 * cloud_noise(uv * 3.1 + warp) - 1.0);
    var coverage = cloud_fbm(uv + warp * 0.7) + ridged * 0.18 + camera.cloud_params.x;
    let front_strength = camera.cloud2_params.w;
    var front = 0.0;
    if (front_strength > 0.0) {
        let heading =
            vec2<f32>(cos(camera.cloud2_params.z), sin(camera.cloud2_params.z));
        let flat_dir = normalize(vec2<f32>(dir.x, dir.z) + vec2<f32>(1.0e-4, 0.0));
        front = smoothstep(0.15, 0.85, dot(flat_dir, heading)) * front_strength
            * (1.0 - smoothstep(0.3, 0.6, dir.y));
        coverage += front * 0.5;
    }
    // Fade the sheet out at the horizon (where it would alias into a busy band); a broad, soft
    // mid-sky belt of cloud so the dome stops reading as one flat pale wash.
    let band = smoothstep(0.04, 0.32, dir.y);
    let cloud = smoothstep(0.40, 0.72, coverage) * band;
    // Lit toward the sun, shaded on the far side — both DERIVED from the profile's key and
    // ambient instead of hardcoded constants, so a golden-hour sun paints the banks warm and an
    // overcast profile's weak grey key reads as a lead lid, for free. The front wall darkens
    // its clouds toward the shaded tone regardless of sun side — a storm face holds no warmth.
    let sun_side = clamp(dot(dir, sun) * 0.5 + 0.5, 0.0, 1.0);
    let cloud_lit = camera.key_rgb + camera.ambient_rgb * 0.5;
    let cloud_shade = camera.ambient_rgb * 2.5 + vec3<f32>(0.15, 0.15, 0.15);
    var cloud_col = mix(cloud_shade, cloud_lit, sun_side);
    cloud_col = mix(cloud_col, cloud_shade * 0.8, clamp(front, 0.0, 1.0));
    color = mix(color, cloud_col, cloud * camera.cloud_params.z);

    // The high thin sheet (cloud2_params.xy): a second, finer and slower layer far above the
    // cumulus — pale streaks that give the dome depth between blue and bank. Drawn at low
    // opacity BEHIND the cumulus (its colour never darkens; high ice stays bright).
    let sheet_opacity = camera.cloud2_params.x;
    if (sheet_opacity > 0.0) {
        let sheet_uv = dir.xz / (dir.y + 0.55) * 1.6 * camera.cloud2_params.y
            + vec2<f32>(drift * 0.35, drift * 0.2);
        let sheet = smoothstep(0.52, 0.78, cloud_fbm(sheet_uv))
            * smoothstep(0.08, 0.4, dir.y) * (1.0 - cloud);
        let sheet_col = mix(color, cloud_lit * 0.9 + vec3<f32>(0.1), 0.85);
        color = mix(color, sheet_col, sheet * sheet_opacity);
    }

    // Horizon treatment: a pale distance band where the air thickens against the ground line,
    // so the dome never meets the terrain as a clean gradient cut (rule 4 — one atmosphere).
    let horizon_band = (1.0 - smoothstep(0.0, 0.12, dir.y)) * smoothstep(-0.05, 0.0, dir.y);
    color = mix(color, camera.sky_horizon_rgb * 1.06, horizon_band * 0.5);

    // Sun: a tight bright disc plus a soft surrounding haze, along the key light direction. The
    // haze of the profile fattens and softens the disc (a hazy day has a milky sun); drawn after
    // the clouds and only above the horizon, so a low sun does not bleed a second glow into the
    // ground band and the disc burns through thin cloud.
    let hazy = clamp(camera.fog_params.x * 700.0, 0.0, 1.0);
    let disc = pow(d, mix(900.0, 350.0, hazy)) * mix(6.0, 3.5, hazy);
    let halo = pow(d, mix(9.0, 6.0, hazy)) * mix(0.20, 0.30, hazy);
    color += camera.key_rgb * (disc + halo) * above;

    // Linear HDR out: the sun disc stays hot past 1.0 so the post pass (and later bloom)
    // receives real energy instead of a clipped white.
    return vec4<f32>(color, 1.0);
}
