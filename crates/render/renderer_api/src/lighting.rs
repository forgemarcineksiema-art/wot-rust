//! Calibrated scene-lighting profiles (hemispheric ambient + key/fill/rim, gradient sky and
//! aerial-perspective fog) shared by the scene and vehicle shaders. Backend-neutral data turned
//! into GPU bytes in exactly one place (`renderer_wgpu::CameraUniform::from_scene`). See
//! `docs/atmosphere-policy.md`.

/// How many local light slots ride the camera uniform. Fixed so the GPU layout is static; unused
/// slots are disabled by `radius_m == 0` and cost one uniform read.
pub const MAX_LOCAL_LIGHTS: usize = 6;

/// An unshadowed local fill pool — a worklamp over the turntable, a zone lamp over a bay.
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

    /// The colour distant air fades toward when looked at STRAIGHT toward the sun — the far end
    /// of the sun-directional scatter blend. The CPU mirror of the shaders' `sun_haze` term in
    /// `apply_fog` (lighting_common.wgsl), kept in exact lockstep so the haze colour is testable
    /// without a GPU. The 0.55/0.45 weights sum to one: scatter warms the air toward the key,
    /// it never ADDS energy — the sun-side haze may be warmer than the horizon, never brighter
    /// than the sky (locked in look_locks).
    pub fn fog_sun_haze_reference(&self) -> [f32; 3] {
        [
            self.sky_horizon_rgb[0] * 0.55 + self.key_rgb[0] * 0.45,
            self.sky_horizon_rgb[1] * 0.55 + self.key_rgb[1] * 0.45,
            self.sky_horizon_rgb[2] * 0.55 + self.key_rgb[2] * 0.45,
        ]
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
            sky_horizon_rgb: [0.55, 0.63, 0.74],
            // Light haze, tuned so enemy vehicles stay crisply readable at combat range (~8% fade
            // at 400 m on the floor — the 0.35 fairness bound has wide margin) while the far
            // field genuinely melts: ~17% at 1.5 km even on the 65-90 m backdrop hills. The
            // gentle height falloff is deliberate — the OLD 0.02 exempted the hills from the air
            // entirely (4-10% fog), which left the brightest, never-shadowed geometry in frame
            // floating full-lit above a milky band (D3's washed-out horizon).
            fog_density: 0.00022,
            fog_height_falloff: 0.008,
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
            // A full stop down from the old [0.78, 0.72, 0.62]: the horizon IS the colour every
            // distant surface fades toward, and at graded luma 0.77 it was ~3x brighter than the
            // lit grass it replaced — the user-verdict "białawy" far field. Same warm-grey hue,
            // graded luma ~0.69, still comfortably above the 0.60 bright-band floor.
            sky_horizon_rgb: [0.64, 0.60, 0.51],
            // Doubled density + gentler falloff: the air now reaches the valley's enclosing
            // hills (~23% at 1.5 km / 75 m, was ~5%) so the far ridge melts DOWN into the haze
            // instead of floating full-lit above it. 400 m floor fade 13% — fairness margin wide.
            fog_density: 0.0003,
            fog_height_falloff: 0.007,
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
            fog_sun_scatter: 0.45,
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
            // Slightly denser, much gentler falloff (0.02 -> 0.008): the evening haze reaches
            // the ridge line too, so the raking-light frames keep their depth planes at range.
            fog_density: 0.00025,
            fog_height_falloff: 0.008,
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
            // B1 moody-workshop grade: the ambient drops a real step — the hall's base tone
            // is HALF-LIGHT now, and what reads bright must be a source (shed key, the gate's
            // day, a lamp pool). The hero stays lit because the key and the pools stand on
            // it, not because the room glows.
            // Held a hair over garage_workshop's ambient — the anti-silhouette lock's floor:
            // moody is the ROOM's tone, not the subject's flanks going near-black again.
            // Światło służy czołgowi (user verdict 2026-08-10: "światła napierdalają w ten
            // czołg, a na ziemi kratownica z cieni" — the lights played the lead). The floor
            // measured a 4.8x brightness swing between sun stripe and bar shadow; the fix is
            // the RATIO, from both ends: ambient up ~1.24x (here) and the key down ~0.71x
            // (below), which prices the stripes toward ~2x — shade you stand in, not a
            // projector pattern. The moody floor still holds: what reads bright is a source.
            ambient_rgb: [0.26, 0.255, 0.248],
            ground_ambient_rgb: [0.14, 0.132, 0.126],
            // The sun through the middle shed's glazing (Hala 3.0 A1). Derivation, so the next
            // relight can redo it instead of dialling: the glazing plane of the sun shed spans
            // z 1.0..5.5 falling from 11.3 m to 9.0 m; a ray from the turntable deck (y 0.62
            // — the flush A2 plate plus the lock's 0.6 m step)
            // along (-0.6, 2.3, 1.0) crosses it at z ≈ 4.0, y ≈ 9.8, x ≈ −2.4 — inside the
            // opening, and the lock's whole 5×5 deck fan crosses between the mullion phases.
            // Steep enough to read as roof daylight, with real horizontal reach so
            // `dot(n, key)` is meaningful on the vertical flanks; the z-lean means the beam
            // rakes DOWN the nave, with the hero framing. Locked by
            // `the_workshop_sun_reaches_the_turntable_through_a_real_opening`.
            key_direction: [-0.233, 0.892, 0.388],
            // Trimmed with the ambient raise above (Światło służy czołgowi): the sun keeps
            // its warmth and its direction — only its SHOUT goes.
            key_rgb: [0.78, 0.71, 0.58],
            // Readable-light doctrine (Hala 2.0 T1 correction, user verdict 2026-08-05): every
            // light in the hall must have a visible source. The fill is the one survivor of the
            // old studio rig, demoted to what it can honestly claim to be — concrete bounce off
            // the sunlit floor — so it stays near-horizontal but a clear step under the key,
            // never a second sun (locked by `garage_hero_lifts_the_subject_off_the_workshop_silhouette`).
            fill_direction: [0.85, 0.30, 0.35],
            fill_rgb: [0.13, 0.14, 0.15],
            // The cool rear rim had no source in the room at all — the hull was peeled off the
            // back wall by a light from nowhere. Dead, deliberately: the GI bake and the lamp
            // pools carry the separation now.
            rim_direction: [0.10, 0.45, -0.98],
            rim_rgb: [0.0, 0.0, 0.0],
            // INDOORS THESE TWO ARE NOT A SKY — they are the room, and they are the only thing
            // any polished surface in it has to reflect. `env_sky` mixes horizon -> zenith by
            // the reflected ray's up fraction, so in the hangar `zenith` is what a horizontal
            // surface sees overhead and `horizon` is what a vertical one sees across the bay.
            //
            // They used to be a dim outdoor gradient (0.12 -> 0.17) carried over from the
            // studio preset under a comment saying they only existed to keep the uniform
            // well-formed. They are not decoration: the turntable deck, the rails and every
            // painted panel on the hero mirror them. The hall's roof openings show
            // `hangar::INTERIOR_BACKGROUND` — 1.30/1.38/1.55 — so the old numbers had the tank
            // reflecting a sky SEVEN TO TEN TIMES darker than the daylight visible above it,
            // and had the gradient upside down: dimmer overhead than sideways, which is true
            // outdoors and false under a glazed roof.
            //
            // Both are derived from the room rather than dialled. Overhead: the shed glazing
            // bands open 30.7% of the roof plane (`hangar::skylight_open_fraction`, computed
            // from the tooth geometry), so 0.307 x the daylight behind them, less the mullions,
            // trusses and crane girder that hang under it. Across the bay: the gunmetal wall's
            // own albedo under the rig, near-neutral because the wall is. Locked to the
            // geometry by `the_rooms_reflection_is_the_room` — move a glazing band and the
            // number must follow it.
            sky_zenith_rgb: [0.32, 0.34, 0.37],
            // C2 raised this with the whitewash: what a vertical surface sees across the bay
            // is lime-washed wall below the seam and dark sheet above, so the horizon term
            // follows the blend upward — still a wall being seen, not a light (held under the
            // whitewash albedo AND under zenith/1.5 by `the_rooms_reflection_is_the_room`).
            // Trimmed with the wash itself when the first, fresher tone lost the hero to the
            // room (HERO_OVER_ROOM 1.68x vs the 2.0x floor).
            sky_horizon_rgb: [0.19, 0.192, 0.198],
            fog_density: 0.0,
            fog_height_falloff: 0.0,
            // Hero shot: the grade must SERVE the phase-1a relight, not undo it — a hot black
            // point re-sank the flanks that the front fill exists to lift (the hero read as the
            // darkest thing in the frame again). So: a bright showroom exposure, near-neutral
            // blacks, gentle contrast; the moody-workshop grade stays on garage_workshop.
            // B1: the showroom grade retires. Exposure back to neutral, a real black point so
            // the shadowed nave READS as shadow — the moody-workshop identity the plan names.
            // The readability that exposure used to buy is bought honestly now: the hero's
            // key, the gate beam and the lamp pools carry the subject, and the p05 floor in
            // the golden harness guards the cheap-panel readability the old exposure hid.
            // Tuned against the frame gate, two steps back from the first candidate:
            // 1.02/0.028 measured dark 0.830 (ceiling 0.81) and crushed the frame p05 to
            // 0.009; 1.06/0.021 still read 0.815. The ceiling is not this branch's to move —
            // this is as moody as the room may run on ambient alone. The mood DEEPENS in B2,
            // when the denser GI bake fills the shadow side with bounced light instead of
            // this grade fighting the readability floor for it.
            exposure: 1.10,
            black_point: 0.020,
            saturation: 1.12,
            contrast: 1.06,
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
            // strip over the workbench, and a zone lamp each for the stores corner and the
            // second bay. Every pool is WARM lamp light: daylight enters this room through
            // the skylights only (the glowing "frosted panes" are gone — a flat HDR slab is
            // not a window, per the 2026-08-05 verdict).
            valley_haze_density: 0.0,
            valley_haze_height_m: 0.0,
            god_ray_strength: 0.0,
            cloud_sheet_opacity: 0.0,
            cloud_sheet_scale: 0.0,
            storm_front_dir_rad: 0.0,
            storm_front_strength: 0.0,
            // Trimmed back from 0.07 with the panes gone: 0.07 was sized for a 3.5 HDR pane
            // bank, and on the remaining lamp faces it smeared. At 0.04 a worklamp face keeps
            // a modest halo and nothing in the hall reads as fog.
            bloom_weight: 0.04,
            vignette: 0.05,
            local_lights: [
                LocalLight {
                    position: [-3.6, 7.6, 1.8],
                    radius_m: 11.0,
                    rgb: [1.0, 0.88, 0.70],
                    intensity: 1.5,
                },
                LocalLight {
                    position: [3.6, 7.6, -1.8],
                    radius_m: 11.0,
                    rgb: [1.0, 0.88, 0.70],
                    intensity: 1.3,
                },
                LocalLight {
                    position: [9.2, 3.4, 6.0],
                    radius_m: 6.0,
                    rgb: [1.0, 0.95, 0.85],
                    intensity: 1.2,
                },
                // B1: the second beam the plan names — the DAY standing in the ajar gate.
                // The one cool pool in the hall, and the one allowed to be: its source is
                // visible (the open gap under the gate slats, INTERIOR_BACKGROUND behind it),
                // so this is not the "pane glow" that died in 2026-08-05 — it is daylight
                // with an address. Radius reaches the drive lane's first metres, so the
                // wedge on the floor and the light that made it agree.
                LocalLight {
                    position: [0.0, 1.0, -21.0],
                    radius_m: 13.0,
                    rgb: [0.72, 0.80, 0.95],
                    intensity: 1.15,
                },
                LocalLight {
                    position: [-8.5, 6.2, 17.0],
                    radius_m: 7.0,
                    rgb: [1.0, 0.90, 0.75],
                    intensity: 1.0,
                },
                LocalLight {
                    position: [-5.5, 7.4, 14.5],
                    radius_m: 8.0,
                    rgb: [1.0, 0.90, 0.75],
                    intensity: 1.0,
                },
            ],
        }
    }
}

