//! The single shell-collision implementation shared by the authoritative server step
//! ([`crate::combat::step_shells`]), the client's reticle ballistic preview, and the client's
//! straight aim-ray sweep. One trajectory + intersection routine means the reticle predicts the
//! exact impact the server will resolve, so a previewed hit is never one the server rejects.

mod cover;
mod legacy_boxes;
mod tank;
mod terrain;
mod types;

use ::terrain::HeightMap;
use game_core::ImpactSurface;
use game_core::math::integrate_shell_step;
use glam::Vec3;

pub use types::{SegmentImpact, ShellTraceWorld, TraceOutcome, TraceTank};

/// Shells live at most this long before despawning (server) / terminating the preview trace.
pub const SHELL_MAX_AGE_SECONDS: f32 = 4.0;

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

/// Integrate a ballistic arc from the muzzle (shared semi-implicit Euler: drag, gravity, move)
/// until it hits something or `max_age_seconds` elapses. Mirrors the authoritative per-tick
/// step exactly — `drag_per_s` comes from the previewed shell ([`game_core::ShellSpec::drag_per_s`]).
pub fn trace_shell(
    start_position: Vec3,
    start_velocity: Vec3,
    drag_per_s: f32,
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
        integrate_shell_step(&mut velocity, drag_per_s, dt_seconds);
        position += velocity * dt_seconds;
        age += dt_seconds;
        let segment_distance = position.distance(previous);

        match segment_impact(previous, position, velocity, world) {
            Some(SegmentImpact::Tank {
                id,
                facing,
                zone,
                impact_angle_degrees,
                hit_position,
                ..
            }) => {
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
