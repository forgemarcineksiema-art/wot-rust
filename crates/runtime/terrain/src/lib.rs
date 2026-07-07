mod battlefield;
mod chunk;
mod coordinates;
mod heightmap;
mod map_id;
mod map_plan;
mod math;
mod prokhorovka;
mod prokhorovka_cover;
mod prokhorovka_features;
mod prokhorovka_layout;

pub use battlefield::{
    BattlefieldMap, MapFeature, MapFeatureKind, SpawnZone, StaticCoverKind, StaticCoverObject,
    StrategicPoint, StrategicRole,
};
pub use chunk::{DEFAULT_CHUNK_SIZE_M, TerrainChunk, TerrainChunkId};
pub use coordinates::{CoordinatePrecision, LargeWorldStrategy, WorldCoordinatePolicy};
pub use heightmap::{HeightMap, HeightMapStats, TerrainError};
pub use map_id::MapId;
pub use map_plan::{TerrainMapLayer, TerrainMapPlan};
pub use prokhorovka::prokhorovka_hill_252_2;
