use std::collections::BTreeMap;

use renderer_api::{MaterialHandle, MeshHandle, RenderFrame};

use crate::{InstanceBatchKind, InstanceBufferAllocator};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderObjectDraw {
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
    pub instance_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct RenderFrameBatchPlan {
    allocator: InstanceBufferAllocator,
    draws: Vec<RenderObjectDraw>,
}

impl RenderFrameBatchPlan {
    pub fn from_frame(frame: &RenderFrame, instance_stride: usize) -> Self {
        let mut allocator = InstanceBufferAllocator::default();
        let mut draws = BTreeMap::<(u32, u32), RenderObjectDraw>::new();

        for object in &frame.objects {
            let kind = if object.tank_id.is_some() {
                InstanceBatchKind::Tanks
            } else {
                InstanceBatchKind::Props
            };
            allocator.push_instances(kind, 1, instance_stride);

            let key = (object.mesh.0, object.material.0);
            draws.entry(key).and_modify(|draw| draw.instance_count += 1).or_insert(
                RenderObjectDraw {
                    mesh: object.mesh,
                    material: object.material,
                    instance_count: 1,
                },
            );
        }

        Self { allocator, draws: draws.into_values().collect() }
    }

    pub fn allocator(&self) -> &InstanceBufferAllocator {
        &self.allocator
    }

    pub fn draws(&self) -> &[RenderObjectDraw] {
        &self.draws
    }
}
