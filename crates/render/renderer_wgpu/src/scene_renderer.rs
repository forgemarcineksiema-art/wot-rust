pub(crate) mod armor_damage;
mod bloom;
mod buffers;
mod cloud_map;
mod draw;
mod draw_depth;
mod dressing;
pub(crate) mod env_group;
mod foliage_atlas;
pub(crate) mod ground;
mod hud_atlas;
pub(crate) mod post;
pub(crate) mod quality;
mod resources;
mod settings;
pub(crate) mod shadow;
pub(crate) mod ssao;
mod ssao_pipelines;
mod terrain;
mod vehicle_materials;
mod water;

use std::cell::Cell;

use renderer_api::{RenderError, SceneLighting, SceneVertex};

use crate::msaa::{review_sample_count, shipped_sample_count, validate_msaa_support};
use crate::offscreen::DEPTH_FORMAT;
use crate::scene_pipeline::{build_hud_pipeline, build_scene_pipeline};
use crate::scene_resources::{SceneMeshRegistry, SceneObjectDraw};
use crate::{CameraUniform, GpuContext, VehicleMeshRegistry, build_vehicle_pipeline};

/// Dynamic-mesh budget (8 MiB each): the offscreen QA examples bake whole blueprint tanks
/// into this path at ~66k vertices (48 B each) per tank — the old 1 MiB silently dropped
/// any frame holding more than a third of one tank, rendering an empty meadow.
const DYNAMIC_VERTEX_CAPACITY: u64 = 1 << 23;
const DYNAMIC_INDEX_CAPACITY: u64 = 1 << 23;
/// HUD overlay budget: 16384 vertices (32 B each). Sized for the full battle HUD — reticle,
/// bars, readouts (each glyph now carries a drop-shadow quad), damage log, ammo panel and the
/// 36x36-cell minimap — with headroom; `set_hud` truncates (with a warning) instead of
/// blanking the frame if a regression ever exceeds it.
const HUD_VERTEX_CAPACITY: u64 = 1 << 19;
/// Battle-FX vertex budget: ~2048 soft quads (6 verts x 36 bytes) with headroom.
const FX_VERTEX_CAPACITY: u64 = 1 << 19;
pub use armor_damage::armor_damage_aperture_budget;
pub use buffers::{VEHICLE_INSTANCE_CAPACITY, vehicle_instance_budget};

