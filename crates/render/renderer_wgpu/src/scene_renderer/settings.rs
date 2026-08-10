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

    /// Set the bloom ladder's mip depth for the CURRENT scene. The garage runs a modest chain
    /// (its emissive panes and lamp faces are the point of having one — "a bit richer", the
    /// 2026-08-04 decision); the battlefield passes its tier default back in. Scene-swap
    /// cadence, never per frame: switching drops the ladder's targets and the post pass's
    /// bind group so both rebuild against the new chain on the next render.
    pub fn set_bloom_mips(&mut self, mips: u32) {
        if self.bloom.mips == mips {
            return;
        }
        self.bloom.mips = mips;
        self.bloom.targets = None;
        self.post.bind_group = None;
    }

    /// The bloom depth this renderer was born with (the quality tier's value) — what the
    /// battlefield restores after the garage's richer chain.
    pub fn default_bloom_mips(&self) -> u32 {
        self.default_bloom_mips
    }

    /// Set or clear the garage hero probe (Hala 3.0 B2): the hall's irradiance cube at the
    /// station, added to every vehicle fragment by the ambient-cube blend. `None` zeroes the
    /// lanes, which the shader turns into a bit-exact no-op — the battlefield's state.
    /// Scene-swap cadence, like the interior background.
    pub fn set_hero_probe(&mut self, probe: Option<[[f32; 3]; 6]>) {
        self.hero_probe = match probe {
            Some(faces) => faces.map(|f| [f[0], f[1], f[2], 0.0]),
            None => [[0.0; 4]; 6],
        };
    }

    /// Enable or disable the interior detail normal (Hala 3.0 C1): on, the hangar's material
    /// grain bends the surface normal so paint tooth and float marks CATCH the worklight;
    /// off (every other scene), interiors keep their authored normals exactly as before —
    /// the per-scene flag that replaced the old fog-density heuristic in `scene.wgsl`.
    pub fn set_interior_detail_normal(&mut self, enabled: bool) {
        self.interior_detail_normal = enabled;
    }

    /// Field dust on the hero vehicle (Hala 3.0 J2): 0 clean .. 1 fresh off the battlefield.
    /// Up-facing surfaces take an earthy film and lose their sheen in `vehicle.wgsl`; at 0
    /// the math is a bit-exact no-op, which is what keeps every battle golden byte-stable.
    pub fn set_vehicle_dust(&mut self, amount: f32) {
        self.vehicle_dust = amount.clamp(0.0, 1.0);
    }

    /// Set or clear the interior reflection cubemap (Hala 3.0 D1): `Some(mips)` uploads the
    /// baked, prefiltered room cube (each mip: 6 faces of RGBA f32 texels, WebGPU face order)
    /// and points every glossy interior reflection at it; `None` restores the black default
    /// and the analytic `env_sky` path. Scene-swap cadence.
    pub fn set_environment_cube(
        &mut self,
        ctx: &crate::GpuContext,
        mips: Option<&[[Vec<[f32; 4]>; 6]]>,
    ) {
        self.environment_cube_set = mips.is_some();
        self.shadow.set_env_cube(&ctx.device, &ctx.queue, &self.shadow_bgl, mips);
    }
}
