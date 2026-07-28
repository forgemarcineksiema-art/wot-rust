//! The single shell-collision implementation shared by the authoritative server step
//! ([`crate::combat::step_shells`]), the client's reticle ballistic preview, and the client's
//! straight aim-ray sweep. One trajectory + intersection routine means the reticle predicts the
//! exact impact the server will resolve, so a previewed hit is never one the server rejects.

mod cover;
mod legacy_boxes;
mod tank;
mod terrain;
mod types;
mod water;

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
    let radius_m = world.projectile_radius_m.max(0.0);
    let tank = tank::first_tank_impact(previous, current, velocity, world.tanks, radius_m);
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
    // A step that does not advance never terminates: with `dt_seconds` at zero (or NaN) the
    // shell neither moves nor ages, and the loop below has no other exit. The authoritative
    // step is always fed a real tick, but this routine is also driven by the client's reticle
    // preview and its gun-elevation solver from caller-supplied timesteps — one paused clock
    // there would hang the frame rather than mispredict it.
    if !dt_seconds.is_finite() || dt_seconds <= 0.0 {
        return TraceOutcome::Expired(start_position);
    }
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
    let radius_m = world.projectile_radius_m.max(0.0);
    let terrain = terrain::first_terrain_impact(previous, current, world.heightmap, radius_m)
        .map(|point| (point, ImpactSurface::Terrain));
    let cover = cover::first_cover_impact(previous, current, world.cover, radius_m)
        .map(|point| (point, ImpactSurface::Cover));
    let hull = tank::first_tank_impact(previous, current, velocity, world.blockers, radius_m)
        .map(|impact| (impact.point(), ImpactSurface::Hull));
    let water =
        water::first_water_impact(previous, current, world.heightmap, world.water, radius_m)
            .map(|point| (point, ImpactSurface::Water));
    [terrain, cover, hull, water].into_iter().flatten().min_by(|(a, _), (b, _)| {
        a.distance_squared(previous).total_cmp(&b.distance_squared(previous))
    })
}

fn obstacle_impact(obstacle: Option<(Vec3, ImpactSurface)>) -> Option<SegmentImpact> {
    obstacle.map(|(position, surface)| SegmentImpact::Obstacle { position, surface })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_world() -> ShellTraceWorld<'static> {
        ShellTraceWorld {
            projectile_radius_m: 0.05,
            tanks: &[],
            blockers: &[],
            heightmap: None,
            cover: &[],
            water: None,
        }
    }

    /// A non-advancing timestep must RETURN, not spin. The trace's only exits are a hit and the
    /// age limit, and with `dt <= 0` the shell neither moves nor ages — so a paused or malformed
    /// clock in the client's reticle preview would hang the frame instead of mispredicting it.
    /// `#[test]` cannot assert "terminates", so this simply calls it: a regression hangs the
    /// suite, which is the loudest possible failure.
    #[test]
    fn a_non_advancing_timestep_terminates_instead_of_spinning() {
        let world = empty_world();
        let muzzle = Vec3::new(0.0, 2.0, 0.0);
        let velocity = Vec3::new(0.0, 0.0, 900.0);
        for dt in [0.0_f32, -1.0 / 60.0, f32::NAN] {
            let outcome = trace_shell(muzzle, velocity, 0.09, dt, 4.0, &world);
            assert_eq!(
                outcome,
                TraceOutcome::Expired(muzzle),
                "dt {dt} must expire at the muzzle, not integrate"
            );
        }
        // And a real timestep still flies the arc.
        let outcome = trace_shell(muzzle, velocity, 0.09, 1.0 / 60.0, 4.0, &world);
        assert_ne!(outcome.impact_point(), muzzle, "a real timestep still moves the shell");
    }
}
