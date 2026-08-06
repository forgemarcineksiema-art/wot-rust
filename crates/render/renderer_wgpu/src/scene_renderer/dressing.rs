//! The DRESSING slot (Żywy Step P2): mid-field grass-clump cards and other shadowless ground
//! dressing, baked statically per map and chunked like the terrain slot — but drawn ONLY in
//! the opaque color pass, with a distance cutoff on top of the frustum: dressing never enters
//! the shadow cascades or the SSAO prepass (a knee-high card's shadow is noise, its occlusion
//! a lie), and past its own collapse band there is nothing left to rasterize.

use renderer_api::{Frustum, Rgba8MipChain, SceneVertex, chunk_scene_indices};
use wgpu::util::DeviceExt;

use super::SceneRenderer;
use super::terrain::TERRAIN_CHUNK_SIZE_M;
use crate::GpuContext;

/// Chunks whose nearest point lies beyond this never draw: the cards inside them have fully
/// collapsed by 330 m (see `scene.wgsl`), so the cutoff only skips guaranteed-empty work.
pub(crate) const DRESSING_CUTOFF_M: f32 = 340.0;

impl SceneRenderer {
    /// Replace the baked dressing geometry (empty slices clear the slot — the garage has no
    /// meadow). Recreated on scene swaps and on crater rebakes, never per frame.
    /// Replace the foliage atlas (Imported Flora 2.0): complete RGBA8 sRGB mips, sampled by the
    /// scene color pass AND the shadow/SSAO depth passes — a leaf's shadow is its mask. The
    /// startup default is one opaque-white texel (a bit-exact no-op for procedural content).
    /// `normals` is the parallel tangent-normal page (hero flora): same regions, same UVs.
    /// `None` binds a flat 1x1 and the surface keeps its geometric normal.
    pub fn set_foliage_atlas(
        &mut self,
        ctx: &GpuContext,
        chain: &Rgba8MipChain,
        normals: Option<&Rgba8MipChain>,
    ) {
        self.foliage_atlas.set(&ctx.device, &ctx.queue, &self.foliage_bgl, chain, normals);
    }

    pub fn set_dressing(&mut self, ctx: &GpuContext, vertices: &[SceneVertex], indices: &[u32]) {
        if vertices.is_empty() || indices.is_empty() {
            self.dressing_index_count = 0;
            self.dressing_chunks.clear();
            return;
        }
        let (reordered, chunks) = chunk_scene_indices(vertices, indices, TERRAIN_CHUNK_SIZE_M);
        self.dressing_vertices = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_dressing_v"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.dressing_indices = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_dressing_i"),
            contents: bytemuck::cast_slice(&reordered),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.dressing_index_count = reordered.len() as u32;
        self.dressing_chunks = chunks;
    }

    /// How many dressing chunks a camera would draw (test/diagnostic hook — the exact
    /// frustum-plus-distance visibility the color pass uses, magnification included).
    pub fn visible_dressing_chunks(&self, view_proj: &[[f32; 4]; 4], eye: [f32; 3]) -> usize {
        let frustum = Frustum::from_view_proj(view_proj);
        let cutoff = DRESSING_CUTOFF_M
            * renderer_api::grass_zoom_band_scale(renderer_api::projection_y_scale(view_proj));
        self.dressing_chunks
            .iter()
            .filter(|chunk| {
                frustum.intersects_aabb(&chunk.aabb)
                    && chunk_within_cutoff(&chunk.aabb, eye, cutoff)
            })
            .count()
    }

    /// Issue the dressing draws for the opaque COLOR pass only. The caller has bound the
    /// pipeline and camera; this binds the slot's buffers and decides what to draw.
    ///
    /// `band_scale` is the frame's grass-band magnification (1.0 unzoomed): the chunk cutoff
    /// must reach as far as the shader is willing to stand a far tuft, or the scope would
    /// look through a hole the CPU never submitted.
    pub(super) fn draw_visible_dressing(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        frustum: &Frustum,
        eye: [f32; 3],
        band_scale: f32,
    ) {
        if self.dressing_index_count == 0 {
            return;
        }
        let cutoff = DRESSING_CUTOFF_M * band_scale;
        pass.set_vertex_buffer(0, self.dressing_vertices.slice(..));
        pass.set_index_buffer(self.dressing_indices.slice(..), wgpu::IndexFormat::Uint32);
        for chunk in &self.dressing_chunks {
            if frustum.intersects_aabb(&chunk.aabb) && chunk_within_cutoff(&chunk.aabb, eye, cutoff)
            {
                pass.draw_indexed(
                    chunk.index_start..chunk.index_start + chunk.index_count,
                    0,
                    0..1,
                );
            }
        }
    }
}

/// Whether any point of the chunk's AABB lies within `cutoff` of the eye (planar).
fn chunk_within_cutoff(aabb: &renderer_api::Aabb, eye: [f32; 3], cutoff: f32) -> bool {
    let dx = (aabb.min[0] - eye[0]).max(eye[0] - aabb.max[0]).max(0.0);
    let dz = (aabb.min[2] - eye[2]).max(eye[2] - aabb.max[2]).max(0.0);
    dx * dx + dz * dz <= cutoff * cutoff
}
