//! Frame geometry buffer creation for the scene renderer (terrain, dynamic, instances, HUD).
//! Split from `scene_renderer.rs` to keep each module within the reviewability budget.

use renderer_api::SceneVertex;
use wgpu::util::DeviceExt;

use crate::scene_resources::SceneInstance;

use super::{
    DYNAMIC_INDEX_CAPACITY, DYNAMIC_VERTEX_CAPACITY, FX_VERTEX_CAPACITY, HUD_VERTEX_CAPACITY,
};

/// Vehicle instance budget in bytes. A full 7v7 of blueprint vehicles is ~3.5k instances — the
/// animated running gear alone is ~200 per tank (90+ track links per side, wheels, swing arms) —
/// so at 80 B per instance the frame needs ~280 KiB; 1 MiB (~13k instances) leaves headroom.
/// `set_vehicle_render_frame` truncates (with a warning) beyond it instead of dropping the whole
/// upload, which would leave the previous frame's poses on screen with every tank frozen mid-run.
pub const VEHICLE_INSTANCE_CAPACITY: u64 = 1 << 20;

/// How many vehicle instances fit [`VEHICLE_INSTANCE_CAPACITY`] — the client locks its worst-case
/// battle frame against this so a roster/gear change cannot silently outgrow the buffer again.
pub const fn vehicle_instance_budget() -> usize {
    (VEHICLE_INSTANCE_CAPACITY as usize)
        / std::mem::size_of::<crate::scene_resources::SceneInstance>()
}

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
            // 512 KiB (~6.5k instances at 80 B): the geometry-path lineup examples submit the
            // whole 7-vehicle roster with animated running gear (~200 instances per tank), and
            // the near-field grass ring (60 m, screen-constant density) peaks at ~4.8k tufts —
            // the old 256 KiB ceiling would have clipped the field's far rim every frame.
            size: 1 << 19,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let vehicle_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vehicle_frame_instances"),
            size: VEHICLE_INSTANCE_CAPACITY,
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