pub struct SceneRenderer {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    armor_damage: armor_damage::ArmorDamageBuffers,
    terrain_vertices: wgpu::Buffer,
    terrain_indices: wgpu::Buffer,
    terrain_index_count: u32,
    /// Spatial draw ranges of the terrain slot (see `terrain.rs`): every pass draws only the
    /// chunks its frustum sees.
    terrain_chunks: Vec<renderer_api::SceneChunk>,
    /// The dressing slot (Żywy Step P2): mid-field grass cards, color-pass-only (see
    /// `dressing.rs`). Empty in the garage and on maps without a baked meadow.
    dressing_vertices: wgpu::Buffer,
    dressing_indices: wgpu::Buffer,
    dressing_index_count: u32,
    dressing_chunks: Vec<renderer_api::SceneChunk>,
    dynamic_vertices: wgpu::Buffer,
    dynamic_indices: wgpu::Buffer,
    dynamic_index_count: u32,
    identity_instance: wgpu::Buffer,
    frame_instances: wgpu::Buffer,
    frame_instance_count: u32,
    frame_draws: Vec<SceneObjectDraw>,
    static_meshes: SceneMeshRegistry,
    vehicle_pipeline: wgpu::RenderPipeline,
    vehicle_materials: vehicle_materials::VehicleMaterialRegistry,
    vehicle_instances: wgpu::Buffer,
    vehicle_instance_count: u32,
    vehicle_draws: Vec<SceneObjectDraw>,
    /// Where each tank stands this frame (Jedna Trawa P9), read off the vehicle frame — the
    /// meadow is pressed down by the nearest of these.
    grass_crushers: Vec<(renderer_api::VehicleId, [f32; 3])>,
    vehicle_meshes: VehicleMeshRegistry,
    sky_pipeline: wgpu::RenderPipeline,
    rain_pipeline: wgpu::RenderPipeline,
    /// Rain streak density 0..1 from the weather look; 0 skips the rain pass entirely.
    pub rain_intensity: f32,
    /// World wetness 0..1 from the weather look (time_params.z).
    pub wetness: f32,
    /// Dynamic match-weather lanes separate from material wetness.
    pub puddle_fill: f32,
    pub cloud_offset: [f32; 2],
    pub rain_phase_s: f32,
    water_pipeline: wgpu::RenderPipeline,
    /// The discrete-tier refraction pipeline + grab resources; unused (bind group stays `None`)
    /// when refraction is off or the frame is single-sampled.
    water_refraction: crate::water_pipeline::WaterRefraction,
    /// Whether this adapter/tier refracts the water (see `LightingQuality::refraction`). The draw
    /// only takes the two-pass grab path when this AND `sample_count > 1` hold.
    refraction: bool,
    /// The static river mesh (vertex, index); `None` on dry scenes — the draw is skipped.
    water_buffers: Option<(wgpu::Buffer, wgpu::Buffer)>,
    water_index_count: u32,
    fx_pipeline: wgpu::RenderPipeline,
    fx_vertices: wgpu::Buffer,
    fx_vertex_count: u32,
    hud_pipeline: wgpu::RenderPipeline,
    hud_vertices: wgpu::Buffer,
    hud_vertex_count: u32,
    hud_font_bgl: wgpu::BindGroupLayout,
    hud_font_sampler: wgpu::Sampler,
    hud_font_bind_group: wgpu::BindGroup,
    sample_count: u32,
    pub sky: wgpu::Color,
    /// Draw the gradient-sky background pass (the outdoor battle look). Interior scenes (the garage
    /// hangar) turn it off via [`Self::set_interior_background`] so their own backdrop shows instead.
    pub draw_sky: bool,
    /// The calibrated three-point lighting for this scene. Battle uses the default profile; the
    /// garage swaps in a warm studio key/fill/rim. Drives both the scene and the vehicle shaders.
    pub scene_lighting: SceneLighting,
    shadow: shadow::ShadowResources,
    shadow_bgl: wgpu::BindGroupLayout,
    /// The foliage atlas + sampler at group 1 of the scene/shadow/prepass pipelines (FL-2).
    foliage_atlas: foliage_atlas::FoliageAtlas,
    foliage_bgl: wgpu::BindGroupLayout,
    ssao: ssao::SsaoResources,
    /// The Terrain Material 2.0 ground slot (battlefield heightfield + splat/macro maps).
    ground: ground::GroundResources,
    /// The HDR image-formation chain: the internal Rgba16Float target the world renders into
    /// and the fullscreen display-transform pass that forms the final picture (rule 7).
    post: post::PostResources,
    /// The FXAA pass over the formed LDR picture — the shipped game's ONE anti-aliasing
    /// (canonical runs 1x MSAA on every adapter); the HUD draws after it, never softened.
    fxaa: post::FxaaResources,
    /// The dual-Kawase bloom ladder feeding the post pass (rule 6); depth from the quality tier.
    bloom: bloom::BloomResources,
    /// Whether this adapter tier runs terrain cloud shadows (`LightingQuality::cloud_shadows`).
    cloud_shadows_enabled: bool,
    /// Whether this adapter tier runs full per-pixel shader detail (F2).
    shader_detail: renderer_api::ShaderDetailMask,
    /// World point the focused sun-shadow box centres on (the player/subject). `None` falls back to
    /// the camera position, which still covers the near action.
    pub shadow_focus: Option<[f32; 3]>,
    /// The scene's near-shadow-box half-size in metres; `None` takes the battlefield default
    /// (`SunShadowParams::default().focus_radius_m`). An interior sets its own — see
    /// `ShadowResources::cascades`.
    pub shadow_focus_radius_m: Option<f32>,
    /// The presentation clock shaders animate with (`Camera.time_params.x`). Tick-domain by
    /// doctrine: the caller feeds interpolated-tick seconds, never an accumulation of
    /// render-frame deltas (see `CameraUniform::time_params`).
    pub scene_time_s: f32,
    pub skipped_mesh_draws: Cell<u32>,
    /// The quality tier's bloom depth, kept so the battlefield can restore it after the
    /// garage's richer chain (`set_bloom_mips`).
    default_bloom_mips: u32,
    /// Per-pass GPU timing. `Disabled` unless a probe arms it, and while disabled the encoder
    /// emits exactly the command stream it emitted before this field existed.
    profiler: crate::frame_profiler::FrameProfiler,
    /// What the last frame submitted, per pass. Counted on EVERY frame — draws and triangles need
    /// no GPU feature, so unlike the timings this half is available on any adapter and inside an
    /// ordinary test. A `Cell` because `render` takes `&self`, like `skipped_mesh_draws`.
    frame_counts: Cell<crate::pass_recorder::FrameCounts>,
    /// Which pass owned which timestamp slot in the last frame — the names for the numbers the
    /// query set gives back. Meaningless while the profiler is `Disabled`, and unread then.
    frame_pass_order: Cell<crate::pass_recorder::PassOrder>,
    /// Reused working set for batching a frame's objects into instanced draws. Both the scene and
    /// the vehicle frame run through it, one after the other, so one is enough.
    instance_scratch: crate::scene_resources::InstanceScratch,
}

