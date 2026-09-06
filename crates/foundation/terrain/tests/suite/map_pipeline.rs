use terrain::{
    CoordinatePrecision, LargeWorldStrategy, TerrainMapLayer, TerrainMapPlan, WorldCoordinatePolicy,
};

#[test]
fn wot_like_map_plan_declares_required_terrain_systems() {
    let plan = TerrainMapPlan::wot_like();

    for layer in [
        TerrainMapLayer::HeightmapChunks,
        TerrainMapLayer::CollisionTerrain,
        TerrainMapLayer::RenderLod,
        TerrainMapLayer::SplatMaps,
        TerrainMapLayer::Roads,
        TerrainMapLayer::Decorations,
        TerrainMapLayer::CoverObjects,
        TerrainMapLayer::SpawnPoints,
        TerrainMapLayer::CaptureZones,
        TerrainMapLayer::BotNavigation,
        TerrainMapLayer::VisibilitySectors,
        TerrainMapLayer::MinimapData,
    ] {
        assert!(plan.required_layers().contains(&layer), "missing terrain layer: {layer:?}");
    }
}

#[test]
fn coordinate_policy_uses_f32_for_normal_battle_maps_with_rebase_escape_hatch() {
    let policy = WorldCoordinatePolicy::wot_like_maps();

    assert_eq!(policy.simulation_precision(), CoordinatePrecision::F32);
    assert_eq!(policy.renderer_precision(), CoordinatePrecision::F32);
    assert_eq!(policy.large_world_strategy(), LargeWorldStrategy::OriginRebaseAboveMeters(4096.0));
    assert!(policy.fits_without_rebase(1000.0));
    assert!(!policy.fits_without_rebase(5000.0));
}
