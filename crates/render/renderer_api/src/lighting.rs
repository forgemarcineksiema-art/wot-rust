//! Calibrated scene-lighting profiles (hemispheric ambient + key/fill/rim, gradient sky and
//! aerial-perspective fog) shared by the scene and vehicle shaders. Backend-neutral data turned
//! into GPU bytes in exactly one place (`renderer_wgpu::CameraUniform::from_scene`). See
//! `docs/atmosphere-policy.md`.

/// How many local light slots ride the camera uniform. Fixed so the GPU layout is static; unused
/// slots are disabled by `radius_m == 0` and cost one uniform read.
pub const MAX_LOCAL_LIGHTS: usize = 6;

/// An unshadowed local fill pool — a worklamp over the turntable, the glow of a frosted pane.
/// Purely additive on top of the directional rig, attenuated by distance so it reads as a POOL
/// of light, never a sun. No shadowing: these are bounce-light approximations, kept soft.
/// `radius_m == 0` disables the slot (the outdoor profiles carry all-off arrays for free).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalLight {
    /// World position of the emitter.
    pub position: [f32; 3],
    /// Radius the pool fades to zero at; 0 disables the light entirely.
    pub radius_m: f32,
    /// Linear colour.
    pub rgb: [f32; 3],
    /// Intensity multiplier on the colour.
    pub intensity: f32,
}

impl LocalLight {
    pub const OFF: Self = Self { position: [0.0; 3], radius_m: 0.0, rgb: [0.0; 3], intensity: 0.0 };

    /// CPU mirror of the shader falloff (`local_pools` in `lighting_common.wgsl`):
    /// `t = clamp(1 - d²/r², 0, 1); t²`. Returns 0 for a disabled light, 1 at the emitter,
    /// 0 at and beyond the radius — testable without a GPU.
    pub fn attenuation_at(&self, distance_m: f32) -> f32 {
        if self.radius_m <= 0.0 {
            return 0.0;
        }
        let t = (1.0 - (distance_m * distance_m) / (self.radius_m * self.radius_m)).clamp(0.0, 1.0);
        t * t
    }
}

/// The all-off local light array every outdoor profile carries: zero radius = disabled slots.
pub const NO_LOCAL_LIGHTS: [LocalLight; MAX_LOCAL_LIGHTS] = [LocalLight::OFF; MAX_LOCAL_LIGHTS];

