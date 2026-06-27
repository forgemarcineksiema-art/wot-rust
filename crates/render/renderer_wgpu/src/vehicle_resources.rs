use std::collections::BTreeMap;

use renderer_api::{MeshHandle, VehicleMeshAsset};
use wgpu::util::DeviceExt;

use crate::GpuContext;

pub struct GpuVehicleMesh {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub vertex_count: u32,
    pub index_count: u32,
}

#[derive(Default)]
pub struct VehicleMeshRegistry {
    meshes: BTreeMap<u32, GpuVehicleMesh>,
}

impl VehicleMeshRegistry {
    pub fn register(&mut self, ctx: &GpuContext, handle: MeshHandle, asset: &VehicleMeshAsset) {
        let vertices = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vehicle_static_mesh_v"),
            contents: bytemuck::cast_slice(asset.vertices()),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vehicle_static_mesh_i"),
            contents: bytemuck::cast_slice(asset.indices()),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.meshes.insert(
            handle.0,
            GpuVehicleMesh {
                vertices,
                indices,
                vertex_count: asset.vertex_count() as u32,
                index_count: asset.index_count() as u32,
            },
        );
    }

    pub fn get(&self, handle: MeshHandle) -> Option<&GpuVehicleMesh> {
        self.meshes.get(&handle.0)
    }
}
