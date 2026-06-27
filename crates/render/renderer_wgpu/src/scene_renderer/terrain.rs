//! Swapping the static scene geometry (the "terrain" slot) so one renderer can host both the
//! battlefield and the garage hangar.

use renderer_api::SceneVertex;
use wgpu::util::DeviceExt;

use super::SceneRenderer;
use crate::GpuContext;

impl SceneRenderer {
    /// Replace the baked static scene geometry. Recreates the vertex and index buffers — cheap
    /// because it only happens on a scene change (e.g. garage <-> battle), not per frame.
    pub fn set_terrain(&mut self, ctx: &GpuContext, vertices: &[SceneVertex], indices: &[u32]) {
        self.terrain_vertices = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_terrain_v"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.terrain_indices = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_terrain_i"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.terrain_index_count = indices.len() as u32;
    }

    /// Index count of the currently bound static scene geometry (test/diagnostic hook).
    pub fn terrain_index_count(&self) -> u32 {
        self.terrain_index_count
    }
}
