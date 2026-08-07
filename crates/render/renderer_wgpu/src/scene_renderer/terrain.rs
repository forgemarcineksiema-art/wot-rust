//! Swapping the static scene geometry (the "terrain" slot) so one renderer can host both the
//! battlefield and the garage hangar. The slot is chunked on upload (`renderer_api::culling`):
//! indices are reordered into spatial draw ranges so every pass draws only the chunks its
//! frustum sees — the camera skips the steppe behind the hill, the sun-shadow boxes skip the
//! map outside their footprint. Chunking is invisible by construction (opaque, depth-tested
//! geometry; the golden screenshot locks hold it to byte-identical frames).

use renderer_api::{Frustum, SceneChunk, SceneVertex, chunk_scene_indices};
use wgpu::util::DeviceExt;

use super::SceneRenderer;
use crate::GpuContext;

/// Chunk edge on the XZ grid: 80 m = 16 heightmap cells on the 5 m battlefield grid, ~13x13
/// chunks on a 1000 m map — small enough to cull meaningfully, large enough that the per-draw
/// overhead stays noise.
pub(crate) const TERRAIN_CHUNK_SIZE_M: f32 = 80.0;

impl SceneRenderer {
    /// Replace the baked static scene geometry. Recreates the vertex and index buffers — cheap
    /// because it only happens on a scene change (e.g. garage <-> battle), not per frame.
    pub fn set_terrain(&mut self, ctx: &GpuContext, vertices: &[SceneVertex], indices: &[u32]) {
        let (reordered, chunks) = chunk_scene_indices(vertices, indices, TERRAIN_CHUNK_SIZE_M);
        self.terrain_vertices = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_terrain_v"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.terrain_indices = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_terrain_i"),
            contents: bytemuck::cast_slice(&reordered),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.terrain_index_count = reordered.len() as u32;
        self.terrain_chunks = chunks;
    }

    /// Index count of the currently bound static scene geometry (test/diagnostic hook).
    pub fn terrain_index_count(&self) -> u32 {
        self.terrain_index_count
    }

    /// How many spatial chunks the bound scene geometry is split into (test/diagnostic hook).
    pub fn terrain_chunk_count(&self) -> usize {
        self.terrain_chunks.len()
    }

    /// How many terrain chunks a camera at `view_proj` would draw (test/diagnostic hook — the
    /// exact visibility the render passes use).
    pub fn visible_terrain_chunks(&self, view_proj: &[[f32; 4]; 4]) -> usize {
        let frustum = Frustum::from_view_proj(view_proj);
        self.terrain_chunks.iter().filter(|c| frustum.intersects_aabb(&c.aabb)).count()
    }

    /// Issue the draw calls for every chunk `frustum` can see. The caller has already bound the
    /// terrain vertex/index buffers and the pass pipeline; this only decides WHAT to draw.
    pub(super) fn draw_visible_terrain(
        &self,
        pass: &mut crate::pass_recorder::CountedPass<'_, '_>,
        frustum: &Frustum,
    ) {
        for chunk in &self.terrain_chunks {
            if frustum.intersects_aabb(&chunk.aabb) {
                pass.draw_indexed(
                    chunk.index_start..chunk.index_start + chunk.index_count,
                    0,
                    0..1,
                );
            }
        }
    }
}

/// Chunk the initial construction-time geometry (the same operation `set_terrain` applies).
pub(crate) fn chunk_initial_terrain(
    vertices: &[SceneVertex],
    indices: &[u32],
) -> (Vec<u32>, Vec<SceneChunk>) {
    chunk_scene_indices(vertices, indices, TERRAIN_CHUNK_SIZE_M)
}