impl SceneRenderer {
    /// A renderer for a REVIEW image, multisampled at `msaa::review_sample_count` — not what the
    /// game runs. For a frame-time capture use [`Self::for_offscreen_as_shipped`].
    pub fn for_offscreen(
        ctx: &GpuContext,
        terrain_vertices: &[SceneVertex],
        terrain_indices: &[u32],
    ) -> Result<Self, RenderError> {
        Self::new(ctx, wgpu::TextureFormat::Rgba8UnormSrgb, terrain_vertices, terrain_indices)
    }

    /// A renderer whose sample count resolves the way the window's does, so an offscreen capture
    /// measures the frame the player pays for. Pair it with [`crate::OffscreenTarget::new_as_shipped`].
    pub fn for_offscreen_as_shipped(
        ctx: &GpuContext,
        terrain_vertices: &[SceneVertex],
        terrain_indices: &[u32],
    ) -> Result<Self, RenderError> {
        Self::new_with_sample_count_and_quality(
            ctx,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            shipped_sample_count(renderer_api::DEFAULT_MSAA_SAMPLES),
            terrain_vertices,
            terrain_indices,
            None,
        )
    }

    /// As [`Self::for_offscreen`] with an EXPLICIT lighting quality, bypassing the adapter
    /// classification and env overrides. For feature tests that measure a specific chain
    /// (bloom halo, cloud shade, far cascade): the F3 fold keys quality on the machine the
    /// test happens to run on — a folded dev laptop must not silently gut the feature under
    /// test.
    pub fn for_offscreen_with_quality(
        ctx: &GpuContext,
        terrain_vertices: &[SceneVertex],
        terrain_indices: &[u32],
        quality: renderer_api::LightingQuality,
    ) -> Result<Self, RenderError> {
        Self::new_with_quality(
            ctx,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            terrain_vertices,
            terrain_indices,
            Some(quality),
        )
    }

    /// Hand this renderer a per-pass timing instrument. Until one is set the renderer emits the
    /// same command stream it always did, so arming is a probe's decision and never a default.
    pub fn set_pass_profiler(&mut self, profiler: crate::frame_profiler::FrameProfiler) {
        self.profiler = profiler;
    }

