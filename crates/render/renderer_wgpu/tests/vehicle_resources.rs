use renderer_api::{MeshHandle, VehicleMeshAsset, VehicleVertex};
use renderer_wgpu::{GpuContext, VehicleMeshRegistry};

fn headless_context() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping vehicle resource test: {error}");
            None
        }
    }
}

#[test]
fn vehicle_mesh_registry_uploads_vehicle_vertex_buffers() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let mut registry = VehicleMeshRegistry::default();
    let handle = MeshHandle(77);
    let asset = VehicleMeshAsset::new(
        vec![
            VehicleVertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0], 0, 1.0),
            VehicleVertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0], 1, 1.0),
            VehicleVertex::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0], 3, 0.0),
        ],
        vec![0, 1, 2],
    );

    registry.register(&ctx, handle, &asset);
    let mesh = registry.get(handle).expect("registered vehicle mesh");

    assert_eq!(mesh.vertex_count, 3);
    assert_eq!(mesh.index_count, 3);
    assert_eq!(mesh.vertices.size(), (3 * std::mem::size_of::<VehicleVertex>()) as u64);
    assert_eq!(mesh.indices.size(), (3 * std::mem::size_of::<u32>()) as u64);
}