/// Calibrated outdoor scene lighting: a hemispheric sky/ground ambient plus key/fill/rim directional
/// lights, consumed by both the scene and the vehicle shaders. Each `*_direction` is a world-space
/// vector pointing *towards* the light (the shader normalizes it); each `*_rgb` is that light's
/// linear colour and intensity (the sun key may exceed `1.0` for HDR punch the tone curve rolls
/// off). `ambient_rgb` is the *sky* (upper-hemisphere) ambient and `ground_ambient_rgb` the warmer
/// ground bounce; the shader blends them by the surface normal's up-facing fraction so a vehicle is
/// grounded in its field instead of flooded by one flat constant. See `docs/atmosphere-policy.md`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneLighting {
    /// Upper-hemisphere (sky) ambient — taken by up-facing surfaces.
    pub ambient_rgb: [f32; 3],
    /// Lower-hemisphere (ground bounce) ambient — taken by down-facing surfaces.
    pub ground_ambient_rgb: [f32; 3],
    pub key_direction: [f32; 3],
    pub key_rgb: [f32; 3],
    pub fill_direction: [f32; 3],
    pub fill_rgb: [f32; 3],
    pub rim_direction: [f32; 3],
    pub rim_rgb: [f32; 3],
    /// Gradient-sky zenith colour (straight up) — linear. The visible sky the ambient hemisphere
    /// samples; see `docs/atmosphere-policy.md` phase 2.
    pub sky_zenith_rgb: [f32; 3],
    /// Gradient-sky horizon colour (and the aerial-perspective fog colour distant surfaces fade
    /// toward) — linear. Distant terrain/vehicles desaturate to this so a 1000 m map reads with
    /// real depth instead of as cardboard cut-outs.
    pub sky_horizon_rgb: [f32; 3],
    /// Distance-fog density: larger fades the horizon in sooner. 0 disables fog (interior looks).
    pub fog_density: f32,
    /// Height falloff for the fog: how fast the fog thins with world height, so valleys fill and
    /// ridgelines cut through. 0 makes the fog uniform with height.
    pub fog_height_falloff: f32,
    /// Linear exposure multiplier applied to HDR radiance *before* the ACES tone curve: the one
    /// knob that makes the whole picture read brighter or moodier without re-tuning every light.
    /// 1.0 is neutral; profiles stay within `[0.5, 2.0]` (locked by tests).
    pub exposure: f32,
    /// Display black point: post-curve values at or below it are pulled to true black
    /// (`(c - black) / (1 - black)`), undoing the ACES-lite lifted near-blacks so shade reads as
    /// shade. 0 is neutral; profiles stay within `[0, 0.08]`.
    pub black_point: f32,
    /// Display saturation around per-pixel luma; 1.0 is neutral. Replaces the old constant 1.18
    /// hardcoded in four shaders — the grade is profile data now.
    pub saturation: f32,
    /// Display contrast S-curve slope around mid grey; 1.0 is neutral. Replaces the old
    /// hardcoded 1.10.
    pub contrast: f32,
    /// Cloud coverage bias, added to the sky FBM before the coverage threshold: 0 is the clear-day
    /// baseline, positive values thicken the banks (≥ ~0.3 reads as an overcast lid), negative
    /// values thin them to high sheets.
    pub cloud_coverage_bias: f32,
    /// Cloud pattern scale (UV multiplier): 1.0 is the baseline bank size; higher = finer/higher
    /// sheets.
    pub cloud_scale: f32,
    /// Cloud opacity over the sky gradient (0..1). 0 removes the layer entirely.
    pub cloud_opacity: f32,
    /// Cloud drift speed in UV per presentation second (tick-domain clock).
    pub cloud_drift: f32,
    /// How strongly the cloud layer shades the terrain's sun (0..1). Kept at 0 under overcast —
    /// the lid itself is the shadow — and gated per tier by `LightingQuality::cloud_shadows`.
    pub cloud_shadow_strength: f32,
    /// Sun-directional scatter in the aerial perspective (0..1): haze looked at *toward* the sun
    /// warms toward the key colour instead of the flat horizon grey. Colour only — the fog
    /// density/height model (and its 400 m fairness bound) is untouched by this.
    pub fog_sun_scatter: f32,
    /// How milky the sun disc reads (0 = a hard crisp disc, 1 = a fat soft glow). Explicit
    /// profile data: the sky pass used to derive this from `fog_density`, which coupled the
    /// disc's hardness to the spotting-fairness fog tuning — retuning the air silently retuned
    /// the sun. Now the fairness knob and the look knob are separate.
    pub sun_softness: f32,
    /// Low-lying valley haze: a SECOND fog layer pooled below `valley_haze_height_m`, its
    /// density fading quadratically to zero at that height. Dawn mist finally sits IN the
    /// valley floor instead of everywhere. The 400 m spotting-fairness sweep covers the SUM of
    /// both layers at every fighting height — a profile that pools too hard fails the gate.
    pub valley_haze_density: f32,
    /// Height (m) the valley haze fades out at; 0 disables the layer.
    pub valley_haze_height_m: f32,
    /// Screen-space crepuscular rays in the central post pass: how strongly hot HDR energy
    /// (the sun disc and its halo) streaks toward the camera through the air. Painted light
    /// shafts, not a volumetric sim — profile-gated, 0 skips the march entirely.
    pub god_ray_strength: f32,
    /// A second, high thin cloud sheet over the cumulus layer: opacity (0 removes it).
    pub cloud_sheet_opacity: f32,
    /// Pattern scale of the high sheet relative to the cumulus layer.
    pub cloud_sheet_scale: f32,
    /// Storm-front heading in radians (world XZ): the horizon direction the front wall
    /// advances from. Only read when `storm_front_strength > 0`.
    pub storm_front_dir_rad: f32,
    /// How hard the front closes coverage and darkens cloud colour along its heading (0..1).
    pub storm_front_strength: f32,
    /// Threshold-free bloom composite weight in the central post pass: how much of the blurred
    /// HDR frame folds back over the sharp one BEFORE the tone curve. Energy-proportional — only
    /// genuinely hot sources (sun, tracers, fires, glints) visibly glow (art-direction rule 6).
    /// Profiles stay within `[0, 0.10]` (locked); 0 disables the chain.
    pub bloom_weight: f32,
    /// Display vignette strength after the grade: 0 = none, capped at 0.15 by the art-direction
    /// bible (rule 6 — the camera is an eye, not a lens).
    pub vignette: f32,
    /// Unshadowed local fill pools (see [`LocalLight`]). All-off on every outdoor profile
    /// ([`NO_LOCAL_LIGHTS`]); the garage rig hangs its worklamps here.
    pub local_lights: [LocalLight; MAX_LOCAL_LIGHTS],
}

