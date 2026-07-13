use renderer_api::{
    FxVertex, HudVertex, MaterialHandle, MeshAsset, MeshHandle, RenderFrame, SceneVertex,
    VehicleMaterialFamilies, VehicleMeshAsset,
};

use crate::GpuContext;
use crate::scene_resources::{SceneInstance, clip_instances_to_capacity, frame_instances};

impl super::SceneRenderer {
    pub fn set_dynamic_mesh(
        &mut self,
        ctx: &GpuContext,
        vertices: &[SceneVertex],
        indices: &[u32],
    ) {
        let vbytes: &[u8] = bytemuck::cast_slice(vertices);
        let ibytes: &[u8] = bytemuck::cast_slice(indices);
        if vbytes.len() as u64 > super::DYNAMIC_VERTEX_CAPACITY
            || ibytes.len() as u64 > super::DYNAMIC_INDEX_CAPACITY
        {
            // Refusing the upload keeps the previous mesh; never do it silently — a dropped
            // dynamic mesh reads as "the tanks vanished" with no trace of why.
            tracing::warn!(
                vertices = vertices.len(),
                indices = indices.len(),
                "dynamic mesh exceeds its buffer budget; keeping the previous mesh"
            );
            return;
        }
        ctx.queue.write_buffer(&self.dynamic_vertices, 0, vbytes);
        ctx.queue.write_buffer(&self.dynamic_indices, 0, ibytes);
        self.dynamic_index_count = indices.len() as u32;
    }

    pub fn register_mesh(&mut self, ctx: &GpuContext, handle: MeshHandle, mesh: &MeshAsset) {
        self.static_meshes.register(ctx, handle, mesh);
    }

    pub fn register_vehicle_mesh(
        &mut self,
        ctx: &GpuContext,
        handle: MeshHandle,
        mesh: &VehicleMeshAsset,
    ) {
        self.vehicle_meshes.register(ctx, handle, mesh);
    }

    pub fn register_vehicle_material(
        &mut self,
        ctx: &GpuContext,
        handle: MaterialHandle,
        families: &VehicleMaterialFamilies,
    ) {
        self.vehicle_materials.register(ctx, handle, families);
    }

    /// An oversized frame is truncated to the buffer budget (with a warning), never dropped
    /// whole: a dropped upload leaves the previous instances on screen, so every object would
    /// freeze in its last uploaded pose while the simulation drives on without it.
    pub fn set_render_frame(&mut self, ctx: &GpuContext, frame: &RenderFrame) {
        let (mut instances, mut draws) = frame_instances(frame);
        let capacity = buffer_instance_capacity(&self.frame_instances);
        if clip_instances_to_capacity(&mut instances, &mut draws, capacity) {
            tracing::warn!(kept = capacity, "scene instance budget exceeded; truncating frame");
        }
        ctx.queue.write_buffer(&self.frame_instances, 0, bytemuck::cast_slice(&instances));
        self.frame_instance_count = instances.len() as u32;
        self.frame_draws = draws;
    }

    /// See [`Self::set_render_frame`]: truncate-and-warn, never a silent whole-frame drop — that
    /// is exactly the failure that froze every tank on screen once a 7v7 exceeded the old budget.
    pub fn set_vehicle_render_frame(&mut self, ctx: &GpuContext, frame: &RenderFrame) {
        self.armor_damage.upload(ctx, &frame.armor_damage);
        let (mut instances, mut draws) = frame_instances(frame);
        let capacity = buffer_instance_capacity(&self.vehicle_instances);
        if clip_instances_to_capacity(&mut instances, &mut draws, capacity) {
            tracing::warn!(kept = capacity, "vehicle instance budget exceeded; truncating frame");
        }
        ctx.queue.write_buffer(&self.vehicle_instances, 0, bytemuck::cast_slice(&instances));
        self.vehicle_instance_count = instances.len() as u32;
        self.vehicle_draws = draws;
    }

    /// Upload this frame's battle-FX quads (already world-space, premultiplied colors). Oversized
    /// uploads are dropped whole, like every other per-frame buffer here.
    pub fn set_fx(&mut self, ctx: &GpuContext, vertices: &[FxVertex]) {
        let bytes: &[u8] = bytemuck::cast_slice(vertices);
        if bytes.len() as u64 > super::FX_VERTEX_CAPACITY {
            return;
        }
        ctx.queue.write_buffer(&self.fx_vertices, 0, bytes);
        self.fx_vertex_count = vertices.len() as u32;
    }

    /// Upload this frame's HUD overlay. An oversized upload keeps the first triangles that fit
    /// instead of blanking the whole HUD — losing the last elements beats losing the reticle —
    /// and warns so the regression is visible in logs.
    pub fn set_hud(&mut self, ctx: &GpuContext, vertices: &[HudVertex]) {
        let capacity_vertices =
            (super::HUD_VERTEX_CAPACITY as usize) / std::mem::size_of::<HudVertex>();
        let vertices = if vertices.len() > capacity_vertices {
            let kept = capacity_vertices - capacity_vertices % 3;
            tracing::warn!(
                submitted = vertices.len(),
                kept,
                "HUD vertex budget exceeded; truncating overlay"
            );
            &vertices[..kept]
        } else {
            vertices
        };
        ctx.queue.write_buffer(&self.hud_vertices, 0, bytemuck::cast_slice(vertices));
        self.hud_vertex_count = vertices.len() as u32;
    }

    /// Vertices the HUD pass will draw this frame — what `set_hud` accepted after budgeting.
    pub fn hud_vertex_count(&self) -> u32 {
        self.hud_vertex_count
    }
}

/// How many [`SceneInstance`]s the given GPU buffer holds.
fn buffer_instance_capacity(buffer: &wgpu::Buffer) -> usize {
    (buffer.size() as usize) / std::mem::size_of::<SceneInstance>()
}
