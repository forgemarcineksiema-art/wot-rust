#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainMapLayer {
    HeightmapChunks,
    CollisionTerrain,
    RenderLod,
    SplatMaps,
    Roads,
    Decorations,
    CoverObjects,
    SpawnPoints,
    CaptureZones,
    BotNavigation,
    VisibilitySectors,
    MinimapData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainMapPlan;

impl TerrainMapPlan {
    pub fn wot_like() -> Self {
        Self
    }

    pub fn required_layers(self) -> [TerrainMapLayer; 12] {
        [
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
        ]
    }
}