impl SceneLighting {
    /// Aerial-perspective fog factor for a surface `distance` metres from the camera at world
    /// `height`: 0 = no fog (near/high), 1 = fully faded to `sky_horizon_rgb`. The CPU mirror of the
    /// shaders' `apply_fog`, so the model is testable without a GPU. Fog thickens with distance and
    /// thins with height; density 0 returns 0 everywhere.
    pub fn fog_factor(&self, distance: f32, height: f32) -> f32 {
        let valley = if self.valley_haze_height_m > 0.0 {
            let pooled = (1.0 - height.max(0.0) / self.valley_haze_height_m).clamp(0.0, 1.0);
            self.valley_haze_density * pooled * pooled
        } else {
            0.0
        };
        let density = self.fog_density.max(0.0);
        if density <= 0.0 && valley <= 0.0 {
            return 0.0;
        }
        let height_term = (-height.max(0.0) * self.fog_height_falloff).exp();
        let f = 1.0 - (-distance.max(0.0) * (density * height_term + valley)).exp();
        f.clamp(0.0, 1.0)
    }

    /// The full display transform for one linear HDR colour: exposure → ACES-lite curve → black
    /// point pull → saturation → contrast. The CPU mirror of the shaders' `aces_curve` +
    /// `display_grade` (lighting_common.wgsl), kept in exact lockstep so the image formation is
    /// testable without a GPU — the same role `fog_factor` plays for `apply_fog`.
    pub fn grade_reference(&self, hdr: [f32; 3]) -> [f32; 3] {
        let aces = |x: f32| {
            let x = x * self.exposure;
            ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
        };
        let black = self.black_point;
        let pulled = hdr.map(|c| ((aces(c) - black) / (1.0 - black)).clamp(0.0, 1.0));
        let luma = 0.2126 * pulled[0] + 0.7152 * pulled[1] + 0.0722 * pulled[2];
        // The contrast S and its toe — see `display_grade` in lighting_common.wgsl for why the
        // straight line had to go. `smoothstep` clamps its argument before the polynomial, and
        // the blend runs against the UNCLAMPED saturated value, exactly as WGSL's `mix` does.
        let k = ((self.contrast - 1.0) * 2.0).clamp(0.0, 1.0);
        pulled.map(|c| {
            let saturated = luma + (c - luma) * self.saturation;
            let t = saturated.clamp(0.0, 1.0);
            let smooth = t * t * (3.0 - 2.0 * t);
            (saturated + (smooth - saturated) * k).clamp(0.0, 1.0)
        })
    }

