use std::f32::consts::PI;

use crate::prokhorovka::{
    HALF_M, HILL_OFFSET_M, HILL_X_M, OVERWATCH_OFFSET_M, OVERWATCH_X_M, SHELF_X_M,
};
use crate::{HeightMap, SpawnZone, StrategicPoint, StrategicRole};

pub(crate) fn spawn_zones(heightmap: &HeightMap) -> Vec<SpawnZone> {
    // Mirror pair across the central axis: the south team faces +z (north), the north team
    // faces -z (south), so both look down the same approach to the embankment.
    vec![
        spawn_zone(heightmap, 1, HALF_M, HALF_M - 350.0, 0.0),
        spawn_zone(heightmap, 2, HALF_M, HALF_M + 350.0, PI),
    ]
}

pub(crate) fn strategic_points(heightmap: &HeightMap) -> Vec<StrategicPoint> {
    vec![
        point(
            heightmap,
            "hill_252_2_south",
            "Hill 252.2 crest (south)",
            StrategicRole::HighGround,
            HILL_X_M,
            HALF_M - HILL_OFFSET_M,
            70.0,
        ),
        point(
            heightmap,
            "hill_252_2_north",
            "Hill 252.2 crest (north)",
            StrategicRole::HighGround,
            HILL_X_M,
            HALF_M + HILL_OFFSET_M,
            70.0,
        ),
        point(
            heightmap,
            "oktyabrskiy",
            "Oktyabrskiy farm rise",
            StrategicRole::Observation,
            HALF_M,
            HALF_M,
            55.0,
        ),
        point(
            heightmap,
            "rail_crossing_west",
            "western railway crossing",
            StrategicRole::Crossing,
            HALF_M - 250.0,
            HALF_M,
            45.0,
        ),
        point(
            heightmap,
            "rail_crossing_east",
            "eastern railway crossing",
            StrategicRole::Crossing,
            HALF_M + 250.0,
            HALF_M,
            45.0,
        ),
        point(
            heightmap,
            "psel_field_south",
            "Psel open flank (south)",
            StrategicRole::FlankRoute,
            120.0,
            HALF_M - 150.0,
            65.0,
        ),
        point(
            heightmap,
            "psel_field_north",
            "Psel open flank (north)",
            StrategicRole::FlankRoute,
            120.0,
            HALF_M + 150.0,
            65.0,
        ),
        point(
            heightmap,
            "psel_overwatch_south",
            "Psel field overwatch (south)",
            StrategicRole::Observation,
            OVERWATCH_X_M,
            HALF_M - OVERWATCH_OFFSET_M,
            40.0,
        ),
        point(
            heightmap,
            "psel_overwatch_north",
            "Psel field overwatch (north)",
            StrategicRole::Observation,
            OVERWATCH_X_M,
            HALF_M + OVERWATCH_OFFSET_M,
            40.0,
        ),
        point(
            heightmap,
            "hill_hulldown_south",
            "Hill 252.2 hull-down shelf (south)",
            StrategicRole::HullDown,
            SHELF_X_M,
            HALF_M - HILL_OFFSET_M,
            35.0,
        ),
        point(
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

fn spawn_zone(heightmap: &HeightMap, team: u16, x: f32, z: f32, facing_yaw_rad: f32) -> SpawnZone {
    SpawnZone { team, center: position(heightmap, x, z), radius_m: 55.0, facing_yaw_rad }
}

fn point(
    heightmap: &HeightMap,
    id: &str,
    name: &str,
    role: StrategicRole,
    x: f32,
    z: f32,
    radius_m: f32,
) -> StrategicPoint {
    StrategicPoint {
        id: id.to_string(),
        name: name.to_string(),
        role,
        position: position(heightmap, x, z),
        radius_m,
    }
}

pub(crate) fn position(heightmap: &HeightMap, x: f32, z: f32) -> [f32; 3] {
    [x, heightmap.sample_height(x, z).unwrap_or(0.0), z]
}
