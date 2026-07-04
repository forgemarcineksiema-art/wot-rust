//! Frame geometry buffer creation for the scene renderer (terrain, dynamic, instances, HUD).
//! Split from `scene_renderer.rs` to keep each module within the reviewability budget.

use renderer_api::SceneVertex;
use wgpu::util::DeviceExt;

use crate::scene_resources::SceneInstance;

use super::{
    DYNAMIC_INDEX_CAPACITY, DYNAMIC_VERTEX_CAPACITY, FX_VERTEX_CAPACITY, HUD_VERTEX_CAPACITY,
};

pub(super) struct GeometryBuffers {
    pub terrain_vertices: wgpu::Buffer,
    pub terrain_indices: wgpu::Buffer,
    pub dynamic_vertices: wgpu::Buffer,
    pub dynamic_indices: wgpu::Buffer,
    pub identity_instance: wgpu::Buffer,
    pub frame_instances: wgpu::Buffer,
    pub vehicle_instances: wgpu::Buffer,
    pub fx_vertices: wgpu::Buffer,
    pub hud_vertices: wgpu::Buffer,
}

impl GeometryBuffers {
    pub fn new(
        device: &wgpu::Device,
        terrain_vertices: &[SceneVertex],
        terrain_indices: &[u32],
    ) -> Self {
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
        let fx_vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene_fx_v"),
            size: FX_VERTEX_CAPACITY,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hud_vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene_hud_v"),
            size: HUD_VERTEX_CAPACITY,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            terrain_vertices: terrain_vbuf,
            terrain_indices: terrain_ibuf,
            dynamic_vertices,
            dynamic_indices,
            identity_instance,
            frame_instances,
            vehicle_instances,
            fx_vertices,
            hud_vertices,
        }
    }
}
