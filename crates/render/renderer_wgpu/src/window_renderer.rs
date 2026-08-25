use renderer_api::{MeshAsset, MeshHandle, RenderError, RenderFrame, RenderSettings, SceneVertex};

use crate::msaa::{shipped_sample_count, validate_msaa_support};
use crate::offscreen::DEPTH_FORMAT;
use crate::select_present_mode;
use crate::{GpuContext, SceneRenderTarget, SceneRenderer};

mod settings;
mod vehicle;

/// The live windowed renderer: owns the GPU device, the presentation surface and the scene
/// renderer. Depth and the multisampled colour belong to the scene renderer now. The caller passes its window handle (e.g. an
/// `Arc<winit::window::Window>`) without ever naming a `wgpu` type.
pub struct WindowRenderer {
    ctx: GpuContext,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    scene: SceneRenderer,
}

impl WindowRenderer {
    pub fn new(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        terrain_vertices: &[SceneVertex],
        terrain_indices: &[u32],
    ) -> Result<Self, RenderError> {
        Self::new_with_settings(
            window,
            width,
            height,
            terrain_vertices,
            terrain_indices,
            RenderSettings::default(),
        )
    }

    pub fn new_with_settings(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        terrain_vertices: &[SceneVertex],
        terrain_indices: &[u32],
        settings: RenderSettings,
    ) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window)
            .map_err(|error| RenderError::new(format!("failed to create surface: {error}")))?;
        let ctx = GpuContext::new(instance, Some(&surface))?;

        let mut config = surface
            .get_default_config(&ctx.adapter, width.max(1), height.max(1))
            .ok_or_else(|| RenderError::new("surface unsupported by adapter".to_string()))?;
        let caps = surface.get_capabilities(&ctx.adapter);
        if let Some(srgb) = caps.formats.iter().copied().find(wgpu::TextureFormat::is_srgb) {
            config.format = srgb;
        }
        // `WOT_VSYNC=0` lets a sub-refresh machine trade FIFO's judder+latency for
        // present-as-ready; the settings default stays vsync.
        let vsync = std::env::var("WOT_VSYNC").map_or(settings.vsync, |value| value.trim() != "0");
        config.present_mode = select_present_mode(&caps.present_modes, vsync);
        // One frame in flight, not wgpu's default two: the second queued frame is a whole extra
        // frame of input latency — at a GPU-bound laptop's 25 FPS that is 40 ms more mush
        // between the stick and the screen, for no smoothness in return.
        config.desired_maximum_frame_latency = 1;
        // One-look policy: MSAA follows the same canonical/rich split as the lighting profile.
        // Resolved through the shared helper, not inline, so an offscreen instrument that claims
        // to measure this frame reaches the same number by construction.
        let sample_count = shipped_sample_count(settings.msaa_samples);
        validate_msaa_support(&ctx, config.format, DEPTH_FORMAT, sample_count)?;
        surface.configure(&ctx.device, &config);

        let scene = SceneRenderer::new_with_sample_count(
            &ctx,
            config.format,
            sample_count,
            terrain_vertices,
            terrain_indices,
        )?;
        Ok(Self { ctx, surface, config, scene })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.ctx.device, &self.config);
    }

    pub fn aspect_ratio(&self) -> f32 {
        self.config.width as f32 / self.config.height.max(1) as f32
    }

    pub fn set_dynamic_mesh(&mut self, vertices: &[SceneVertex], indices: &[u32]) {
        self.scene.set_dynamic_mesh(&self.ctx, vertices, indices);
    }

    pub fn register_mesh(&mut self, handle: MeshHandle, mesh: &MeshAsset) {
        self.scene.register_mesh(&self.ctx, handle, mesh);
    }

    pub fn set_render_frame(&mut self, frame: &RenderFrame) {
        self.scene.set_render_frame(&self.ctx, frame);
    }

    /// Swap the static scene geometry (battlefield <-> garage hangar). See
    /// [`SceneRenderer::set_terrain`]; only call on a scene change, not per frame.
    pub fn set_battlefield_ground(
        &mut self,
        vertices: &[SceneVertex],
        indices: &[u32],
        maps: &renderer_api::TerrainGroundMaps,
        materials: &renderer_api::TerrainMaterialSet,
    ) {
        self.scene.set_battlefield_ground(&self.ctx, vertices, indices, maps, materials);
    }

    /// Geometry-only ground swap (true deformation, protocol v31): craters re-mesh the
    /// heightfield; the baked splat/macro maps stay bound.
    pub fn update_battlefield_ground_geometry(
        &mut self,
        vertices: &[SceneVertex],
        indices: &[u32],
    ) {
        self.scene.update_battlefield_ground_geometry(&self.ctx, vertices, indices);
    }

    /// The dressing slot (Żywy Step P2): mid-field grass cards, color-pass-only; empty
    /// slices clear it (the garage has no meadow).
    pub fn set_dressing(&mut self, vertices: &[SceneVertex], indices: &[u32]) {
        self.scene.set_dressing(&self.ctx, vertices, indices);
    }

    pub fn clear_battlefield_ground(&mut self) {
        self.scene.clear_battlefield_ground();
    }

    /// Replace the foliage atlas (Imported Flora 2.0, FL-2) — see
    /// [`crate::SceneRenderer::set_foliage_atlas`].
    pub fn set_foliage_atlas(
        &mut self,
        chain: &renderer_api::Rgba8MipChain,
        normals: Option<&renderer_api::Rgba8MipChain>,
    ) {
        self.scene.set_foliage_atlas(&self.ctx, chain, normals);
    }

    pub fn set_terrain(&mut self, vertices: &[SceneVertex], indices: &[u32]) {
        self.scene.set_terrain(&self.ctx, vertices, indices);
    }

    pub fn set_hud(&mut self, vertices: &[renderer_api::HudVertex]) {
        self.scene.set_hud(&self.ctx, vertices);
    }

    /// Upload this frame's battle-FX quads (world-space, premultiplied colors); see
    /// [`SceneRenderer::set_fx`].
    pub fn set_fx(&mut self, vertices: &[renderer_api::FxVertex]) {
        self.scene.set_fx(&self.ctx, vertices);
    }

    /// Upload the HUD glyph atlas (single-channel R8 coverage, `width`*`height` bytes). Call once
    /// after construction so HUD text samples real glyphs instead of the 1x1 placeholder.
    pub fn set_hud_font_atlas(&mut self, width: u32, height: u32, coverage: &[u8]) {
        self.scene.set_hud_font_atlas(&self.ctx, width, height, coverage);
    }

    pub fn render(
        &mut self,
        view_proj: [[f32; 4]; 4],
        camera_pos: [f32; 3],
    ) -> Result<(), RenderError> {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            // The surface no longer matches the window (a resize is in flight): reconfigure and draw
            // the next frame against the fresh surface. Common and benign — no log.
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.ctx.device, &self.config);
                return Ok(());
            }
            // Minimized or fully behind another window: skip the frame. Frequent while minimized, so
            // it stays silent — logging here would spam once per frame.
            wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
            // The GPU did not hand back a frame in time — a transient hitch. Skip it, but leave a
            // low-severity trace so a persistent stall is at least visible in a debug log.
            wgpu::CurrentSurfaceTexture::Timeout => {
                tracing::debug!("surface acquire timed out — skipping this frame");
                return Ok(());
            }
            // The surface (and possibly the device) was lost — a driver reset, a Windows TDR, a GPU
            // or display change. Reconfiguring is wgpu's documented first response; the WARN is the
            // whole point of this arm — a lost device used to leave a silent black screen with no
            // trace at all (the declared `GpuErrorPolicy` promised a handler that did not exist).
            wgpu::CurrentSurfaceTexture::Lost => {
                tracing::warn!(
                    "surface lost — reconfiguring (driver reset / TDR / display change)"
                );
                self.surface.configure(&self.ctx.device, &self.config);
                return Ok(());
            }
            // A validation error, or any status a future wgpu adds: never swallow it into a silent
            // black frame. Surface it so the caller logs it instead of rendering nothing forever.
            status => return Err(RenderError::new(format!("surface unavailable: {status:?}"))),
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let target = SceneRenderTarget {
            output_view: &view,
            width: self.config.width,
            height: self.config.height,
        };
        self.scene.render(&self.ctx, target, view_proj, camera_pos)?;
        frame.present();
        Ok(())
    }
}
