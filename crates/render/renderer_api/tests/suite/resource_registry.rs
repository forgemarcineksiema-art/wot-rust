use renderer_api::{
    MaterialDescriptor, MeshAsset, MeshRegistry, RenderMaterialRegistry, SceneVertex,
};

#[test]
fn mesh_registry_assigns_stable_handles_to_static_meshes() {
    let mut registry = MeshRegistry::default();
    let asset = MeshAsset::new(
        vec![
            SceneVertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.3, 0.4, 0.3]),
            SceneVertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.3, 0.4, 0.3]),
            SceneVertex::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.3, 0.4, 0.3]),
        ],
        vec![0, 1, 2],
    );

    let first = registry.register("t54_hull_extra", asset.clone());
    let second = registry.register("t54_hull_extra", asset);

    assert_eq!(first, second);
    assert_eq!(registry.mesh(first).expect("registered mesh").index_count(), 3);
    assert_eq!(registry.mesh_label(first), Some("t54_hull_extra"));
}

#[test]
fn material_registry_deduplicates_by_descriptor() {
    let mut registry = RenderMaterialRegistry::default();
    let descriptor = MaterialDescriptor::new("soviet_green", [0.30, 0.40, 0.28]);

    let first = registry.register(descriptor.clone());
    let second = registry.register(descriptor);

    assert_eq!(first, second);
    assert_eq!(
        registry.material(first).expect("registered material").base_color,
        [0.30, 0.40, 0.28]
    );
}
