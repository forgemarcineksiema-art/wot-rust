use game_core::TankId;
use renderer_api::{MaterialHandle, MeshHandle, RenderFrame, RenderObject};
use renderer_wgpu::{InstanceBatchKind, RenderFrameBatchPlan};

#[test]
fn render_frame_batch_plan_groups_tank_objects_for_instancing() {
    let frame = RenderFrame {
        objects: vec![object(1, 10, 2), object(1, 11, 2), object(1, 12, 2), object(2, 10, 2)],
        ..Default::default()
    };

    let plan = RenderFrameBatchPlan::from_frame(&frame, 64);

    let tank_batch = plan.allocator().batch(InstanceBatchKind::Tanks).expect("tank instance batch");
    assert_eq!(tank_batch.instance_count, 4);
    assert_eq!(tank_batch.byte_len, 4 * 64);

    assert_eq!(plan.draws().len(), 3, "same mesh/material pairs should share draw batches");
    assert_eq!(plan.draws()[0].instance_count, 2);
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
            [0.0, 0.0, 0.0, 1.0],
        ],
        tint: [1.0, 1.0, 1.0],
    }
}
