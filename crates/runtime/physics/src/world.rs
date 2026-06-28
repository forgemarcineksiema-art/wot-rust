use terrain::{HeightMap, StaticCoverObject};

use crate::collision::{
    TankWorldObstacles, default_tank_footprint, resolve_tank_collision_with_velocity,
};
use crate::contact::{TerrainContact, sample_tank_terrain_contact};
use crate::controller_settings::TankControllerSettings;
use crate::cover::resolve_cover_collision_with_velocity;
use crate::movement::{
    TankControlInput, TankKinematicState, step_custom_tank_controller_on_contact,
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
    let (position, velocity) = resolve_cover_collision_with_velocity(
        previous,
        state.position,
        state.yaw_rad,
        state.velocity,
        obstacles.tank_footprint,
        obstacles.cover,
    );
    state.position = position;
    state.velocity = velocity;

    let (position, velocity) = resolve_tank_collision_with_velocity(
        previous,
        state.position,
        state.yaw_rad,
        state.velocity,
        obstacles.tank_footprint,
        obstacles.tanks,
    );
    state.position = position;
    state.velocity = velocity;

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
