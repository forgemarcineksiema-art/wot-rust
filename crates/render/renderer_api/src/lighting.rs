//! Calibrated scene-lighting profiles (hemispheric ambient + key/fill/rim, gradient sky and
//! aerial-perspective fog) shared by the scene and vehicle shaders. Backend-neutral data turned
//! into GPU bytes in exactly one place (`renderer_wgpu::CameraUniform::from_scene`). See
//! `docs/atmosphere-policy.md`.

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
}

impl SceneLighting {
    /// Aerial-perspective fog factor for a surface `distance` metres from the camera at world
    /// `height`: 0 = no fog (near/high), 1 = fully faded to `sky_horizon_rgb`. The CPU mirror of the
    /// shaders' `apply_fog`, so the model is testable without a GPU. Fog thickens with distance and
    /// thins with height; density 0 returns 0 everywhere.
    pub fn fog_factor(&self, distance: f32, height: f32) -> f32 {
        if self.fog_density <= 0.0 {
            return 0.0;
        }
        let height_term = (-height.max(0.0) * self.fog_height_falloff).exp();
        let f = 1.0 - (-distance.max(0.0) * self.fog_density * height_term).exp();
        f.clamp(0.0, 1.0)
    }

    /// The battlefield look: a warm sun key raking low from the side (so it sculpts the sides of a
    /// low hull, not just the decks), a cool sky fill and sky ambient from above, a warm ground
    /// bounce from below, and a live sky rim that lifts the silhouette off the horizon. Tuned to be
    /// read through the ACES-lite tone curve, so the key deliberately runs hot.
    pub fn battlefield_default() -> Self {
        Self {
            ambient_rgb: [0.20, 0.23, 0.29],
            ground_ambient_rgb: [0.15, 0.14, 0.11],
            key_direction: [0.62, 0.52, 0.34],
            key_rgb: [1.08, 0.98, 0.82],
            fill_direction: [-0.5, 0.62, -0.28],
            fill_rgb: [0.17, 0.20, 0.26],
            rim_direction: [-0.42, 0.4, -0.88],
            rim_rgb: [0.20, 0.23, 0.30],
            // A clear-day sky: a deeper blue overhead easing to a pale, slightly warm haze at the
            // horizon. The horizon doubles as the fog colour, so distant hills melt into the sky.
            sky_zenith_rgb: [0.19, 0.34, 0.58],
            sky_horizon_rgb: [0.66, 0.74, 0.82],
            // Very light haze, tuned so enemy vehicles stay crisply readable at combat range:
            // ~4% fade at 300 m, ~6% at 500 m, only ~10-15% out past 1 km where the far terrain
            // melts into the horizon. Aerial perspective for depth, never for hiding targets.
            fog_density: 0.00013,
            fog_height_falloff: 0.02,
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
        }
    }
}

impl Default for SceneLighting {
    fn default() -> Self {
        Self::battlefield_default()
    }
}