    /// The battlefield look: a warm sun key raking low from the side (so it sculpts the sides of a
    /// low hull, not just the decks), a cool sky fill and sky ambient from above, a warm ground
    /// bounce from below, and a live sky rim that lifts the silhouette off the horizon. Tuned to be
    /// read through the ACES-lite tone curve, so the key deliberately runs hot.
    pub fn battlefield_default() -> Self {
        Self {
            // A touch deeper and cooler than before, so the cast shadows the world now throws read as
            // real shade instead of a flooded mid-grey. Still well above the ground bounce (the
            // hemispheric-ambient invariant) and above the fog floor (a target at 400 m stays read).
            ambient_rgb: [0.16, 0.19, 0.26],
            ground_ambient_rgb: [0.14, 0.13, 0.10],
            key_direction: [0.62, 0.52, 0.34],
            // A hotter, warmer sun: with a deeper ambient the sunlit decks now separate from the
            // shaded flanks, and the ACES curve rolls the extra punch off the top rather than clipping.
            key_rgb: [1.28, 1.14, 0.90],
            fill_direction: [-0.5, 0.62, -0.28],
            fill_rgb: [0.16, 0.19, 0.25],
            rim_direction: [-0.42, 0.4, -0.88],
            rim_rgb: [0.20, 0.23, 0.30],
            // A clear-day sky: a deeper, more saturated blue overhead easing to a hazy blue-grey at
            // the horizon (less milky than before, so white cloud reads against it). The horizon
            // doubles as the fog colour, so distant hills still melt into the same haze.
            sky_zenith_rgb: [0.15, 0.32, 0.62],
            sky_horizon_rgb: [0.58, 0.70, 0.82],
            // Very light haze, tuned so enemy vehicles stay crisply readable at combat range:
            // ~4% fade at 300 m, ~6% at 500 m, only ~10-15% out past 1 km where the far terrain
            // melts into the horizon. Aerial perspective for depth, never for hiding targets.
            fog_density: 0.00013,
            fog_height_falloff: 0.02,
            // First-pass image formation (the proper per-map taste pass is a later phase): a
            // touch of extra exposure so the sunlit field glows, a real black point so cast
            // shadows finally reach black, and slightly more contrast than the old hardcoded
            // 1.10 now that the blacks anchor it.
            exposure: 1.1,
            black_point: 0.03,
            saturation: 1.18,
            contrast: 1.15,
            // A clear day with scattered banks: baseline coverage and drift, a gentle patchwork
            // of cloud shade wandering the field, light warm scatter around the sun.
            cloud_coverage_bias: 0.0,
            cloud_scale: 1.0,
            cloud_opacity: 0.9,
            cloud_drift: 0.004,
            cloud_shadow_strength: 0.25,
            fog_sun_scatter: 0.5,
            // Matches the old fog-derived value (0.00013 * 700) so the clear-day disc is unchanged.
            sun_softness: 0.09,
            valley_haze_density: 0.0,
            valley_haze_height_m: 0.0,
            god_ray_strength: 0.0,
            cloud_sheet_opacity: 0.25,
            cloud_sheet_scale: 1.4,
            storm_front_dir_rad: 0.0,
            storm_front_strength: 0.0,
            bloom_weight: 0.05,
            vignette: 0.08,
            local_lights: NO_LOCAL_LIGHTS,
        }
    }

    /// Dolina Bystrej, golden afternoon: the sun low in the WEST over the farmland flank, so it
    /// rakes across the valley into the town's facades and glitters down the river. Warm key,
    /// blue-shadowed ambient, the lightest haze of the three variants.
    pub fn bystra_clear_afternoon() -> Self {
        Self {
            ambient_rgb: [0.19, 0.22, 0.28],
            ground_ambient_rgb: [0.17, 0.14, 0.10],
            key_direction: [-0.62, 0.34, 0.18],
            key_rgb: [1.18, 0.94, 0.66],
            fill_direction: [0.55, 0.60, -0.30],
            fill_rgb: [0.16, 0.19, 0.26],
            rim_direction: [0.45, 0.38, 0.85],
            rim_rgb: [0.22, 0.22, 0.26],
            sky_zenith_rgb: [0.20, 0.33, 0.55],
            sky_horizon_rgb: [0.78, 0.72, 0.62],
            fog_density: 0.00015,
            fog_height_falloff: 0.02,
            // Golden afternoon: warm light wants saturation and glow, gentle blacks.
            exposure: 1.1,
            black_point: 0.025,
            saturation: 1.22,
            contrast: 1.12,
            // Golden afternoon: a few more banks than the battle noon, drifting cloud shade on
            // the farmland, and a strong warm glow in the haze around the low western sun.
            cloud_coverage_bias: 0.04,
            cloud_scale: 1.1,
            cloud_opacity: 0.9,
            cloud_drift: 0.004,
            cloud_shadow_strength: 0.3,
            fog_sun_scatter: 0.65,
            sun_softness: 0.1,
            valley_haze_density: 5e-05,
            valley_haze_height_m: 8.0,
            god_ray_strength: 0.0,
            cloud_sheet_opacity: 0.3,
            cloud_sheet_scale: 1.5,
            storm_front_dir_rad: 0.0,
            storm_front_strength: 0.0,
            bloom_weight: 0.05,
            vignette: 0.08,
            local_lights: NO_LOCAL_LIGHTS,
        }
    }

