//! The water-surface slot: a static river mesh swapped with the scene (empty in the garage,
//! the Bystra's corridor in battle). Animation is entirely shader-side, so like the terrain
//! this uploads once per scene change, never per frame.

use renderer_api::WaterVertex;
use wgpu::util::DeviceExt;

use super::SceneRenderer;
use crate::GpuContext;

impl SceneRenderer {
    /// Replace the water-surface mesh. An empty slice clears the slot (dry maps, the garage).
    pub fn set_water(&mut self, ctx: &GpuContext, vertices: &[WaterVertex], indices: &[u32]) {
        self.water_index_count = indices.len() as u32;
        if indices.is_empty() {
            self.water_buffers = None;
            return;
        }
        let vertex_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_water_v"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_water_i"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.water_buffers = Some((vertex_buffer, index_buffer));
    }

    /// Index count of the currently bound water mesh (test/diagnostic hook).
    pub fn water_index_count(&self) -> u32 {
        self.water_index_count
    }
}
