use game_core::ContactFootprint;
use glam::Vec3;
use terrain::{HeightMap, StaticCoverObject};

use crate::collision::{TankWorldObstacles, default_tank_footprint};
use crate::contact::{TerrainContact, sample_tank_terrain_contact};
use crate::controller_settings::TankControllerSettings;
use crate::cover::{footprint_blocked_by_cover, resolve_cover_collision_with_velocity};
use crate::hull_attitude::advance_hull_attitude;
use crate::movement::{
    TankControlInput, TankKinematicState, step_custom_tank_controller_on_contact,
};
use crate::tank_resolve::{footprint_blocked_by_tanks, resolve_tank_collision_with_velocity};
use crate::track_contact::{sample_support, support_height};
use crate::vertical::{GroundStep, is_grounded, resolve_vertical};

/// How far inside the heightmap the arena's red line stands. Chosen just past the ground
/// probe reach (`ground_probe_length_m` = 3.0) so a hull held at the line still reads real
/// terrain under every probe — the contact model never degrades against the wall.
pub const MAP_BORDER_MARGIN_M: f32 = 3.5;

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
    // Standing water over the contact: wading drag enters the force model through the contact,
    // and the riverbed softens the tracks. Zero depth (or a dry map) leaves both untouched.
    if let Some(water) = obstacles.water {
        contact.water_depth_m = water.depth_over(contact.height_m);
        contact.traction = crate::water::wading_traction(contact.traction, contact.water_depth_m);
    }
    let was_grounded = is_grounded(state.position.y, contact.height_m);

    let previous_yaw = state.yaw_rad;
    step_custom_tank_controller_on_contact(state, input, settings, contact, dt_seconds);
    // Rotation is collision-resolved like translation: a pivot that would grind the hull's
    // corners INTO a wall or a neighbour is refused (yaw reverts, the rotation rate dies),
    // exactly as a blocked move holds its position. Tested at the PREVIOUS position so pure
    // rotation is judged on its own; and only when the OLD yaw was clear — a hull already
    // overlapped (spawn accident, pileup) keeps its freedom to rotate its way out.
    {
        let fp = obstacles.tank_footprint;
        let blocked = |yaw: f32| {
            footprint_blocked_by_cover(previous, yaw, fp, obstacles.cover)
                || footprint_blocked_by_tanks(previous, yaw, fp, obstacles.tanks)
        };
        if state.yaw_rad != previous_yaw && blocked(state.yaw_rad) && !blocked(previous_yaw) {
            state.yaw_rad = previous_yaw;
            state.yaw_rate_rad_s = 0.0;
        }
    }
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

    // The arena's red line: the map ends where the heightmap ends. Without this the stepper
    // fell into terrain-free mode beyond the border and a tank could drive off the world onto
    // the render-only backdrop. Same contract as a cover wall — the position holds, the
    // velocity component INTO the wall dies, sliding along it survives. Server and client
    // predictor share this step, so prediction stays in lockstep at the line.
    if let Some(heightmap) = heightmap {
        let [extent_x, extent_z] = heightmap.extent_m();
        let margin = MAP_BORDER_MARGIN_M;
        if state.position.x < margin {
            state.position.x = margin;
            state.velocity.x = state.velocity.x.max(0.0);
        } else if state.position.x > extent_x - margin {
            state.position.x = extent_x - margin;
            state.velocity.x = state.velocity.x.min(0.0);
        }
        if state.position.z < margin {
            state.position.z = margin;
            state.velocity.z = state.velocity.z.max(0.0);
        } else if state.position.z > extent_z - margin {
            state.position.z = extent_z - margin;
            state.velocity.z = state.velocity.z.min(0.0);
        }
    }

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
