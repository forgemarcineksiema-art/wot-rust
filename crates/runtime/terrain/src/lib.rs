mod battlefield;
mod bystra;
mod bystra_cover;
mod bystra_features;
mod bystra_layout;
mod bystra_scenery;
mod chunk;
mod coordinates;
mod craters;
mod heightmap;
mod map_build;
mod map_id;
mod map_plan;
mod math;
mod prokhorovka;
mod prokhorovka_cover;
mod prokhorovka_features;
mod prokhorovka_layout;
mod prokhorovka_scenery;
mod scenery;
mod sculpt;
mod water;

pub use battlefield::{
    BattlefieldMap, MapFeature, MapFeatureKind, Road, RoadSurface, SpawnZone, StaticCoverKind,
    StaticCoverObject, StrategicPoint, StrategicRole,
};
pub use bystra::{
    RIVER_CORRIDOR_HALF_WIDTH_M, bystra_backdrop_height, bystra_river_center_x, bystra_valley,
};
pub use chunk::{DEFAULT_CHUNK_SIZE_M, TerrainChunk, TerrainChunkId};
pub use coordinates::{CoordinatePrecision, LargeWorldStrategy, WorldCoordinatePolicy};
pub use craters::{
    COVER_SCAR_KIND_HIGH_EXPLOSIVE, COVER_SCAR_KIND_KINETIC, COVER_SCAR_RADIUS_STEP_M,
    CRATER_DEPTH_STEP_M, CRATER_INFLUENCE_FACTOR, CRATER_KIND_HIGH_EXPLOSIVE,
    CRATER_POSITION_STEP_M, CRATER_RADIUS_STEP_M, CRATER_RIM_FRACTION, CoverScar, CraterField,
    CraterRecord, MAX_COVER_SCARS_PER_COVER, he_crater_depth_m, he_crater_radius_m,
};
pub use heightmap::{HeightMap, HeightMapStats, TerrainError};
pub use map_id::MapId;
pub use map_plan::{TerrainMapLayer, TerrainMapPlan};
pub use prokhorovka::{prokhorovka_beyond_height, prokhorovka_hill_252_2};
pub use scenery::{SceneryInstance, SceneryKind};
pub use water::WaterBody;