    /// The timing instrument this renderer holds — `Disabled` unless a probe armed one.
    pub fn pass_profiler(&self) -> &crate::frame_profiler::FrameProfiler {
        &self.profiler
    }

    /// The three switches that decide which passes this frame encodes, read off the renderer.
    pub fn frame_switches(&self) -> crate::frame_graph::FrameSwitches {
        crate::frame_graph::FrameSwitches {
            ssao: self.ssao.strength > 0.0,
            // Weight AND depth: a tier that sets `bloom_mips` to zero encodes no ladder no matter
            // what the lighting asks for, and the graph has to say what the frame does.
            bloom: self.scene_lighting.bloom_weight > 0.0 && self.bloom.mips > 0,
            refraction: self.refraction && self.sample_count > 1,
        }
    }

    /// Which passes the last frame encoded, in order. Recorded on every frame, armed or not.
    pub fn last_frame_pass_order(&self) -> crate::pass_recorder::PassOrder {
        self.frame_pass_order.get()
    }

    /// What the last rendered frame submitted, per pass: draws, triangles and instances.
    ///
    /// Always populated, on every adapter — this is the half of the instrument that costs nothing
    /// and needs no optional feature, so a draw-call question can be answered without a GPU that
    /// can time anything.
    pub fn last_frame_counts(&self) -> crate::pass_recorder::FrameCounts {
        self.frame_counts.get()
    }

    /// What each pass of the last frame cost on the GPU, if this renderer is armed for timing.
    ///
    /// Blocks on the device, so it belongs after a probe's own fence and never in a shipped
    /// frame. `None` means the instrument is not armed or the frame encoded nothing — never a
    /// table of zeroes, which would read as a free frame.
    pub fn read_pass_timings(
        &self,
        ctx: &GpuContext,
    ) -> Option<crate::frame_profiler::FrameTimings> {
        self.profiler.active()?.read(&ctx.device, self.frame_pass_order.get())
    }

    pub fn new(
        ctx: &GpuContext,
        color_format: wgpu::TextureFormat,
        terrain_vertices: &[SceneVertex],
        terrain_indices: &[u32],
    ) -> Result<Self, RenderError> {
        Self::new_with_quality(ctx, color_format, terrain_vertices, terrain_indices, None)
    }

    fn new_with_quality(
        ctx: &GpuContext,
        color_format: wgpu::TextureFormat,
        terrain_vertices: &[SceneVertex],
        terrain_indices: &[u32],
        quality_override: Option<renderer_api::LightingQuality>,
    ) -> Result<Self, RenderError> {
        Self::new_with_sample_count_and_quality(
            ctx,
            color_format,
            review_sample_count(),
            terrain_vertices,
            terrain_indices,
            quality_override,
        )
    }

    pub fn new_with_sample_count(
        ctx: &GpuContext,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        terrain_vertices: &[SceneVertex],
        terrain_indices: &[u32],
    ) -> Result<Self, RenderError> {
        Self::new_with_sample_count_and_quality(
            ctx,
            color_format,
            sample_count,
            terrain_vertices,
            terrain_indices,
            None,
        )
    }

