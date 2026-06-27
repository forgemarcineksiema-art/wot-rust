use renderer_wgpu::{
    DynamicUniformRingBuffer, FrameUploadArena, GpuReadbackQueue, InstanceBatchKind,
    InstanceBufferAllocator, TextureExtent, TextureFormat, TextureUploadQueue,
};

#[test]
fn frame_upload_arena_collects_large_batches_per_frame() {
    let mut arena = FrameUploadArena::new(1024);

    let first = arena.stage("tank_instances", &[1, 2, 3, 4]);
    let second = arena.stage("foliage_instances", &[5, 6, 7, 8, 9, 10]);

    assert_eq!(first.offset, 0);
    assert_eq!(second.offset, 4);
    assert_eq!(arena.pending_batches().len(), 2);
    assert_eq!(arena.pending_bytes(), 10);

    arena.reset_frame();

    assert!(arena.pending_batches().is_empty());
    assert_eq!(arena.pending_bytes(), 0);
}

#[test]
fn dynamic_uniform_ring_buffer_returns_aligned_offsets() {
    let mut ring = DynamicUniformRingBuffer::new(1024, 256);

    let camera = ring.allocate(64).expect("camera uniform allocation");
    let lights = ring.allocate(80).expect("lights uniform allocation");

    assert_eq!(camera.offset, 0);
    assert_eq!(lights.offset, 256);
    assert_eq!(camera.size, 64);
    assert_eq!(lights.size, 80);

    ring.begin_frame();
    assert_eq!(ring.allocate(64).expect("next frame allocation").offset, 0);
}

#[test]
fn instance_allocator_groups_draw_data_by_kind() {
    let mut allocator = InstanceBufferAllocator::default();

    allocator.push_instances(InstanceBatchKind::Tanks, 3, 48);
    allocator.push_instances(InstanceBatchKind::Tanks, 2, 48);
    allocator.push_instances(InstanceBatchKind::Foliage, 10, 32);
    allocator.push_instances(InstanceBatchKind::Particles, 64, 24);

    let tanks = allocator.batch(InstanceBatchKind::Tanks).expect("tank batch");
    assert_eq!(tanks.instance_count, 5);
    assert_eq!(tanks.byte_len, 240);
    assert_eq!(allocator.batch_count(), 3);
}

#[test]
fn texture_upload_queue_tracks_mip_safe_uploads() {
    let mut queue = TextureUploadQueue::default();

    let id = queue.enqueue_2d(
        "tank_albedo",
        TextureExtent { width: 512, height: 512, depth_or_layers: 1 },
        TextureFormat::Rgba8UnormSrgb,
        4,
        vec![255; 512 * 512 * 4],
    );

    let upload = queue.pending_uploads().iter().find(|item| item.id == id).unwrap();
    assert_eq!(upload.label, "tank_albedo");
    assert_eq!(upload.mip_level_count, 4);
    assert_eq!(upload.data.len(), 512 * 512 * 4);
}

#[test]
fn readback_queue_separates_requests_from_completed_results() {
    let mut queue = GpuReadbackQueue::default();

    let request = queue.request("frame_stats", 128);
    assert_eq!(queue.pending_requests().len(), 1);

    queue.complete(request, vec![7; 128]).expect("complete request");

    assert!(queue.pending_requests().is_empty());
    let result = queue.take_completed(request).expect("completed readback");
    assert_eq!(result.label, "frame_stats");
    assert_eq!(result.data.len(), 128);
}