    /// Dolina Bystrej, rain squalls: a lead sky, no sun disc worth the name — a weak cool key
    /// straight down, flat ambient, and the densest fog the fairness bound allows (see the
    /// fog-fairness test: a spotted tank at 400 m must stay identifiable in EVERY variant).
    pub fn bystra_rain() -> Self {
        Self {
            ambient_rgb: [0.22, 0.24, 0.27],
            ground_ambient_rgb: [0.12, 0.13, 0.13],
            key_direction: [0.15, 0.92, 0.20],
            key_rgb: [0.42, 0.46, 0.52],
            fill_direction: [-0.40, 0.50, -0.60],
            fill_rgb: [0.14, 0.16, 0.19],
            rim_direction: [-0.30, 0.45, 0.80],
            rim_rgb: [0.16, 0.18, 0.22],
            sky_zenith_rgb: [0.30, 0.34, 0.39],
            sky_horizon_rgb: [0.46, 0.50, 0.54],
            fog_density: 0.0009,
            fog_height_falloff: 0.004,
            // Rain: a flat lead-grey day — near-neutral saturation, soft contrast, shallow
            // blacks (an overcast sky fills every shadow).
            exposure: 1.0,
            black_point: 0.015,
            saturation: 1.06,
            contrast: 1.08,
            // Rain: the coverage bias pushes the same FBM into a genuine overcast lid; no cloud
            // shade on the ground (the lid IS the shadow), no sun to scatter around.
            cloud_coverage_bias: 0.35,
            cloud_scale: 1.3,
            cloud_opacity: 0.97,
            cloud_drift: 0.006,
            cloud_shadow_strength: 0.0,
            fog_sun_scatter: 0.15,
            // A rain sky holds no hard disc: the softest sun of the outdoor set.
            sun_softness: 0.63,
            valley_haze_density: 0.0001,
            valley_haze_height_m: 10.0,
            god_ray_strength: 0.0,
            cloud_sheet_opacity: 0.35,
            cloud_sheet_scale: 1.2,
            storm_front_dir_rad: 2.4,
            storm_front_strength: 0.5,
            bloom_weight: 0.04,
            vignette: 0.06,
            local_lights: NO_LOCAL_LIGHTS,
        }
    }

    /// Dolina Bystrej, dawn fog: a low cold sun rising behind the EASTERN quarry ridge, mist
    /// filling the valley floor and the river while the ridgelines cut through — the strongest
    /// height falloff of the set, still under the 400 m fairness bound at every fighting height.
    pub fn bystra_dawn_fog() -> Self {
        Self {
            ambient_rgb: [0.20, 0.22, 0.27],
            ground_ambient_rgb: [0.13, 0.13, 0.14],
            key_direction: [0.80, 0.18, -0.12],
            key_rgb: [0.92, 0.82, 0.70],
            fill_direction: [-0.55, 0.55, 0.25],
            fill_rgb: [0.15, 0.17, 0.22],
            rim_direction: [-0.75, 0.30, 0.30],
            rim_rgb: [0.24, 0.24, 0.28],
            sky_zenith_rgb: [0.36, 0.42, 0.55],
            sky_horizon_rgb: [0.72, 0.68, 0.66],
            // Rebalanced for the two-layer model: the valley haze carries the floor
            // mist now; base + valley at height 0 stays under the 400 m fairness bound.
            fog_density: 0.0008,
            fog_height_falloff: 0.10,
            // Dawn mist: muted colour in the fog, a little extra exposure so the low sun still
            // carries through it.
            exposure: 1.05,
            black_point: 0.02,
            saturation: 1.10,
            contrast: 1.10,
            // Dawn: thin, fine high sheets rather than banks; the strongest sun scatter of the
            // set — the whole eastern mist glows toward the low sun.
            cloud_coverage_bias: -0.05,
            cloud_scale: 1.6,
            cloud_opacity: 0.55,
            cloud_drift: 0.003,
            cloud_shadow_strength: 0.1,
            fog_sun_scatter: 0.8,
            // Dawn mist: a milky low sun carrying through the fog.
            sun_softness: 0.56,
            valley_haze_density: 0.00022,
            valley_haze_height_m: 12.0,
            god_ray_strength: 0.12,
            cloud_sheet_opacity: 0.45,
            cloud_sheet_scale: 2.0,
            storm_front_dir_rad: 0.0,
            storm_front_strength: 0.0,
            bloom_weight: 0.06,
            vignette: 0.08,
            local_lights: NO_LOCAL_LIGHTS,
        }
    }

