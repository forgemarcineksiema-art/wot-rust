mod draw;
mod hud_atlas;
mod resources;
mod terrain;
mod vehicle_materials;

use std::cell::Cell;

use renderer_api::{RenderError, SceneVertex};
use wgpu::util::DeviceExt;

use crate::msaa::{default_sample_count, validate_msaa_support};
use crate::offscreen::DEPTH_FORMAT;
use crate::scene_pipeline::{build_hud_pipeline, build_scene_pipeline};
use crate::scene_resources::{SceneInstance, SceneMeshRegistry, SceneObjectDraw};
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
    /// Per-scene RGB colour multiplier on the lit result (1,1,1 = unchanged). Warm in the garage.
    pub scene_tint: [f32; 3],
    pub skipped_mesh_draws: Cell<u32>,
}

impl SceneRenderer {
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
        let (pipeline, camera_bgl) = build_scene_pipeline(device, color_format, sample_count);
        let (vehicle_pipeline, vehicle_camera_bgl, vehicle_material_bgl) =
            build_vehicle_pipeline(device, color_format, sample_count);

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

        let terrain_vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_terrain_v"),
            contents: bytemuck::cast_slice(terrain_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let terrain_ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_terrain_i"),
            contents: bytemuck::cast_slice(terrain_indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let dynamic_vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene_dynamic_v"),
            size: DYNAMIC_VERTEX_CAPACITY,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dynamic_indices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene_dynamic_i"),
            size: DYNAMIC_INDEX_CAPACITY,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let identity_instance = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_identity_instance"),
            contents: bytemuck::bytes_of(&SceneInstance::identity()),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let frame_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene_frame_instances"),
            size: 1 << 16,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let vehicle_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vehicle_frame_instances"),
            size: 1 << 16,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (hud_font_bgl, hud_font_sampler, hud_font_bind_group) =
            hud_atlas::create_hud_font_resources(device, &ctx.queue);
        let vehicle_materials = vehicle_materials::VehicleMaterialRegistry::new(
            device,
            &ctx.queue,
            vehicle_material_bgl,
        );
        let hud_pipeline = build_hud_pipeline(device, color_format, sample_count, &hud_font_bgl);
        let hud_vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene_hud_v"),
            size: HUD_VERTEX_CAPACITY,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            pipeline,
            camera_buffer,
            camera_bind_group,
            terrain_vertices: terrain_vbuf,
            terrain_indices: terrain_ibuf,
            terrain_index_count: terrain_indices.len() as u32,
            dynamic_vertices,
            dynamic_indices,
            dynamic_index_count: 0,
            identity_instance,
            frame_instances,
            frame_instance_count: 0,
            frame_draws: Vec::new(),
            static_meshes: SceneMeshRegistry::default(),
            vehicle_pipeline,
            vehicle_camera_bind_group,
            vehicle_materials,
            vehicle_instances,
            vehicle_instance_count: 0,
            vehicle_draws: Vec::new(),
            vehicle_meshes: VehicleMeshRegistry::default(),
            hud_pipeline,
            hud_vertices,
            hud_vertex_count: 0,
            hud_font_bgl,
            hud_font_sampler,
            hud_font_bind_group,
            sample_count,
            sky: wgpu::Color { r: 0.55, g: 0.69, b: 0.87, a: 1.0 },
            scene_tint: [1.0, 1.0, 1.0],
            skipped_mesh_draws: Cell::new(0),
        })
    }
}
