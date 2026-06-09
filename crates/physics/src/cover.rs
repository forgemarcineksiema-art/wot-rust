use glam::Vec3;
use terrain::StaticCoverObject;

use crate::collision::{TANK_COLLISION_RADIUS_M, trim_forward_speed};

/// Keep a tank out of static cover footprints. Tries the full move, then each horizontal axis
/// alone so the hull slides along a wall instead of sticking; if every option still overlaps
/// cover the hull holds its previous horizontal position. `y` is taken from `attempted`.
pub fn resolve_cover_collision(
    previous: Vec3,
    attempted: Vec3,
    cover: &[StaticCoverObject],
) -> Vec3 {
    if cover.is_empty() || !blocked(attempted.x, attempted.z, cover) {
        return attempted;
    }
    if !blocked(attempted.x, previous.z, cover) {
        return Vec3::new(attempted.x, attempted.y, previous.z);
    }
    if !blocked(previous.x, attempted.z, cover) {
        return Vec3::new(previous.x, attempted.y, attempted.z);
    }
    Vec3::new(previous.x, attempted.y, previous.z)
}

pub fn resolve_cover_collision_with_speed(
    previous: Vec3,
    attempted: Vec3,
    yaw_rad: f32,
    forward_speed_mps: f32,
    cover: &[StaticCoverObject],
    dt_seconds: f32,
) -> (Vec3, f32) {
    let resolved = resolve_cover_collision(previous, attempted, cover);
    trim_forward_speed(previous, attempted, resolved, yaw_rad, forward_speed_mps, dt_seconds)
}

fn blocked(x: f32, z: f32, cover: &[StaticCoverObject]) -> bool {
    cover.iter().any(|object| {
        let min_x = object.center[0] - object.half_extents_m[0] - TANK_COLLISION_RADIUS_M;
        let max_x = object.center[0] + object.half_extents_m[0] + TANK_COLLISION_RADIUS_M;
        let min_z = object.center[2] - object.half_extents_m[2] - TANK_COLLISION_RADIUS_M;
        let max_z = object.center[2] + object.half_extents_m[2] + TANK_COLLISION_RADIUS_M;
        (min_x..=max_x).contains(&x) && (min_z..=max_z).contains(&z)
    })
}