    /// Prokhorovka, golden evening: the sun low in the WEST, raking long shadows across the Psel
    /// killzone — the look the shadow cascades were built to sell. A hot amber key, dusk-blue
    /// ambient in the shade, a deep-blue zenith easing into a warm band at the horizon, and the
    /// strongest sun scatter of the outdoor set so the whole western haze glows.
    pub fn prokhorovka_golden_evening() -> Self {
        Self {
            ambient_rgb: [0.13, 0.15, 0.24],
            ground_ambient_rgb: [0.15, 0.11, 0.08],
            // Low in the west: the normalized elevation sits ~0.25 — long shadows, real raking.
            key_direction: [-0.92, 0.25, 0.20],
            key_rgb: [1.32, 0.95, 0.55],
            fill_direction: [0.60, 0.55, -0.30],
            fill_rgb: [0.13, 0.15, 0.22],
            rim_direction: [0.50, 0.35, 0.80],
            rim_rgb: [0.24, 0.20, 0.20],
            sky_zenith_rgb: [0.15, 0.23, 0.46],
            sky_horizon_rgb: [0.86, 0.66, 0.46],
            fog_density: 0.00018,
            fog_height_falloff: 0.02,
            exposure: 1.1,
            black_point: 0.035,
            saturation: 1.25,
            contrast: 1.15,
            cloud_coverage_bias: 0.02,
            cloud_scale: 1.0,
            cloud_opacity: 0.9,
            cloud_drift: 0.004,
            cloud_shadow_strength: 0.3,
            fog_sun_scatter: 0.85,
            sun_softness: 0.13,
            valley_haze_density: 0.0,
            valley_haze_height_m: 0.0,
            god_ray_strength: 0.15,
            cloud_sheet_opacity: 0.25,
            cloud_sheet_scale: 1.3,
            storm_front_dir_rad: 0.0,
            storm_front_strength: 0.0,
            bloom_weight: 0.06,
            vignette: 0.1,
            local_lights: NO_LOCAL_LIGHTS,
        }
    }

    /// Prokhorovka, dry overcast: a lead lid over the steppe — flat cool light, no sun disc worth
    /// the name, soft contrast — but DRY, unlike the Bystra squalls: no rain pass, no soaked
    /// world. The mood day for a long-range gunnery duel.
    pub fn prokhorovka_overcast() -> Self {
        Self {
            ambient_rgb: [0.24, 0.26, 0.29],
            ground_ambient_rgb: [0.13, 0.13, 0.13],
            key_direction: [0.20, 0.90, 0.15],
            key_rgb: [0.50, 0.53, 0.58],
            fill_direction: [-0.45, 0.50, -0.55],
            fill_rgb: [0.14, 0.16, 0.19],
            rim_direction: [-0.30, 0.45, 0.80],
            rim_rgb: [0.16, 0.18, 0.22],
            sky_zenith_rgb: [0.34, 0.37, 0.42],
            sky_horizon_rgb: [0.52, 0.55, 0.58],
            fog_density: 0.0005,
            fog_height_falloff: 0.01,
            exposure: 1.0,
            black_point: 0.015,
            saturation: 1.05,
            contrast: 1.08,
            cloud_coverage_bias: 0.4,
            cloud_scale: 1.2,
            cloud_opacity: 0.95,
            cloud_drift: 0.005,
            cloud_shadow_strength: 0.0,
            fog_sun_scatter: 0.1,
            sun_softness: 0.35,
            valley_haze_density: 0.0,
            valley_haze_height_m: 0.0,
            god_ray_strength: 0.0,
            cloud_sheet_opacity: 0.5,
            cloud_sheet_scale: 1.1,
            storm_front_dir_rad: 0.0,
            storm_front_strength: 0.0,
            bloom_weight: 0.04,
            vignette: 0.06,
            local_lights: NO_LOCAL_LIGHTS,
        }
    }

