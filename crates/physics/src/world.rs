use terrain::{HeightMap, StaticCoverObject};

use crate::collision::{
    TankWorldObstacles, default_tank_footprint, resolve_cover_collision_with_speed,
    resolve_tank_collision_with_speed,
};
use crate::controller_settings::TankControllerSettings;
use crate::movement::{
    TankControlInput, TankKinematicState, TerrainContact, sample_tank_terrain_contact,
    step_custom_tank_controller_on_contact,
};

/// Advance one tick on terrain only. Kept for callers without a cover set.
pub fn step_tank_on_heightmap(
    state: &mut TankKinematicState,
    input: TankControlInput,
    settings: &TankControllerSettings,
    heightmap: &HeightMap,
    dt_seconds: f32,
) {
    step_tank_on_world(state, input, settings, heightmap, &[], dt_seconds);
}

/// Advance one tick on terrain plus static cover: the controller moves the hull, cover
/// collision keeps it out of cover footprints, then the hull is grounded to the terrain. The
/// sim and the client predictor share this so prediction stays in lockstep with the server
/// even against cover.
pub fn step_tank_on_world(
    state: &mut TankKinematicState,
    input: TankControlInput,
    settings: &TankControllerSettings,
    heightmap: &HeightMap,
    cover: &[StaticCoverObject],
    dt_seconds: f32,
) {
    step_tank_on_world_with_tanks(
        state,
        input,
        settings,
        Some(heightmap),
        TankWorldObstacles::new(cover, default_tank_footprint(), &[]),
        dt_seconds,
    );
}

/// Advance one tick against optional terrain plus cover and other tanks. `heightmap` is `None`
/// for terrain-free modes (the hull stays on a flat plane); the server and the client predictor
/// share this so prediction stays in lockstep, terrain or not.
pub fn step_tank_on_world_with_tanks(
    state: &mut TankKinematicState,
    input: TankControlInput,
    settings: &TankControllerSettings,
    heightmap: Option<&HeightMap>,
    obstacles: TankWorldObstacles<'_>,
    dt_seconds: f32,
) {
    let previous = state.position;
    let contact = heightmap
        .and_then(|heightmap| {
            sample_tank_terrain_contact(
                heightmap,
                state.position,
                state.yaw_rad,
                settings.ground_probe_length_m,
            )
        })
        .unwrap_or_else(|| TerrainContact::flat(state.position.y));

    step_custom_tank_controller_on_contact(state, input, settings, contact, dt_seconds);
    let (position, speed) = resolve_cover_collision_with_speed(
        previous,
        state.position,
        state.yaw_rad,
        state.forward_speed_mps,
        obstacles.cover,
        dt_seconds,
    );
    state.position = position;
    state.forward_speed_mps = speed;

    let (position, speed) = resolve_tank_collision_with_speed(
        previous,
        state.position,
        state.yaw_rad,
        state.forward_speed_mps,
        obstacles.tank_footprint,
        obstacles.tanks,
        dt_seconds,
    );
    state.position = position;
    state.forward_speed_mps = speed;

    if let Some(heightmap) = heightmap
        && let Some(next_contact) = sample_tank_terrain_contact(
            heightmap,
            state.position,
            state.yaw_rad,
            settings.ground_probe_length_m,
        )
    {
        state.position.y = next_contact.height_m;
    }
}
