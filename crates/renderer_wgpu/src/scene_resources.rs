use std::collections::BTreeMap;

use renderer_api::{MaterialHandle, MeshAsset, MeshHandle, RenderFrame};
use wgpu::util::DeviceExt;

use crate::GpuContext;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SceneInstance {
    pub model: [[f32; 4]; 4],
    /// Per-instance team tint (rgb + unused w for 16-byte alignment).
    pub tint: [f32; 4],
}

impl SceneInstance {
    pub const fn identity() -> Self {
        Self {
            model: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            tint: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

pub struct GpuSceneMesh {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub index_count: u32,
}

#[derive(Default)]
pub struct SceneMeshRegistry {
    meshes: BTreeMap<u32, GpuSceneMesh>,
}

impl SceneMeshRegistry {
    pub fn register(&mut self, ctx: &GpuContext, handle: MeshHandle, asset: &MeshAsset) {
        let vertices = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_static_mesh_v"),
            contents: bytemuck::cast_slice(asset.vertices()),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_static_mesh_i"),
            contents: bytemuck::cast_slice(asset.indices()),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.meshes.insert(
            handle.0,
            GpuSceneMesh { vertices, indices, index_count: asset.index_count() as u32 },
        );
    }

    pub fn get(&self, handle: MeshHandle) -> Option<&GpuSceneMesh> {
        self.meshes.get(&handle.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneObjectDraw {
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
    pub instance_start: u32,
    pub instance_count: u32,
}

pub fn frame_instances(frame: &RenderFrame) -> (Vec<SceneInstance>, Vec<SceneObjectDraw>) {
    let mut batches = Vec::<SceneObjectBatch>::new();
    for object in &frame.objects {
        let [r, g, b] = object.tint;
        let instance = SceneInstance { model: object.transform, tint: [r, g, b, 1.0] };
        if let Some(batch) = batches
            .iter_mut()
            .find(|batch| batch.mesh == object.mesh && batch.material == object.material)
        {
            batch.instances.push(instance);
        } else {
            batches.push(SceneObjectBatch {
                mesh: object.mesh,
                material: object.material,
                instances: vec![instance],
            });
        }
    }

    let mut instances = Vec::with_capacity(frame.objects.len());
    let mut draws = Vec::with_capacity(batches.len());
    for batch in batches {
        let instance_start = instances.len() as u32;
        let instance_count = batch.instances.len() as u32;
        instances.extend(batch.instances);
        draws.push(SceneObjectDraw {
            mesh: batch.mesh,
            material: batch.material,
            instance_start,
            instance_count,
        });
    }
    (instances, draws)
}

struct SceneObjectBatch {
    mesh: MeshHandle,
    material: MaterialHandle,
    instances: Vec<SceneInstance>,
}

#[cfg(test)]
mod tests {
    use game_core::TankId;
    use renderer_api::{MaterialHandle, RenderObject};

    use super::*;

    #[test]
    fn frame_instances_batch_same_mesh_and_material_into_one_draw() {
        let frame = RenderFrame {
            objects: vec![object(1, 10, 2), object(2, 11, 2), object(3, 10, 2)],
            ..Default::default()
        };

        let (instances, draws) = frame_instances(&frame);

        assert_eq!(instances.len(), 3);
        assert_eq!(draws.len(), 2);
        assert_eq!(draws[0].mesh, MeshHandle(10));
        assert_eq!(draws[0].material, MaterialHandle(2));
        assert_eq!(draws[0].instance_start, 0);
        assert_eq!(draws[0].instance_count, 2);
        assert_eq!(draws[1].mesh, MeshHandle(11));
        assert_eq!(draws[1].material, MaterialHandle(2));
        assert_eq!(draws[1].instance_start, 2);
        assert_eq!(draws[1].instance_count, 1);
        assert_eq!(instances[0].tint[0], 1.0);
        assert_eq!(instances[1].tint[0], 3.0);
        assert_eq!(instances[2].tint[0], 2.0);
    }

    fn object(tank_id: u64, mesh: u32, material: u32) -> RenderObject {
        RenderObject {
            tank_id: Some(TankId(tank_id)),
            mesh: MeshHandle(mesh),
            material: MaterialHandle(material),
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [tank_id as f32, 0.0, 0.0, 1.0],
            ],
            tint: [tank_id as f32, 1.0, 1.0],
        }
    }
}