    fn new_with_sample_count_and_quality(
        ctx: &GpuContext,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        terrain_vertices: &[SceneVertex],
        terrain_indices: &[u32],
        quality_override: Option<renderer_api::LightingQuality>,
    ) -> Result<Self, RenderError> {
        validate_msaa_support(ctx, color_format, DEPTH_FORMAT, sample_count)?;
        let device = &ctx.device;
        let shadow_bgl = env_group::build_shadow_bind_group_layout(device);
        // Every world pipeline renders linear HDR into the internal Rgba16Float target; only the
        // post pass (display transform) and the HUD write the caller's output format.
        let hdr_format = post::HDR_FORMAT;
        let foliage_bgl = foliage_atlas::build_foliage_bind_group_layout(device);
        let foliage_atlas = foliage_atlas::FoliageAtlas::new(device, &ctx.queue, &foliage_bgl);
        let (pipeline, camera_bgl) =
            build_scene_pipeline(device, hdr_format, sample_count, &shadow_bgl, &foliage_bgl);
        let fx_pipeline =
            crate::fx_pipeline::build_fx_pipeline(device, hdr_format, sample_count, &camera_bgl);
        let sky_pipeline =
            crate::sky_pipeline::build_sky_pipeline(device, hdr_format, sample_count, &camera_bgl);
        let rain_pipeline = crate::rain_pipeline::build_rain_pipeline(
            device,
            hdr_format,
            sample_count,
            &camera_bgl,
        );
        let water_pipeline = crate::water_pipeline::build_water_pipeline(
            device,
            hdr_format,
            sample_count,
            &camera_bgl,
        );
        let water_refraction = crate::water_pipeline::WaterRefraction::new(
            device,
            hdr_format,
            sample_count,
            &camera_bgl,
        );
        let (vehicle_pipeline, vehicle_material_bgl) =
            build_vehicle_pipeline(device, hdr_format, sample_count, &shadow_bgl, &camera_bgl);
        // Adapter class + WOT_* env overrides become the frame's lighting quality in one place.
        // One-look policy: every adapter renders the canonical profile; WOT_QUALITY=high is
        // the dev-only rich profile for captures. The startup line names the adapter and the
        // chosen profile — no guessing what the game picked. Feature tests hand an explicit
        // quality instead and skip the resolution entirely.
        let adapter_info = ctx.adapter.get_info();
        let lighting_quality = quality_override.unwrap_or_else(|| {
            let quality_env = std::env::var("WOT_QUALITY").ok();
            let base = quality::resolve_base_profile(quality_env.as_deref());
            tracing::info!(
                adapter = %adapter_info.name,
                reported = ?adapter_info.device_type,
                profile = if base == renderer_api::LightingQuality::rich() {
                    "rich (dev)"
                } else {
                    "canonical"
                },
                "one-look profile resolved (dev override: WOT_QUALITY=high)"
            );
            quality::apply_shader_detail_override(
                quality::apply_cloud_shadow_override(
                    quality::apply_refraction_override(
                        quality::resolve_lighting_quality_with_bloom(
                            base,
                            std::env::var("WOT_SHADOW_RES").ok().as_deref(),
                            std::env::var("WOT_SHADOW_CASCADES").ok().as_deref(),
                            std::env::var("WOT_SSAO").ok().as_deref(),
                            std::env::var("WOT_BLOOM").ok().as_deref(),
                        ),
                        std::env::var("WOT_REFRACTION").ok().as_deref(),
                    ),
                    std::env::var("WOT_CLOUD_SHADOWS").ok().as_deref(),
                ),
                std::env::var("WOT_GPU_DETAIL").ok().as_deref(),
            )
        });
        let ssao = ssao::SsaoResources::new(
            device,
            &camera_bgl,
            &foliage_bgl,
            lighting_quality.ssao_scale,
        );
        let placeholder_ao = ssao_pipelines::placeholder_ao_view(device, &ctx.queue);
        let shadow = shadow::ShadowResources::new(
            device,
            &ctx.queue,
            &shadow_bgl,
            &camera_bgl,
            &foliage_bgl,
            &placeholder_ao,
            lighting_quality.shadow_resolution,
            lighting_quality.shadow_cascades,
        );

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene_camera"),
            size: u64::from(CameraUniform::wgsl_size() as u32),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let armor_damage = armor_damage::ArmorDamageBuffers::new(device);
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene_camera_bg"),
            layout: &camera_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: camera_buffer.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: armor_damage.headers.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: armor_damage.apertures.as_entire_binding(),
                },
            ],
        });

        let (chunked_indices, terrain_chunks) =
            terrain::chunk_initial_terrain(terrain_vertices, terrain_indices);
        let buffers = buffers::GeometryBuffers::new(device, terrain_vertices, &chunked_indices);
        let (hud_font_bgl, hud_font_sampler, hud_font_bind_group) =
            hud_atlas::create_hud_font_resources(device, &ctx.queue);
        let vehicle_materials = vehicle_materials::VehicleMaterialRegistry::new(
            device,
            &ctx.queue,
            vehicle_material_bgl,
        );
        let hud_pipeline = build_hud_pipeline(device, color_format, &hud_font_bgl);
        let post = post::PostResources::new(device, &camera_bgl);
        let fxaa = post::FxaaResources::new(device, color_format, &camera_bgl);
        let bloom = bloom::BloomResources::new(device, lighting_quality.bloom_mips);
        let ground = ground::GroundResources::new(
            device,
            hdr_format,
            sample_count,
            &camera_bgl,
            &shadow_bgl,
        );

        Ok(Self {
            pipeline,
            camera_buffer,
            camera_bind_group,
            armor_damage,
            terrain_vertices: buffers.terrain_vertices,
            terrain_indices: buffers.terrain_indices,
            terrain_index_count: chunked_indices.len() as u32,
            terrain_chunks,
            dressing_vertices: buffers.dressing_vertices,
            dressing_indices: buffers.dressing_indices,
            dressing_index_count: 0,
            dressing_chunks: Vec::new(),
            dynamic_vertices: buffers.dynamic_vertices,
            dynamic_indices: buffers.dynamic_indices,
            dynamic_index_count: 0,
            identity_instance: buffers.identity_instance,
            frame_instances: buffers.frame_instances,
            frame_instance_count: 0,
            frame_draws: Vec::new(),
            static_meshes: SceneMeshRegistry::default(),
            vehicle_pipeline,
            vehicle_materials,
            vehicle_instances: buffers.vehicle_instances,
            vehicle_instance_count: 0,
            vehicle_draws: Vec::new(),
            grass_crushers: Vec::new(),
            vehicle_meshes: VehicleMeshRegistry::default(),
            sky_pipeline,
            rain_pipeline,
            rain_intensity: 0.0,
            wetness: 0.0,
            puddle_fill: 0.0,
            cloud_offset: [0.0; 2],
            rain_phase_s: 0.0,
            water_pipeline,
            water_refraction,
            refraction: lighting_quality.refraction,
            water_buffers: None,
            water_index_count: 0,
            fx_pipeline,
            fx_vertices: buffers.fx_vertices,
            fx_vertex_count: 0,
            hud_pipeline,
            hud_vertices: buffers.hud_vertices,
            hud_vertex_count: 0,
            hud_font_bgl,
            hud_font_sampler,
            hud_font_bind_group,
            sample_count,
            sky: wgpu::Color { r: 0.55, g: 0.69, b: 0.87, a: 1.0 },
            // Outdoor by default: the gradient-sky pass paints the background. Interior scenes turn
            // it off (see set_interior_background).
            draw_sky: true,
            scene_lighting: SceneLighting::battlefield_default(),
            shadow,
            shadow_bgl,
            foliage_atlas,
            foliage_bgl,
            ssao,
            ground,
            post,
            fxaa,
            bloom,
            default_bloom_mips: lighting_quality.bloom_mips,
            cloud_shadows_enabled: lighting_quality.cloud_shadows,
            shader_detail: lighting_quality.shader_detail,
            shadow_focus: None,
            shadow_focus_radius_m: None,
            scene_time_s: 0.0,
            skipped_mesh_draws: Cell::new(0),
            profiler: crate::frame_profiler::FrameProfiler::Disabled,
            frame_counts: Cell::new(crate::pass_recorder::FrameCounts::default()),
            frame_pass_order: Cell::new(crate::pass_recorder::PassOrder::default()),
            instance_scratch: crate::scene_resources::InstanceScratch::default(),
        })
    }
}
