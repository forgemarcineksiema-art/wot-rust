use std::f32::consts::PI;

use crate::bystra::{
    FORD_OFFSET_M, HALF_M, KNOLL_OFFSET_M, KNOLL_X_M, PLANK_OFFSET_M, QUARRY_X_M,
    RIDGE_PERCH_OFFSET_M, RIDGE_PERCH_X_M, TOWN_CENTER_X_M, WINDMILL_SHELF_OFFSET_M,
    WINDMILL_SHELF_X_M, WINDMILL_X_M, bystra_river_center_x,
};
use crate::map_build::{grounded_point, grounded_spawn_zone};
use crate::{HeightMap, SpawnZone, StrategicPoint, StrategicRole};

/// Spawns sit on the western fields, mirrored across the axis: each team deploys with the
/// plank crossing at its shoulder (fast town rotation) and the open flank ahead — both banks
/// are equally reachable from either side, which is the point of running the river ALONG the
/// axis of advance.
pub(crate) fn valley_spawn_zones(heightmap: &HeightMap) -> Vec<SpawnZone> {
    vec![
        grounded_spawn_zone(heightmap, 1, 400.0, HALF_M - 350.0, 0.0),
        grounded_spawn_zone(heightmap, 2, 400.0, HALF_M + 350.0, PI),
    ]
}

pub(crate) fn valley_strategic_points(heightmap: &HeightMap) -> Vec<StrategicPoint> {
    let bridge_x = bystra_river_center_x(HALF_M);
    let ford_south_z = HALF_M - FORD_OFFSET_M;
    let ford_north_z = HALF_M + FORD_OFFSET_M;
    let plank_south_z = HALF_M - PLANK_OFFSET_M;
    let plank_north_z = HALF_M + PLANK_OFFSET_M;
    vec![
        grounded_point(
            heightmap,
            "windmill_hill",
            "Windmill Hill crest",
            StrategicRole::HighGround,
            WINDMILL_X_M,
            HALF_M,
            70.0,
        ),
        grounded_point(
            heightmap,
            "ridge_perch_south",
            "quarry ridge perch (south)",
            StrategicRole::HighGround,
            RIDGE_PERCH_X_M,
            HALF_M - RIDGE_PERCH_OFFSET_M,
            55.0,
        ),
        grounded_point(
            heightmap,
            "ridge_perch_north",
            "quarry ridge perch (north)",
            StrategicRole::HighGround,
            RIDGE_PERCH_X_M,
            HALF_M + RIDGE_PERCH_OFFSET_M,
            55.0,
        ),
        grounded_point(
            heightmap,
            "stone_bridge",
            "Kamienna stone bridge",
            StrategicRole::Crossing,
            bridge_x,
            HALF_M,
            45.0,
        ),
        grounded_point(
            heightmap,
            "ford_south",
            "southern ford",
            StrategicRole::Crossing,
            bystra_river_center_x(ford_south_z),
            ford_south_z,
            40.0,
        ),
        grounded_point(
            heightmap,
            "ford_north",
            "northern ford",
            StrategicRole::Crossing,
            bystra_river_center_x(ford_north_z),
            ford_north_z,
            40.0,
        ),
        grounded_point(
            heightmap,
            "plank_crossing_south",
            "southern plank crossing",
            StrategicRole::Crossing,
            bystra_river_center_x(plank_south_z),
            plank_south_z,
            35.0,
        ),
        grounded_point(
            heightmap,
            "plank_crossing_north",
            "northern plank crossing",
            StrategicRole::Crossing,
            bystra_river_center_x(plank_north_z),
            plank_north_z,
            35.0,
        ),
        grounded_point(
            heightmap,
            "windmill_hulldown_south",
            "Windmill Hill hull-down shelf (south)",
            StrategicRole::HullDown,
            WINDMILL_SHELF_X_M,
            HALF_M - WINDMILL_SHELF_OFFSET_M,
            35.0,
        ),
        grounded_point(
            heightmap,
            "windmill_hulldown_north",
            "Windmill Hill hull-down shelf (north)",
            StrategicRole::HullDown,
            WINDMILL_SHELF_X_M,
            HALF_M + WINDMILL_SHELF_OFFSET_M,
            35.0,
        ),
        grounded_point(
            heightmap,
            "knoll_overwatch_south",
            "field knoll overwatch (south)",
            StrategicRole::Observation,
            KNOLL_X_M,
            HALF_M - KNOLL_OFFSET_M,
            40.0,
        ),
        grounded_point(
            heightmap,
            "knoll_overwatch_north",
            "field knoll overwatch (north)",
            StrategicRole::Observation,
            KNOLL_X_M,
            HALF_M + KNOLL_OFFSET_M,
            40.0,
        ),
        grounded_point(
            heightmap,
            "market_square",
            "Kamienna market square",
            StrategicRole::Observation,
            TOWN_CENTER_X_M,
            HALF_M,
            50.0,
        ),
        grounded_point(
            heightmap,
            "quarry_bowl",
            "quarry rotation bowl",
            StrategicRole::FlankRoute,
            QUARRY_X_M,
            HALF_M,
            50.0,
        ),
        grounded_point(
            heightmap,
            "field_lane_south",
            "western field lane (south)",
            StrategicRole::FlankRoute,
            180.0,
            HALF_M - 150.0,
            60.0,
        ),
        grounded_point(
            heightmap,
            "field_lane_north",
            "western field lane (north)",
            StrategicRole::FlankRoute,
            180.0,
            HALF_M + 150.0,
            60.0,
        ),
    ]
}
