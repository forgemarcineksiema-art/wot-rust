//! The tank-vs-tank movement resolver, split from `collision.rs` (which keeps the footprint
//! types, the SAT overlap and the velocity trim) for the reviewability budget.

use glam::{Vec2, Vec3};

use crate::collision::{TankFootprint, TankObstacle, obstacles_overlap, trim_velocity};

/// Keep a moving tank footprint out of other tank footprints. This mirrors the static cover
/// resolver: try the full move, then each horizontal axis alone, then hold the previous
/// horizontal position if every candidate still overlaps.
///
/// Interpenetration escape: hulls can END UP overlapped without any blocked move — a pivot swings
/// the oriented footprint into a neighbor (yaw is not collision-resolved), and two hulls stepping
/// in the same tick can each clear the other's pre-step pose. From an overlapped start, "hold
/// previous" rejects EVERY move — including the one that backs out — deadlocking both tanks for
/// the rest of the battle. So when the previous pose already overlaps, a candidate is accepted if
/// it strictly separates from every hull it still touches and digs into no new one.
pub fn resolve_tank_collision(
    previous: Vec3,
    attempted: Vec3,
    yaw_rad: f32,
    footprint: TankFootprint,
    obstacles: &[TankObstacle],
) -> Vec3 {
    if obstacles.is_empty() {
        return attempted;
    }
    let stuck_on = overlapping_indices(previous, yaw_rad, footprint, obstacles);
    let candidates = [
        attempted,
        Vec3::new(attempted.x, attempted.y, previous.z),
        Vec3::new(previous.x, attempted.y, attempted.z),
    ];
    for candidate in candidates {
        let allowed = if stuck_on.is_empty() {
            !footprint_blocked_by_tanks(candidate, yaw_rad, footprint, obstacles)
        } else {
            escapes_overlaps(previous, candidate, yaw_rad, footprint, obstacles, &stuck_on)
        };
        if allowed {
            return candidate;
        }
    }
    Vec3::new(previous.x, attempted.y, previous.z)
}

pub fn resolve_tank_collision_with_velocity(
    previous: Vec3,
    attempted: Vec3,
    yaw_rad: f32,
    velocity: Vec3,
    footprint: TankFootprint,
    obstacles: &[TankObstacle],
) -> (Vec3, Vec3) {
    let resolved = resolve_tank_collision(previous, attempted, yaw_rad, footprint, obstacles);
    (resolved, trim_velocity(previous, attempted, resolved, velocity))
}

/// Indices of the obstacles whose footprints overlap the tank at `position`.
fn overlapping_indices(
    position: Vec3,
    yaw_rad: f32,
    footprint: TankFootprint,
    obstacles: &[TankObstacle],
) -> Vec<usize> {
    let moving = TankObstacle::new(position, yaw_rad, footprint);
    obstacles
        .iter()
        .enumerate()
        .filter(|(_, obstacle)| obstacles_overlap(&moving, obstacle))
        .map(|(index, _)| index)
        .collect()
}

/// Whether moving `previous -> candidate` monotonically escapes an interpenetration: every hull
/// still overlapped at `candidate` was already overlapped at `previous` (no fresh penetration)
/// AND the move points away from it (positive component along obstacle-centre -> hull-centre),
/// so repeated ticks separate instead of grinding deeper or sliding sideways forever.
fn escapes_overlaps(
    previous: Vec3,
    candidate: Vec3,
    yaw_rad: f32,
    footprint: TankFootprint,
    obstacles: &[TankObstacle],
    stuck_on: &[usize],
) -> bool {
    let delta = Vec2::new(candidate.x - previous.x, candidate.z - previous.z);
    if delta.length_squared() < 1.0e-12 {
        return false;
    }
    overlapping_indices(candidate, yaw_rad, footprint, obstacles).iter().all(|index| {
        let obstacle = &obstacles[*index];
        let away = Vec2::new(previous.x - obstacle.center.x, previous.z - obstacle.center.z);
        stuck_on.contains(index) && away.length_squared() > 1.0e-9 && delta.dot(away) > 0.0
    })
}

/// Whether the hull footprint at `position`/`yaw_rad` overlaps any tank obstacle. Public
/// because the world step also asks it about a ROTATION candidate.
pub fn footprint_blocked_by_tanks(
    position: Vec3,
    yaw_rad: f32,
    footprint: TankFootprint,
    obstacles: &[TankObstacle],
) -> bool {
    let moving = TankObstacle::new(position, yaw_rad, footprint);
    obstacles.iter().any(|obstacle| obstacles_overlap(&moving, obstacle))
}
