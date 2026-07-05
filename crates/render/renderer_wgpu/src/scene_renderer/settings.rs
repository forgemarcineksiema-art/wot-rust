//! Scene-renderer setters: background/sky selection and the shadow/SSAO capability toggles. Split
//! out of `scene_renderer.rs` to keep that file within the reviewability budget.

use super::SceneRenderer;

impl SceneRenderer {
    /// Set the sky clear colour (RGB in 0–1). The garage uses a dim interior tone; the battle
    /// uses the default daylight blue.
    pub fn set_sky(&mut self, r: f64, g: f64, b: f64) {
        self.sky = wgpu::Color { r, g, b, a: 1.0 };
    }

    /// Use a flat interior backdrop instead of the outdoor gradient sky: sets the clear colour and
    /// turns off the gradient-sky pass. The garage hangar calls this so its dim interior tone shows
    /// rather than a daylight sky dome.
    pub fn set_interior_background(&mut self, r: f64, g: f64, b: f64) {
        self.set_sky(r, g, b);
        self.draw_sky = false;
    }

    /// Use the outdoor gradient sky: sets the clear colour (a fallback tone behind the sky pass) and
    /// turns the gradient-sky pass on. The counterpart of [`Self::set_interior_background`], used
    /// when switching back to the battlefield after a garage visit.
    pub fn set_outdoor_sky(&mut self, r: f64, g: f64, b: f64) {
        self.set_sky(r, g, b);
        self.draw_sky = true;
    }

    /// Enable or disable the sun shadow (the capability fallback disables it). Disabled = `strength`
    /// 0, which the shaders read as "always lit" while keeping every bind group valid.
    pub fn set_shadows_enabled(&mut self, enabled: bool) {
        self.shadow.strength = if enabled { 1.0 } else { 0.0 };
    }

    /// Enable or disable SSAO (the capability fallback disables it). Disabled = `strength` 0,
    /// which skips the prepass/AO passes and the shaders read as "fully open".
    pub fn set_ssao_enabled(&mut self, enabled: bool) {
        self.ssao.strength = if enabled { 1.0 } else { 0.0 };
    }
}