impl SceneLighting {
    /// The hero rig at a moment in time (Hala 3.0 E2): `garage_hero` with the workbench's
    /// tube light FLICKERING — an old fluorescent that mostly holds and briefly buzzes when
    /// its oscillators conspire. Deterministic and tick-domain, so the live garage evaluates
    /// it per presented frame while the golden harness freezes it: at the review clock the
    /// factor is exactly 1.0 and this function IS `garage_hero()`, to the bit.
    pub fn garage_hero_at(seconds: f32) -> Self {
        let mut rig = Self::garage_hero();
        rig.local_lights[2].intensity *= fluorescent_flicker(seconds);
        rig
    }
}

/// Deterministic tube-flicker factor in `[0.35, 1.0]`: healthy most of the time, with brief
/// rapid-buzz dips when three slow oscillators peak together. No RNG — the same input second
/// always flickers the same way, which is what lets a review frame freeze it.
pub fn fluorescent_flicker(seconds: f32) -> f32 {
    let gate = (seconds * 0.83).sin() + (seconds * 1.97 + 1.3).sin() + (seconds * 0.31 + 4.0).sin();
    if gate > 2.35 { 0.35 + 0.25 * (seconds * 41.0).sin().abs() } else { 1.0 }
}

impl Default for SceneLighting {
    fn default() -> Self {
        Self::battlefield_default()
    }
}
