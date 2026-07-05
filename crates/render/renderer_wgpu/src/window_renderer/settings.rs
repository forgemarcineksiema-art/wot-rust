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
}
