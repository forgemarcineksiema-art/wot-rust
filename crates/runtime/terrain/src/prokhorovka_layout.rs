use std::f32::consts::PI;

use crate::map_build::{grounded_point, grounded_spawn_zone};
use crate::prokhorovka::{
    HALF_M, HILL_OFFSET_M, HILL_X_M, OVERWATCH_OFFSET_M, OVERWATCH_X_M, SHELF_X_M,
};
use crate::{HeightMap, SpawnZone, StrategicPoint, StrategicRole};

pub(crate) fn spawn_zones(heightmap: &HeightMap) -> Vec<SpawnZone> {
    // Mirror pair across the central axis: the south team faces +z (north), the north team
    // faces -z (south), so both look down the same approach to the embankment.
    vec![
        grounded_spawn_zone(heightmap, 1, HALF_M, HALF_M - 350.0, 0.0),
        grounded_spawn_zone(heightmap, 2, HALF_M, HALF_M + 350.0, PI),
    ]
}

pub(crate) fn strategic_points(heightmap: &HeightMap) -> Vec<StrategicPoint> {
    vec![
        grounded_point(
            heightmap,
            "hill_252_2_south",
            "Hill 252.2 crest (south)",
            StrategicRole::HighGround,
            HILL_X_M,
            HALF_M - HILL_OFFSET_M,
            70.0,
        ),
        grounded_point(
            heightmap,
            "hill_252_2_north",
            "Hill 252.2 crest (north)",
            StrategicRole::HighGround,
            HILL_X_M,
            HALF_M + HILL_OFFSET_M,
            70.0,
        ),
        grounded_point(
            heightmap,
            "oktyabrskiy",
            "Oktyabrskiy farm rise",
            StrategicRole::Observation,
            HALF_M,
            HALF_M,
            55.0,
        ),
        grounded_point(
            heightmap,
            "rail_crossing_west",
            "western railway crossing",
            StrategicRole::Crossing,
            HALF_M - 250.0,
            HALF_M,
            45.0,
        ),
        grounded_point(
            heightmap,
            "rail_crossing_east",
            "eastern railway crossing",
            StrategicRole::Crossing,
            HALF_M + 250.0,
            HALF_M,
            45.0,
        ),
        grounded_point(
            heightmap,
            "psel_field_south",
            "Psel open flank (south)",
            StrategicRole::FlankRoute,
            120.0,
            HALF_M - 150.0,
            65.0,
        ),
        grounded_point(
            heightmap,
            "psel_field_north",
            "Psel open flank (north)",
            StrategicRole::FlankRoute,
            120.0,
            HALF_M + 150.0,
            65.0,
        ),
        grounded_point(
            heightmap,
            "psel_overwatch_south",
            "Psel field overwatch (south)",
            StrategicRole::Observation,
            OVERWATCH_X_M,
            HALF_M - OVERWATCH_OFFSET_M,
            40.0,
        ),
        grounded_point(
            heightmap,
            "psel_overwatch_north",
            "Psel field overwatch (north)",
            StrategicRole::Observation,
            OVERWATCH_X_M,
            HALF_M + OVERWATCH_OFFSET_M,
            40.0,
        ),
        grounded_point(
            heightmap,
            "hill_hulldown_south",
            "Hill 252.2 hull-down shelf (south)",
            StrategicRole::HullDown,
            SHELF_X_M,
            HALF_M - HILL_OFFSET_M,
            35.0,
        ),
        grounded_point(
            heightmap,
            "hill_hulldown_north",
            "Hill 252.2 hull-down shelf (north)",
            StrategicRole::HullDown,
            SHELF_X_M,
            HALF_M + HILL_OFFSET_M,
            35.0,
        ),
    ]
}
