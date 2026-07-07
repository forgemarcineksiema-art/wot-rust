mod battlefield;
mod bystra;
mod bystra_cover;
mod bystra_features;
mod bystra_layout;
mod bystra_scenery;
mod chunk;
mod coordinates;
mod heightmap;
mod map_build;
mod map_id;
mod map_plan;
mod math;
mod prokhorovka;
mod prokhorovka_cover;
mod prokhorovka_features;
mod prokhorovka_layout;
mod scenery;
mod sculpt;
mod water;

pub use battlefield::{
    BattlefieldMap, MapFeature, MapFeatureKind, SpawnZone, StaticCoverKind, StaticCoverObject,
    StrategicPoint, StrategicRole,
};
pub use bystra::{RIVER_CORRIDOR_HALF_WIDTH_M, bystra_river_center_x, bystra_valley};
pub use chunk::{DEFAULT_CHUNK_SIZE_M, TerrainChunk, TerrainChunkId};
pub use coordinates::{CoordinatePrecision, LargeWorldStrategy, WorldCoordinatePolicy};
pub use heightmap::{HeightMap, HeightMapStats, TerrainError};
pub use map_id::MapId;
pub use map_plan::{TerrainMapLayer, TerrainMapPlan};
pub use prokhorovka::prokhorovka_hill_252_2;
pub use scenery::{SceneryInstance, SceneryKind};
pub use water::WaterBody;
