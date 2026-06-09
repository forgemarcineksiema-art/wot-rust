use game_core::HitboxProfile;
use glam::{Vec2, Vec3};
use terrain::StaticCoverObject;

const TANK_COLLISION_RADIUS_M: f32 = 1.6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TankFootprint {
    pub half_width_m: f32,
    pub half_length_m: f32,
}

impl TankFootprint {
    pub fn from_hitbox(hitbox: HitboxProfile) -> Self {
        Self {
            half_width_m: hitbox.half_width_m.max(0.01),
            half_length_m: hitbox.half_length_m.max(0.01),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TankObstacle {
    pub center: Vec3,
    pub yaw_rad: f32,
    pub footprint: TankFootprint,
}

impl TankObstacle {
    pub fn new(center: Vec3, yaw_rad: f32, footprint: TankFootprint) -> Self {
        Self { center, yaw_rad, footprint }
    }

    pub fn from_hitbox(center: Vec3, yaw_rad: f32, hitbox: HitboxProfile) -> Self {
        Self::new(center, yaw_rad, TankFootprint::from_hitbox(hitbox))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TankWorldObstacles<'a> {
    pub cover: &'a [StaticCoverObject],
    pub tank_footprint: TankFootprint,
    pub tanks: &'a [TankObstacle],
}

impl<'a> TankWorldObstacles<'a> {
    pub fn new(
        cover: &'a [StaticCoverObject],
        tank_footprint: TankFootprint,
        tanks: &'a [TankObstacle],
    ) -> Self {
        Self { cover, tank_footprint, tanks }
    }
}

pub fn default_tank_footprint() -> TankFootprint {
    TankFootprint { half_width_m: TANK_COLLISION_RADIUS_M, half_length_m: TANK_COLLISION_RADIUS_M }
}

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

/// Keep a moving tank footprint out of other tank footprints. This mirrors the static cover
/// resolver: try the full move, then each horizontal axis alone, then hold the previous
/// horizontal position if every candidate still overlaps.
pub fn resolve_tank_collision(
    previous: Vec3,
    attempted: Vec3,
    yaw_rad: f32,
    footprint: TankFootprint,
    obstacles: &[TankObstacle],
) -> Vec3 {
    if obstacles.is_empty() || !tank_blocked(attempted, yaw_rad, footprint, obstacles) {
        return attempted;
    }
    if !tank_blocked(Vec3::new(attempted.x, attempted.y, previous.z), yaw_rad, footprint, obstacles)
    {
        return Vec3::new(attempted.x, attempted.y, previous.z);
    }
    if !tank_blocked(Vec3::new(previous.x, attempted.y, attempted.z), yaw_rad, footprint, obstacles)
    {
        return Vec3::new(previous.x, attempted.y, attempted.z);
    }
    Vec3::new(previous.x, attempted.y, previous.z)
}

pub fn resolve_tank_collision_with_speed(
    previous: Vec3,
    attempted: Vec3,
    yaw_rad: f32,
    forward_speed_mps: f32,
    footprint: TankFootprint,
    obstacles: &[TankObstacle],
    dt_seconds: f32,
) -> (Vec3, f32) {
    let resolved = resolve_tank_collision(previous, attempted, yaw_rad, footprint, obstacles);
    trim_forward_speed(previous, attempted, resolved, yaw_rad, forward_speed_mps, dt_seconds)
}

fn trim_forward_speed(
    previous: Vec3,
    attempted: Vec3,
    resolved: Vec3,
    yaw_rad: f32,
    forward_speed_mps: f32,
    dt_seconds: f32,
) -> (Vec3, f32) {
    if horizontal_position_matches(resolved, attempted) || dt_seconds <= f32::EPSILON {
        return (resolved, forward_speed_mps);
    }

    let actual_delta = Vec3::new(resolved.x - previous.x, 0.0, resolved.z - previous.z);
    let forward = Vec3::new(yaw_rad.sin(), 0.0, yaw_rad.cos());
    let projected_speed = actual_delta.dot(forward) / dt_seconds;
    if projected_speed.signum() == forward_speed_mps.signum() {
        (resolved, projected_speed)
    } else {
        (resolved, 0.0)
    }
}

fn horizontal_position_matches(a: Vec3, b: Vec3) -> bool {
    (a.x - b.x).abs() <= 1.0e-5 && (a.z - b.z).abs() <= 1.0e-5
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

fn tank_blocked(
    position: Vec3,
    yaw_rad: f32,
    footprint: TankFootprint,
    obstacles: &[TankObstacle],
) -> bool {
    obstacles.iter().any(|obstacle| {
        footprints_overlap(
            position,
            yaw_rad,
            footprint,
            obstacle.center,
            obstacle.yaw_rad,
            obstacle.footprint,
        )
    })
}

fn footprints_overlap(
    center_a: Vec3,
    yaw_a: f32,
    footprint_a: TankFootprint,
    center_b: Vec3,
    yaw_b: f32,
    footprint_b: TankFootprint,
) -> bool {
    let center_a = Vec2::new(center_a.x, center_a.z);
    let center_b = Vec2::new(center_b.x, center_b.z);
    let [right_a, forward_a] = footprint_axes(yaw_a);
    let [right_b, forward_b] = footprint_axes(yaw_b);
    let delta = center_b - center_a;
    for axis in [right_a, forward_a, right_b, forward_b] {
        let radius_a = footprint_a.half_width_m * axis.dot(right_a).abs()
            + footprint_a.half_length_m * axis.dot(forward_a).abs();
        let radius_b = footprint_b.half_width_m * axis.dot(right_b).abs()
            + footprint_b.half_length_m * axis.dot(forward_b).abs();
        if delta.dot(axis).abs() >= radius_a + radius_b - 1.0e-5 {
            return false;
        }
    }
    true
}

fn footprint_axes(yaw_rad: f32) -> [Vec2; 2] {
    let forward = Vec2::new(yaw_rad.sin(), yaw_rad.cos());
    let right = Vec2::new(forward.y, -forward.x);
    [right, forward]
}
