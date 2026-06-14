use renderer_api::{
    HudVertex, MaterialHandle, MeshAsset, MeshHandle, RenderFrame, SceneVertex,
    VehicleMaterialMaps, VehicleMeshAsset,
};

use crate::GpuContext;
use crate::scene_resources::frame_instances;

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
        maps: &VehicleMaterialMaps,
    ) {
        self.vehicle_materials.register(ctx, handle, maps);
    }

    pub fn set_render_frame(&mut self, ctx: &GpuContext, frame: &RenderFrame) {
        let (instances, draws) = frame_instances(frame);
        let bytes: &[u8] = bytemuck::cast_slice(&instances);
        if bytes.len() as u64 > self.frame_instances.size() {
            return;
        }
        ctx.queue.write_buffer(&self.frame_instances, 0, bytes);
        self.frame_instance_count = instances.len() as u32;
        self.frame_draws = draws;
    }

    pub fn set_vehicle_render_frame(&mut self, ctx: &GpuContext, frame: &RenderFrame) {
        let (instances, draws) = frame_instances(frame);
        let bytes: &[u8] = bytemuck::cast_slice(&instances);
        if bytes.len() as u64 > self.vehicle_instances.size() {
            return;
        }
        ctx.queue.write_buffer(&self.vehicle_instances, 0, bytes);
        self.vehicle_instance_count = instances.len() as u32;
        self.vehicle_draws = draws;
    }

    pub fn set_hud(&mut self, ctx: &GpuContext, vertices: &[HudVertex]) {
        let bytes: &[u8] = bytemuck::cast_slice(vertices);
        if bytes.len() as u64 > super::HUD_VERTEX_CAPACITY {
            return;
        }
        ctx.queue.write_buffer(&self.hud_vertices, 0, bytes);
        self.hud_vertex_count = vertices.len() as u32;
    }
}