    /// The garage studio: a soft warm key from front-left-above, a weak cool fill from the right,
    /// and a restrained rear rim to lift the silhouette, on a near-neutral sky/floor ambient so the
    /// vehicle's own material colour reads true. The result is a neutral tint with shaped studio
    /// light.
    pub fn garage_studio() -> Self {
        Self {
            ambient_rgb: [0.30, 0.30, 0.33],
            ground_ambient_rgb: [0.16, 0.16, 0.17],
            key_direction: [-0.55, 0.72, 0.45],
            key_rgb: [0.98, 0.90, 0.74],
            fill_direction: [0.95, 0.25, 0.10],
            fill_rgb: [0.20, 0.24, 0.30],
            rim_direction: [0.15, 0.55, -0.95],
            rim_rgb: [0.26, 0.26, 0.30],
            // Interior: a neutral studio backdrop, no aerial perspective (the hangar clear colour
            // still overrides the visible background; these keep the uniform well-formed).
            sky_zenith_rgb: [0.14, 0.15, 0.17],
            sky_horizon_rgb: [0.18, 0.19, 0.21],
            fog_density: 0.0,
            fog_height_falloff: 0.0,
            // Studio: near-neutral grade — the vehicle's own material colour reads true.
            exposure: 1.0,
            black_point: 0.02,
            saturation: 1.10,
            contrast: 1.08,
            // Interior: no sky layer, no cloud shade, no scatter.
            cloud_coverage_bias: 0.0,
            cloud_scale: 1.0,
            cloud_opacity: 0.0,
            cloud_drift: 0.0,
            cloud_shadow_strength: 0.0,
            fog_sun_scatter: 0.0,
            sun_softness: 0.0,
            valley_haze_density: 0.0,
            valley_haze_height_m: 0.0,
            god_ray_strength: 0.0,
            cloud_sheet_opacity: 0.0,
            cloud_sheet_scale: 0.0,
            storm_front_dir_rad: 0.0,
            storm_front_strength: 0.0,
            bloom_weight: 0.02,
            vignette: 0.0,
            local_lights: NO_LOCAL_LIGHTS,
        }
    }

    /// The workshop look: a hard warm key raking down nearly vertically as if pouring through the
    /// roof skylights (so the tank throws a real contact shadow on the turntable and its decks read
    /// bright against the shaded flanks), a cool weak fill from the open bay door, a low neutral
    /// ambient so the shadowed sides stay moody rather than flooded, and a cold rear rim to peel the
    /// silhouette off the dim back wall.
    pub fn garage_workshop() -> Self {
        Self {
            ambient_rgb: [0.19, 0.20, 0.23],
            ground_ambient_rgb: [0.12, 0.11, 0.10],
            // Steep and slightly to the front-left: the skylight strips run overhead, so the key
            // comes down the roof rather than in from the side.
            key_direction: [-0.28, 0.92, 0.26],
            key_rgb: [1.15, 1.02, 0.80],
            // Cool daylight leaking in the back doorway, opposing the warm key.
            fill_direction: [0.35, 0.30, -0.90],
            fill_rgb: [0.20, 0.25, 0.34],
            rim_direction: [0.10, 0.45, -0.98],
            rim_rgb: [0.30, 0.33, 0.40],
            // Workshop interior: a dim cool backdrop, no aerial perspective.
            sky_zenith_rgb: [0.10, 0.11, 0.13],
            sky_horizon_rgb: [0.15, 0.16, 0.19],
            fog_density: 0.0,
            fog_height_falloff: 0.0,
            // Workshop: moodier than the studio — deeper blacks under the skylight key.
            exposure: 1.05,
            black_point: 0.035,
            saturation: 1.10,
            contrast: 1.12,
            // Interior: no sky layer, no cloud shade, no scatter.
            cloud_coverage_bias: 0.0,
            cloud_scale: 1.0,
            cloud_opacity: 0.0,
            cloud_drift: 0.0,
            cloud_shadow_strength: 0.0,
            fog_sun_scatter: 0.0,
            sun_softness: 0.0,
            valley_haze_density: 0.0,
            valley_haze_height_m: 0.0,
            god_ray_strength: 0.0,
            cloud_sheet_opacity: 0.0,
            cloud_sheet_scale: 0.0,
            storm_front_dir_rad: 0.0,
            storm_front_strength: 0.0,
            bloom_weight: 0.03,
            vignette: 0.06,
            local_lights: NO_LOCAL_LIGHTS,
        }
    }

