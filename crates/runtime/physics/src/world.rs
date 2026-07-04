use game_core::ContactFootprint;
use glam::Vec3;
use terrain::{HeightMap, StaticCoverObject};

use crate::collision::{
    TankWorldObstacles, default_tank_footprint, resolve_tank_collision_with_velocity,
};
use crate::contact::{TerrainContact, sample_tank_terrain_contact};
use crate::controller_settings::TankControllerSettings;
use crate::cover::resolve_cover_collision_with_velocity;
use crate::hull_attitude::advance_hull_attitude;
use crate::movement::{
    TankControlInput, TankKinematicState, step_custom_tank_controller_on_contact,
};
use crate::track_contact::{sample_support, support_height};
use crate::vertical::{GroundStep, is_grounded, resolve_vertical};

/// Advance one tick on terrain only. Kept for callers without a cover set.
pub fn step_tank_on_heightmap(
    state: &mut TankKinematicState,
    input: TankControlInput,
    settings: &TankControllerSettings,
    heightmap: &HeightMap,
    dt_seconds: f32,
) -> GroundStep {
    step_tank_on_world(state, input, settings, heightmap, &[], dt_seconds)
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
) -> GroundStep {
    step_tank_on_world_with_tanks(
        state,
        input,
        settings,
        Some(heightmap),
        TankWorldObstacles::new(cover, default_tank_footprint(), &[]),
        None,
        dt_seconds,
    )
}

/// Advance one tick against optional terrain plus cover and other tanks. `heightmap` is `None`
/// for terrain-free modes (the hull stays on a flat plane); the server and the client predictor
/// share this so prediction stays in lockstep, terrain or not. With a `footprint`, the hull's
/// ride height comes from the running-gear support envelope (trench bridging, crest overhang —
/// see `track_contact`) instead of the centre probe; slopes and traction still read the probe
/// cross. The returned [`GroundStep`] says whether the hull ended the tick carried by the ground
/// and how hard a landing it absorbed.
#[allow(clippy::too_many_arguments)]
pub fn step_tank_on_world_with_tanks(
    state: &mut TankKinematicState,
    input: TankControlInput,
    settings: &TankControllerSettings,
    heightmap: Option<&HeightMap>,
    obstacles: TankWorldObstacles<'_>,
    footprint: Option<&ContactFootprint>,
    dt_seconds: f32,
) -> GroundStep {
    let previous = state.position;
    let ride_height = |position: Vec3, yaw_rad: f32| -> Option<f32> {
        let heightmap = heightmap?;
        if let Some(footprint) = footprint
            && let Some(height) = support_height(heightmap, position, yaw_rad, footprint)
        {
            return Some(height);
        }
        None
    };
    let mut contact = heightmap
        .and_then(|heightmap| {
            sample_tank_terrain_contact(
                heightmap,
                state.position,
                state.yaw_rad,
                settings.ground_probe_length_m,
            )
        })
        .unwrap_or_else(|| TerrainContact::flat(state.position.y));
    if let Some(height) = ride_height(state.position, state.yaw_rad) {
        contact.height_m = height;
    }
    let was_grounded = is_grounded(state.position.y, contact.height_m);

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

    // Vertical resolution against the post-collision ground: the terrain either carries the
    // hull (the kinematic follow) or lets it fly and later catches it (see `vertical`). A
    // grounded hull then rotates toward the support plane's attitude; an airborne hull keeps
    // the attitude it left the ground with.
    if let Some(heightmap) = heightmap
        && let Some(next_contact) = sample_tank_terrain_contact(
            heightmap,
            state.position,
            state.yaw_rad,
            settings.ground_probe_length_m,
        )
    {
        let support = footprint.and_then(|footprint| {
            sample_support(heightmap, state.position, state.yaw_rad, footprint)
        });
        let ground = support.map(|s| s.height_m).unwrap_or(next_contact.height_m);
        let moved_xz = (state.position.x - previous.x).hypot(state.position.z - previous.z);
        let step = resolve_vertical(state, ground, was_grounded, moved_xz, dt_seconds);
        if step.grounded {
            // Attitude targets: the support plane when the running gear is known, the probe
            // gradients otherwise (+forward_slope = rises ahead = nose up; +side_slope = right
            // higher = right side up — both match `game_core::math::hull_basis`).
            let (target_pitch, target_roll) = match support {
                Some(support) => (support.pitch_rad, support.roll_rad),
                None => (next_contact.forward_slope.atan(), next_contact.side_slope.atan()),
            };
            advance_hull_attitude(state, target_pitch, target_roll, dt_seconds);
        }
        return step;
    }
    // Terrain-free mode: level ground, so the attitude settles back to level.
    advance_hull_attitude(state, 0.0, 0.0, dt_seconds);
    GroundStep::resting()
}
