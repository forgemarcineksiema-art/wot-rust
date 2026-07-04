mod buffers;
mod draw;
mod draw_depth;
mod hud_atlas;
mod resources;
pub(crate) mod env_group;
pub(crate) mod shadow;
pub(crate) mod ssao;
mod ssao_pipelines;
mod terrain;
mod vehicle_materials;

use std::cell::Cell;

use renderer_api::{RenderError, SceneLighting, SceneVertex};

use crate::msaa::{default_sample_count, validate_msaa_support};
use crate::offscreen::DEPTH_FORMAT;
use crate::scene_pipeline::{build_hud_pipeline, build_scene_pipeline};
use crate::scene_resources::{SceneMeshRegistry, SceneObjectDraw};
use crate::{CameraUniform, GpuContext, VehicleMeshRegistry, build_vehicle_pipeline};

const DYNAMIC_VERTEX_CAPACITY: u64 = 1 << 20;
const DYNAMIC_INDEX_CAPACITY: u64 = 1 << 20;
const HUD_VERTEX_CAPACITY: u64 = 1 << 16;

pub struct SceneRenderer {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    terrain_vertices: wgpu::Buffer,
    terrain_indices: wgpu::Buffer,
    terrain_index_count: u32,
    dynamic_vertices: wgpu::Buffer,
    dynamic_indices: wgpu::Buffer,
    dynamic_index_count: u32,
    identity_instance: wgpu::Buffer,
    frame_instances: wgpu::Buffer,
    frame_instance_count: u32,
    frame_draws: Vec<SceneObjectDraw>,
    static_meshes: SceneMeshRegistry,
    vehicle_pipeline: wgpu::RenderPipeline,
    vehicle_camera_bind_group: wgpu::BindGroup,
    vehicle_materials: vehicle_materials::VehicleMaterialRegistry,
    vehicle_instances: wgpu::Buffer,
    vehicle_instance_count: u32,
    vehicle_draws: Vec<SceneObjectDraw>,
    vehicle_meshes: VehicleMeshRegistry,
    hud_pipeline: wgpu::RenderPipeline,
    hud_vertices: wgpu::Buffer,
    hud_vertex_count: u32,
    hud_font_bgl: wgpu::BindGroupLayout,
    hud_font_sampler: wgpu::Sampler,
    hud_font_bind_group: wgpu::BindGroup,
    sample_count: u32,
    pub sky: wgpu::Color,
    /// The calibrated three-point lighting for this scene. Battle uses the default profile; the
    /// garage swaps in a warm studio key/fill/rim. Drives both the scene and the vehicle shaders.
    pub scene_lighting: SceneLighting,
    shadow: shadow::ShadowResources,
    shadow_bgl: wgpu::BindGroupLayout,
    ssao: ssao::SsaoResources,
    /// World point the focused sun-shadow box centres on (the player/subject). `None` falls back to
    /// the camera position, which still covers the near action.
    pub shadow_focus: Option<[f32; 3]>,
    pub skipped_mesh_draws: Cell<u32>,
}

impl SceneRenderer {
    /// Set the sky clear colour (RGB in 0–1). The garage uses a dim interior tone; the battle
    /// uses the default daylight blue.
    pub fn set_sky(&mut self, r: f64, g: f64, b: f64) {
        self.sky = wgpu::Color { r, g, b, a: 1.0 };
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

    pub fn for_offscreen(
        ctx: &GpuContext,
        terrain_vertices: &[SceneVertex],
        terrain_indices: &[u32],
    ) -> Result<Self, RenderError> {
        Self::new(ctx, wgpu::TextureFormat::Rgba8UnormSrgb, terrain_vertices, terrain_indices)
    }

    pub fn new(
        ctx: &GpuContext,
        color_format: wgpu::TextureFormat,
        terrain_vertices: &[SceneVertex],
        terrain_indices: &[u32],
    ) -> Result<Self, RenderError> {
        Self::new_with_sample_count(
            ctx,
            color_format,
            default_sample_count(),
            terrain_vertices,
            terrain_indices,
        )
    }

    pub fn new_with_sample_count(
        ctx: &GpuContext,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        terrain_vertices: &[SceneVertex],
        terrain_indices: &[u32],
    ) -> Result<Self, RenderError> {
        validate_msaa_support(ctx, color_format, DEPTH_FORMAT, sample_count)?;
        let device = &ctx.device;
        let shadow_bgl = env_group::build_shadow_bind_group_layout(device);
        let (pipeline, camera_bgl) =
            build_scene_pipeline(device, color_format, sample_count, &shadow_bgl);
        let (vehicle_pipeline, vehicle_camera_bgl, vehicle_material_bgl) =
            build_vehicle_pipeline(device, color_format, sample_count, &shadow_bgl);
        let ssao = ssao::SsaoResources::new(device, &camera_bgl);
        let placeholder_ao = ssao_pipelines::placeholder_ao_view(device, &ctx.queue);
        let shadow = shadow::ShadowResources::new(device, &shadow_bgl, &camera_bgl, &placeholder_ao);

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene_camera"),
            size: u64::from(CameraUniform::wgsl_size() as u32),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene_camera_bg"),
            layout: &camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let vehicle_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vehicle_camera_bg"),
            layout: &vehicle_camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let buffers = buffers::GeometryBuffers::new(device, terrain_vertices, terrain_indices);
        let (hud_font_bgl, hud_font_sampler, hud_font_bind_group) =
            hud_atlas::create_hud_font_resources(device, &ctx.queue);
        let vehicle_materials = vehicle_materials::VehicleMaterialRegistry::new(
            device,
            &ctx.queue,
            vehicle_material_bgl,
        );
        let hud_pipeline = build_hud_pipeline(device, color_format, sample_count, &hud_font_bgl);

        Ok(Self {
            pipeline,
            camera_buffer,
            camera_bind_group,
            terrain_vertices: buffers.terrain_vertices,
            terrain_indices: buffers.terrain_indices,
            terrain_index_count: terrain_indices.len() as u32,
            dynamic_vertices: buffers.dynamic_vertices,
            dynamic_indices: buffers.dynamic_indices,
            dynamic_index_count: 0,
            identity_instance: buffers.identity_instance,
            frame_instances: buffers.frame_instances,
            frame_instance_count: 0,
            frame_draws: Vec::new(),
            static_meshes: SceneMeshRegistry::default(),
            vehicle_pipeline,
            vehicle_camera_bind_group,
            vehicle_materials,
            vehicle_instances: buffers.vehicle_instances,
            vehicle_instance_count: 0,
            vehicle_draws: Vec::new(),
            vehicle_meshes: VehicleMeshRegistry::default(),
            hud_pipeline,
            hud_vertices: buffers.hud_vertices,
            hud_vertex_count: 0,
            hud_font_bgl,
            hud_font_sampler,
            hud_font_bind_group,
            sample_count,
            sky: wgpu::Color { r: 0.55, g: 0.69, b: 0.87, a: 1.0 },
            scene_lighting: SceneLighting::battlefield_default(),
            shadow,
            shadow_bgl,
            ssao,
            shadow_focus: None,
            skipped_mesh_draws: Cell::new(0),
        })
    }
}