    /// The garage HERO look: the workshop mood kept on the room, but the parked vehicle lit so it
    /// reads as the subject instead of a silhouette. The plain `garage_workshop` starved the flanks
    /// the orbit camera sees — a near-vertical key hits only the decks and the fill was aimed out the
    /// back door, so the camera-facing sides fell to ambient-only near-black. Three moves fix that
    /// without touching the battle look (the garage is a wholly separate lighting branch): a brighter
    /// (but still sub-studio, cool) hemispheric ambient so the shaded sides clear black, a key raked
    /// off vertical so it rakes the vertical flanks, and a near-horizontal fill thrown across the
    /// turntable from the opposite side of the key to lift the shadowed flank. The upper walls stay
    /// dark by their own low albedo, so the skylight contrast survives.
    pub fn garage_hero() -> Self {
        Self {
            // Near-neutral, faintly warm — the old blue-leaning ambient painted the gunmetal
            // walls navy; the workshop is lit by daylight and worklights, not moonlight.
            // Trimmed a step below the facelift value: the local pools carry the warmth now,
            // and pools only read as POOLS against a quieter base.
            ambient_rgb: [0.26, 0.255, 0.25],
            ground_ambient_rgb: [0.15, 0.14, 0.13],
            // Steep enough to still read as skylight-through-the-roof, but with real horizontal reach
            // so `dot(n, key)` is meaningful on the vertical flanks, not only the decks.
            key_direction: [-0.45, 0.78, 0.42],
            key_rgb: [1.10, 1.00, 0.82],
            // The lever: a near-horizontal fill from the side opposite the key (studio motif), aimed
            // across the turntable at the camera-facing flank the key leaves in shadow.
            fill_direction: [0.85, 0.30, 0.35],
            fill_rgb: [0.26, 0.27, 0.30],
            // A cool rear rim to peel the hull off the dim back wall.
            rim_direction: [0.10, 0.45, -0.98],
            rim_rgb: [0.30, 0.33, 0.40],
            sky_zenith_rgb: [0.12, 0.125, 0.14],
            sky_horizon_rgb: [0.17, 0.175, 0.20],
            fog_density: 0.0,
            fog_height_falloff: 0.0,
            // Hero shot: the grade must SERVE the phase-1a relight, not undo it — a hot black
            // point re-sank the flanks that the front fill exists to lift (the hero read as the
            // darkest thing in the frame again). So: a bright showroom exposure, near-neutral
            // blacks, gentle contrast; the moody-workshop grade stays on garage_workshop.
            exposure: 1.18,
            black_point: 0.012,
            saturation: 1.12,
            contrast: 1.05,
            cloud_coverage_bias: 0.0,
            cloud_scale: 1.0,
            cloud_opacity: 0.0,
            cloud_drift: 0.0,
            cloud_shadow_strength: 0.0,
            fog_sun_scatter: 0.0,
            sun_softness: 0.0,
            // The worklight rig. Positions coincide with the lamp housings the hangar mesh
            // hangs (`hangar_gallery::push_high_bay_lamps`) — the light pools where the lamp
            // is, or the room reads as haunted. Two warm high-bays over the turntable, a
            // strip over the workbench, the cool glow of the frosted panes over the gate,
            // and a zone lamp each for the stores corner and the second bay.
            valley_haze_density: 0.0,
            valley_haze_height_m: 0.0,
            god_ray_strength: 0.0,
            cloud_sheet_opacity: 0.0,
            cloud_sheet_scale: 0.0,
            storm_front_dir_rad: 0.0,
            storm_front_strength: 0.0,
            // Raised from 0.03 with the emission boost (Hala 2.0 T1a.1): the composite is
            // `hdr + blurred * weight`, and 3% of a blurred pane was a halo of ~0.02 - nothing.
            // At 0.07 (under the 0.10 art-direction cap) a 3.5 HDR pane carries a real glow.
            bloom_weight: 0.07,
            vignette: 0.05,
            local_lights: [
                LocalLight {
                    position: [-3.6, 9.8, 1.8],
                    radius_m: 11.0,
                    rgb: [1.0, 0.88, 0.70],
                    intensity: 1.5,
                },
                LocalLight {
                    position: [3.6, 9.8, -1.8],
                    radius_m: 11.0,
                    rgb: [1.0, 0.88, 0.70],
                    intensity: 1.3,
                },
                LocalLight {
                    position: [16.0, 3.4, 6.0],
                    radius_m: 6.0,
                    rgb: [1.0, 0.95, 0.85],
                    intensity: 1.2,
                },
                LocalLight {
                    position: [0.0, 7.6, -17.0],
                    radius_m: 10.0,
                    rgb: [0.72, 0.82, 1.0],
                    intensity: 0.8,
                },
                LocalLight {
                    position: [-14.5, 6.2, 10.0],
                    radius_m: 7.0,
                    rgb: [1.0, 0.90, 0.75],
                    intensity: 1.0,
                },
                LocalLight {
                    position: [10.5, 8.6, 9.5],
                    radius_m: 8.0,
                    rgb: [1.0, 0.90, 0.75],
                    intensity: 1.0,
                },
            ],
        }
    }
}

impl Default for SceneLighting {
    fn default() -> Self {
        Self::battlefield_default()
    }
}
