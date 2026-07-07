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
    // Keyed batch lookup instead of a linear scan per object: with thousands of running-gear
    // instances over 100+ (mesh, material) batches the old find() was O(objects × batches)
    // every frame. First-seen batch order is preserved, so draws come out identical.
    let mut batch_index =
        std::collections::HashMap::<(u32, u32), usize>::with_capacity(frame.objects.len().min(256));
    for object in &frame.objects {
        let [r, g, b] = object.tint;
        let instance = SceneInstance { model: object.transform, tint: [r, g, b, 1.0] };
        let key = (object.mesh.0, object.material.0);
        match batch_index.get(&key) {
            Some(&index) => batches[index].instances.push(instance),
            None => {
                batch_index.insert(key, batches.len());
                batches.push(SceneObjectBatch {
                    mesh: object.mesh,
                    material: object.material,
                    instances: vec![instance],
                });
            }
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

/// Clip an instance upload to the buffer budget: keep the first `capacity` instances and trim the
/// draw list to match (draws past the cut are dropped, the boundary draw is shortened). Returns
/// whether anything was clipped. Losing the last-submitted objects keeps the frame LIVE — dropping
/// the whole oversized upload instead leaves the previous frame's instances on screen, freezing
/// every vehicle in its last uploaded pose.
pub fn clip_instances_to_capacity(
    instances: &mut Vec<SceneInstance>,
    draws: &mut Vec<SceneObjectDraw>,
    capacity: usize,
) -> bool {
    if instances.len() <= capacity {
        return false;
    }
    instances.truncate(capacity);
    draws.retain_mut(|draw| {
        let start = draw.instance_start as usize;
        if start >= capacity {
            return false;
        }
        draw.instance_count = draw.instance_count.min((capacity - start) as u32);
        true
    });
    true
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

    #[test]
    fn clipping_to_budget_shortens_the_boundary_draw_and_drops_the_rest() {
        let frame = RenderFrame {
            objects: vec![object(1, 10, 2), object(2, 10, 2), object(3, 11, 2), object(4, 12, 2)],
            ..Default::default()
        };
        let (mut instances, mut draws) = frame_instances(&frame);
        assert_eq!((instances.len(), draws.len()), (4, 3));

        let clipped = clip_instances_to_capacity(&mut instances, &mut draws, 3);

        assert!(clipped);
        assert_eq!(instances.len(), 3);
        // First draw (2 instances) survives whole, the boundary draw is shortened to what fits,
        // and the draw past the cut is gone — no draw may index instances beyond the upload.
        assert_eq!(draws.len(), 2);
        assert_eq!((draws[0].instance_start, draws[0].instance_count), (0, 2));
        assert_eq!((draws[1].instance_start, draws[1].instance_count), (2, 1));
        for draw in &draws {
            assert!((draw.instance_start + draw.instance_count) as usize <= instances.len());
        }
    }

    #[test]
    fn clipping_under_budget_is_a_no_op() {
        let frame =
            RenderFrame { objects: vec![object(1, 10, 2), object(2, 11, 2)], ..Default::default() };
        let (mut instances, mut draws) = frame_instances(&frame);

        assert!(!clip_instances_to_capacity(&mut instances, &mut draws, 2));
        assert_eq!(instances.len(), 2);
        assert_eq!(draws.len(), 2);
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
