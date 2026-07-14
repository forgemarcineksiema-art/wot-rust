use renderer_api::{MeshAsset, MeshHandle, RenderError, RenderFrame, RenderSettings, SceneVertex};

use crate::msaa::{resolve_msaa_samples, validate_msaa_support};
use crate::offscreen::DEPTH_FORMAT;
use crate::select_present_mode;
use crate::{GpuContext, SceneRenderTarget, SceneRenderer};

mod settings;
mod vehicle;

/// The live windowed renderer: owns the GPU device, the presentation surface, a depth
/// buffer, and the scene renderer. The caller passes its window handle (e.g. an
/// `Arc<winit::window::Window>`) without ever naming a `wgpu` type.
pub struct WindowRenderer {
    ctx: GpuContext,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    sample_count: u32,
    msaa_color_view: Option<wgpu::TextureView>,
    depth_view: wgpu::TextureView,
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
        // F3: MSAA keys on the same effective class as the lighting tier — an entry-class
        // discrete chip (GeForce MX) folds to the integrated diet here too.
        let adapter_info = ctx.adapter.get_info();
        let sample_count = resolve_msaa_samples(
            settings.msaa_samples,
            crate::scene_renderer::quality::effective_device_class(
                adapter_info.device_type,
                &adapter_info.name,
                std::env::var("WOT_QUALITY").ok().as_deref(),
            ),
            std::env::var("WOT_MSAA").ok().as_deref(),
        );
        validate_msaa_support(&ctx, config.format, DEPTH_FORMAT, sample_count)?;
        surface.configure(&ctx.device, &config);

        let msaa_color_view =
            create_msaa_color_view(&ctx, config.format, config.width, config.height, sample_count);
        let depth_view = create_depth_view(&ctx, config.width, config.height, sample_count);
        let scene = SceneRenderer::new_with_sample_count(
            &ctx,
            config.format,
            sample_count,
            terrain_vertices,
            terrain_indices,
        )?;
        Ok(Self { ctx, surface, config, sample_count, msaa_color_view, depth_view, scene })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.ctx.device, &self.config);
        self.msaa_color_view =
            create_msaa_color_view(&self.ctx, self.config.format, width, height, self.sample_count);
        self.depth_view = create_depth_view(&self.ctx, width, height, self.sample_count);
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

    pub fn clear_battlefield_ground(&mut self) {
        self.scene.clear_battlefield_ground();
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
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.ctx.device, &self.config);
                return Ok(());
            }
            _ => return Ok(()),
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let target = SceneRenderTarget {
            color_view: self.msaa_color_view.as_ref().unwrap_or(&view),
            resolve_target: self.msaa_color_view.as_ref().map(|_| &view),
            depth_view: &self.depth_view,
            sample_count: self.sample_count,
            width: self.config.width,
            height: self.config.height,
        };
        self.scene.render(&self.ctx, target, view_proj, camera_pos)?;
        frame.present();
        Ok(())
    }
}

fn create_msaa_color_view(
    ctx: &GpuContext,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    sample_count: u32,
) -> Option<wgpu::TextureView> {
    if sample_count == 1 {
        return None;
    }

    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("window_msaa_color"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    Some(texture.create_view(&wgpu::TextureViewDescriptor::default()))
}

fn create_depth_view(
    ctx: &GpuContext,
    width: u32,
    height: u32,
    sample_count: u32,
) -> wgpu::TextureView {
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("window_depth"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
