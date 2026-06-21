//! The single shell-collision implementation shared by the authoritative server step
//! ([`crate::combat::step_shells`]), the client's reticle ballistic preview, and the client's
//! straight aim-ray sweep. One trajectory + intersection routine means the reticle predicts the
//! exact impact the server will resolve, so a previewed hit is never one the server rejects.

mod cover;
mod tank;
mod terrain;

use ::terrain::{HeightMap, StaticCoverObject};
use game_core::math::GRAVITY_MPS2;
use game_core::{
    ArmorFacing, ArmorZone, HitboxProfile, ImpactSurface, MountFrames, TankId, TankSpec,
    VehicleKind,
};
use glam::Vec3;

/// Shells live at most this long before despawning (server) / terminating the preview trace.
pub const SHELL_MAX_AGE_SECONDS: f32 = 4.0;

/// Neutral tank hull for shell collision. The server builds these from `TankState`, the client
/// from `net::TankSnapshot`; both pre-split the battle into damageable targets and absorbing
/// blockers before tracing.
#[derive(Debug, Clone, Copy)]
pub struct TraceTank {
    pub id: TankId,
    pub position: Vec3,
    pub yaw_rad: f32,
    pub turret_yaw_rad: f32,
    pub hitbox: HitboxProfile,
    /// Local-space Z of the turret-ring axis the turret volume traverses about. Comes from the
    /// vehicle's `MountFrames` — use [`TraceTank::for_kind`] so it cannot desync from the hitbox.
    pub turret_ring_z_m: f32,
}

impl TraceTank {
    pub fn from_spec(
        id: TankId,
        position: Vec3,
        yaw_rad: f32,
        turret_yaw_rad: f32,
        spec: &TankSpec,
    ) -> Self {
        Self {
            id,
            position,
            yaw_rad,
            turret_yaw_rad,
            hitbox: spec.hitbox,
            turret_ring_z_m: spec.mounts.turret_ring.translation.z,
        }
    }

    /// Build a trace hull for a vehicle kind, sourcing the hitbox and the turret-ring pivot from
    /// `game_core` so the two cannot drift apart at call sites.
    pub fn for_kind(
        id: TankId,
        position: Vec3,
        yaw_rad: f32,
        turret_yaw_rad: f32,
        kind: VehicleKind,
    ) -> Self {
        Self {
            id,
            position,
            yaw_rad,
            turret_yaw_rad,
            hitbox: HitboxProfile::for_vehicle(kind),
            turret_ring_z_m: MountFrames::for_vehicle(kind).turret_ring.translation.z,
        }
    }
}

/// The world a shell segment is tested against. `tanks` are live enemies (hits resolve as
/// damage); `blockers` are hulls that absorb the shell without damage — wrecks and friendly
/// vehicles. The shell's owner belongs to neither slice.
#[derive(Debug, Clone, Copy)]
pub struct ShellTraceWorld<'a> {
    pub tanks: &'a [TraceTank],
    pub blockers: &'a [TraceTank],
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
    /// Absorbed without enemy damage; `surface` says by what (terrain, cover, or a hull).
    Obstacle { position: Vec3, surface: ImpactSurface },
}

impl SegmentImpact {
    pub fn point(self) -> Vec3 {
        match self {
            SegmentImpact::Tank { hit_position, .. } => hit_position,
            SegmentImpact::Obstacle { position, .. } => position,
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
    Obstacle {
        position: Vec3,
        surface: ImpactSurface,
    },
    Expired(Vec3),
}

impl TraceOutcome {
    pub fn impact_point(self) -> Vec3 {
        match self {
            TraceOutcome::Tank { hit_position, .. } => hit_position,
            TraceOutcome::Obstacle { position, .. } => position,
            TraceOutcome::Expired(point) => point,
        }
    }
}

/// First impact along a single segment `previous -> current` (the shell travels at `velocity`):
/// the nearest of enemy hull / blocker hull / terrain / cover. Ties resolve to the damageable
/// tank, matching the authoritative step.
pub fn segment_impact(
    previous: Vec3,
    current: Vec3,
    velocity: Vec3,
    world: &ShellTraceWorld<'_>,
) -> Option<SegmentImpact> {
    let tank = tank::first_tank_impact(previous, current, velocity, world.tanks);
    let obstacle = nearest_obstacle(previous, current, velocity, world);
    match (tank, obstacle) {
        (Some(tank), Some((position, _))) => {
            if tank.point().distance_squared(previous) <= position.distance_squared(previous) {
                Some(tank)
            } else {
                obstacle_impact(obstacle)
            }
        }
        (Some(tank), None) => Some(tank),
        (None, Some(_)) => obstacle_impact(obstacle),
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
            Some(SegmentImpact::Obstacle { position, surface }) => {
                return TraceOutcome::Obstacle { position, surface };
            }
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

/// The nearest absorbing obstacle on the segment: terrain, cover, or a blocker hull.
fn nearest_obstacle(
    previous: Vec3,
    current: Vec3,
    velocity: Vec3,
    world: &ShellTraceWorld<'_>,
) -> Option<(Vec3, ImpactSurface)> {
    let terrain = terrain::first_terrain_impact(previous, current, world.heightmap)
        .map(|point| (point, ImpactSurface::Terrain));
    let cover = cover::first_cover_impact(previous, current, world.cover)
        .map(|point| (point, ImpactSurface::Cover));
    let hull = tank::first_tank_impact(previous, current, velocity, world.blockers)
        .map(|impact| (impact.point(), ImpactSurface::Hull));
    [terrain, cover, hull].into_iter().flatten().min_by(|(a, _), (b, _)| {
        a.distance_squared(previous).total_cmp(&b.distance_squared(previous))
    })
}

fn obstacle_impact(obstacle: Option<(Vec3, ImpactSurface)>) -> Option<SegmentImpact> {
    obstacle.map(|(position, surface)| SegmentImpact::Obstacle { position, surface })
}
