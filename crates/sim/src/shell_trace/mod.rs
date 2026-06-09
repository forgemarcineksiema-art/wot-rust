//! The single shell-collision implementation shared by the authoritative server step
//! ([`crate::combat::step_shells`]), the client's reticle ballistic preview, and the client's
//! straight aim-ray sweep. One trajectory + intersection routine means the reticle predicts the
//! exact impact the server will resolve, so a previewed hit is never one the server rejects.

mod cover;
mod tank;
mod terrain;

use ::terrain::{HeightMap, StaticCoverObject};
use game_core::math::GRAVITY_MPS2;
use game_core::{ArmorFacing, ArmorZone, HitboxProfile, TankId};
use glam::Vec3;

/// Shells live at most this long before despawning (server) / terminating the preview trace.
pub const SHELL_MAX_AGE_SECONDS: f32 = 4.0;

/// Neutral tank target for shell collision. The server builds these from `TankState`, the client
/// from `net::TankSnapshot`; both pre-filter the slice (owner / dead / friendly) before tracing.
#[derive(Debug, Clone, Copy)]
pub struct TraceTank {
    pub id: TankId,
    pub position: Vec3,
    pub yaw_rad: f32,
    pub turret_yaw_rad: f32,
    pub hitbox: HitboxProfile,
}

/// The static + dynamic world a shell segment is tested against.
#[derive(Debug, Clone, Copy)]
pub struct ShellTraceWorld<'a> {
    pub tanks: &'a [TraceTank],
    pub heightmap: Option<&'a HeightMap>,
    pub cover: &'a [StaticCoverObject],
}

/// First thing a single shell segment hits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SegmentImpact {
    Tank {
        id: TankId,
        facing: ArmorFacing,
        zone: ArmorZone,
        impact_angle_degrees: f32,
        hit_position: Vec3,
    },
    Obstacle(Vec3),
}

impl SegmentImpact {
    pub fn point(self) -> Vec3 {
        match self {
            SegmentImpact::Tank { hit_position, .. } => hit_position,
            SegmentImpact::Obstacle(point) => point,
        }
    }
}

/// Where a full ballistic trace ended. `Expired` carries the final position so a shot into open
/// space still yields an aim point for the reticle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TraceOutcome {
    Tank {
        id: TankId,
        facing: ArmorFacing,
        zone: ArmorZone,
        impact_angle_degrees: f32,
        hit_position: Vec3,
        distance_m: f32,
    },
    Obstacle(Vec3),
    Expired(Vec3),
}

impl TraceOutcome {
    pub fn impact_point(self) -> Vec3 {
        match self {
            TraceOutcome::Tank { hit_position, .. } => hit_position,
            TraceOutcome::Obstacle(point) | TraceOutcome::Expired(point) => point,
        }
    }
}

/// First impact along a single segment `previous -> current` (the shell travels at `velocity`),
/// nearest of tank / terrain / cover. Ties resolve to the tank, matching the authoritative step.
pub fn segment_impact(
    previous: Vec3,
    current: Vec3,
    velocity: Vec3,
    world: &ShellTraceWorld<'_>,
) -> Option<SegmentImpact> {
    let tank = tank::first_tank_impact(previous, current, velocity, world.tanks);
    let terrain = terrain::first_terrain_impact(previous, current, world.heightmap);
    let cover = cover::first_cover_impact(previous, current, world.cover);
    let obstacle = nearer_point(previous, terrain, cover);
    match (tank, obstacle) {
        (Some(tank), Some(obstacle)) => {
            if tank.point().distance_squared(previous) <= obstacle.distance_squared(previous) {
                Some(tank)
            } else {
                Some(SegmentImpact::Obstacle(obstacle))
            }
        }
        (Some(tank), None) => Some(tank),
        (None, Some(obstacle)) => Some(SegmentImpact::Obstacle(obstacle)),
        (None, None) => None,
    }
}

/// Integrate a ballistic arc from the muzzle (semi-implicit Euler: gravity, then move) until it
/// hits something or `max_age_seconds` elapses. Mirrors the authoritative per-tick step exactly.
pub fn trace_shell(
    start_position: Vec3,
    start_velocity: Vec3,
    dt_seconds: f32,
    max_age_seconds: f32,
    world: &ShellTraceWorld<'_>,
) -> TraceOutcome {
    let mut position = start_position;
    let mut velocity = start_velocity;
    let mut age = 0.0;
    let mut travelled = 0.0;

    loop {
        let previous = position;
        velocity.y -= GRAVITY_MPS2 * dt_seconds;
        position += velocity * dt_seconds;
        age += dt_seconds;
        let segment_distance = position.distance(previous);

        match segment_impact(previous, position, velocity, world) {
            Some(SegmentImpact::Tank { id, facing, zone, impact_angle_degrees, hit_position }) => {
                return TraceOutcome::Tank {
                    id,
                    facing,
                    zone,
                    impact_angle_degrees,
                    hit_position,
                    distance_m: travelled + hit_position.distance(previous),
                };
            }
            Some(SegmentImpact::Obstacle(point)) => return TraceOutcome::Obstacle(point),
            None => {}
        }

        if ground_contact(position, world.heightmap) || age >= max_age_seconds {
            return TraceOutcome::Expired(position);
        }
        travelled += segment_distance;
    }
}

/// True once a shell has fallen to or below the terrain surface beneath it.
pub fn ground_contact(position: Vec3, heightmap: Option<&HeightMap>) -> bool {
    heightmap
        .and_then(|map| map.sample_height(position.x, position.z))
        .is_some_and(|ground| position.y <= ground)
}

/// The obstacle hit nearer to `origin`, merging the terrain and cover sweeps.
fn nearer_point(origin: Vec3, a: Option<Vec3>, b: Option<Vec3>) -> Option<Vec3> {
    match (a, b) {
        (Some(a), Some(b)) => {
            if a.distance_squared(origin) <= b.distance_squared(origin) {
                Some(a)
            } else {
                Some(b)
            }
        }
        (first, second) => first.or(second),
    }
}
