//! Window-renderer background/lighting setters, delegating to the owned [`SceneRenderer`]. Split
//! out of `window_renderer.rs` to keep that file within the reviewability budget.

use super::WindowRenderer;

impl WindowRenderer {
    /// Set the clear/background color (e.g. a dim interior tone behind the garage hangar).
    pub fn set_sky(&mut self, r: f64, g: f64, b: f64) {
        self.scene.sky = wgpu::Color { r, g, b, a: 1.0 };
    }

    /// Use a flat interior backdrop instead of the outdoor gradient sky (garage hangar). Sets the
    /// clear colour and turns the gradient-sky pass off. See [`SceneRenderer::set_interior_background`].
    ///
    /// [`SceneRenderer::set_interior_background`]: crate::SceneRenderer::set_interior_background
    pub fn set_interior_background(&mut self, r: f64, g: f64, b: f64) {
        self.scene.set_interior_background(r, g, b);
    }

    /// Use the outdoor gradient sky (battlefield). Sets the fallback clear colour and turns the
    /// gradient-sky pass on. See [`SceneRenderer::set_outdoor_sky`].
    ///
    /// [`SceneRenderer::set_outdoor_sky`]: crate::SceneRenderer::set_outdoor_sky
    pub fn set_outdoor_sky(&mut self, r: f64, g: f64, b: f64) {
        self.scene.set_outdoor_sky(r, g, b);
    }

    /// Set the calibrated scene lighting (key/fill/rim + ambient). Battle uses the default profile;
    /// the garage swaps in [`renderer_api::SceneLighting::garage_studio`].
    pub fn set_scene_lighting(&mut self, lighting: renderer_api::SceneLighting) {
        self.scene.scene_lighting = lighting;
    }

    /// Pin the sun-shadow boxes to a world point, or `None` to restore the forward chase-camera
    /// heuristic. The garage pins the turntable — the orbit camera's "forward" swings a full
    /// circle, and an unpinned focus walks the shadow boxes off the parked hero.
    pub fn set_shadow_focus(&mut self, focus: Option<[f32; 3]>) {
        self.scene.shadow_focus = focus;
    }

    /// Size the near shadow box to the scene: `Some(half_size_m)` for an interior, `None` for the
    /// battlefield default. The shadow map's resolution never changes — this only stops a small
    /// room from being covered by a box built for a 1000 m field, which is the whole difference
    /// between 6.25 cm and 2.9 cm texels in the hangar.
    /// Declare that this scene fits entirely inside its own near shadow box, so the far cascade
    /// carries nothing and its draws are skipped. See [`SceneRenderer::shadow_cascades`].
    ///
    /// [`SceneRenderer::shadow_cascades`]: crate::SceneRenderer::shadow_cascades
    pub fn set_shadow_cascades(&mut self, cascades: Option<u32>) {
        self.scene.shadow_cascades = cascades;
    }

    pub fn set_shadow_focus_radius_m(&mut self, radius_m: Option<f32>) {
        self.scene.shadow_focus_radius_m = radius_m;
    }

    /// Set the bloom mip depth for the current scene (see [`SceneRenderer::set_bloom_mips`]);
    /// [`Self::default_bloom_mips`] is the tier value the battlefield restores.
    ///
    /// [`SceneRenderer::set_bloom_mips`]: crate::SceneRenderer::set_bloom_mips
    pub fn set_bloom_mips(&mut self, mips: u32) {
        self.scene.set_bloom_mips(mips);
    }

    pub fn default_bloom_mips(&self) -> u32 {
        self.scene.default_bloom_mips()
    }

    /// Set or clear the garage hero probe (see [`SceneRenderer::set_hero_probe`]).
    ///
    /// [`SceneRenderer::set_hero_probe`]: crate::SceneRenderer::set_hero_probe
    pub fn set_hero_probe(&mut self, probe: Option<[[f32; 3]; 6]>) {
        self.scene.set_hero_probe(probe);
    }

    /// Advance the presentation clock shaders animate with (water ripple, foliage sway, weather).
    /// Tick-domain by doctrine: pass interpolated-tick seconds (whole fixed ticks + the sub-tick
    /// render phase over the tick rate), never an accumulation of render-frame deltas — a jittery
    /// frame clock must not wobble world animation.
    pub fn set_scene_time_s(&mut self, time_s: f32) {
        self.scene.scene_time_s = time_s;
    }
}

impl super::WindowRenderer {
    /// Replace the water-surface mesh (see [`crate::SceneRenderer::set_water`]); an empty
    /// slice clears the slot. Scene-swap cadence, never per frame.
    pub fn set_water(&mut self, vertices: &[renderer_api::WaterVertex], indices: &[u32]) {
        self.scene.set_water(&self.ctx, vertices, indices);
    }
}

impl super::WindowRenderer {
    /// Rain streak density 0..1 from the weather look; 0 skips the rain pass entirely.
    pub fn set_rain_intensity(&mut self, intensity: f32) {
        self.scene.rain_intensity = intensity.clamp(0.0, 1.0);
    }
}

impl super::WindowRenderer {
    /// World wetness 0..1 from the weather look: darkens albedo, sharpens material finishes,
    /// pools sheen on level ground (scene and vehicle shaders alike).
    pub fn set_wetness(&mut self, wetness: f32) {
        self.scene.wetness = wetness.clamp(0.0, 1.0);
    }

    pub fn set_weather_dynamics(
        &mut self,
        puddle_fill: f32,
        cloud_offset: [f32; 2],
        rain_phase_s: f32,
    ) {
        self.scene.puddle_fill = puddle_fill.clamp(0.0, 1.0);
        self.scene.cloud_offset = cloud_offset;
        self.scene.rain_phase_s = rain_phase_s;
    }
}
