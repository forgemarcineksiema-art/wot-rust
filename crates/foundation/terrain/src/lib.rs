mod battlefield;
mod chunk;
mod coordinates;
mod craters;
mod ground;
mod heightmap;
mod map_build;
mod map_id;
mod map_plan;
mod math;
mod river;
mod rubble;
mod scenery;
mod sculpt;
mod water;

pub use battlefield::{
    BattlefieldMap, CaptureZone, MapFeature, MapFeatureKind, Road, RoadSurface, SpawnZone,
    StaticCoverKind, StaticCoverObject, StrategicPoint, StrategicRole, born_cover_phase_byte,
    initial_cover_phase_bytes,
};
pub use chunk::{DEFAULT_CHUNK_SIZE_M, TerrainChunk, TerrainChunkId};
pub use coordinates::{CoordinatePrecision, LargeWorldStrategy, WorldCoordinatePolicy};
pub use craters::{
    COVER_SCAR_KIND_HIGH_EXPLOSIVE, COVER_SCAR_KIND_KINETIC, COVER_SCAR_RADIUS_STEP_M,
    CRATER_DEPTH_STEP_M, CRATER_INFLUENCE_FACTOR, CRATER_KIND_HIGH_EXPLOSIVE,
    CRATER_POSITION_STEP_M, CRATER_RADIUS_STEP_M, CRATER_RIM_FRACTION, CoverScar, CraterField,
    CraterRecord, MAX_COVER_SCARS_PER_COVER, he_crater_depth_m, he_crater_radius_m,
};
pub use ground::{
    GroundClassifier, GroundMaterial, GroundProperties, grass_patchwork_noise, road_blend,
    road_blend_at, value_noise,
};
pub use heightmap::{HeightMap, HeightMapStats, TerrainError};
// The map compiler (`map_forge`) builds battlefields from blueprints through the SAME shared
// helpers — one grounding/grounding-math implementation, no forked copies.
pub use map_build::{
    ground_position, grounded_cover, grounded_feature, grounded_point, grounded_spawn_zone,
    heightmap_from_fn,
};
pub use map_id::MapId;
pub use map_plan::{TerrainMapLayer, TerrainMapPlan};
pub use math::{gaussian1, gaussian2};
pub use river::{RIVER_CORRIDOR_HALF_WIDTH_M, RiverSpec, bystra_river_center_x};
pub use rubble::{RUBBLE_REPOSE_GRADE, RubbleMound, ground_with_rubble, rubble_height_at};
pub use scenery::{
    ScatterRegion, SceneryInstance, SceneryKind, covers_containing, inside_any_cover,
    scatter_mirrored,
};
pub use sculpt::{band_mask, lerp, polyline_distance, smoothstep01};
pub use water::WaterBody;
