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
}
