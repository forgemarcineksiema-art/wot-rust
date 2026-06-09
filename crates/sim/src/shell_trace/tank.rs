use game_core::ArmorZone;
use game_core::math::{armor_normal, segment_box_entry, world_to_tank_local};
use glam::Vec3;

use super::{SegmentImpact, TraceTank};

/// Nearest tank the segment `previous -> current` enters (analytic ray vs hull-local AABB). The
/// caller pre-filters the slice (owner, dead, friendly), so this is pure geometry + classification.
pub(super) fn first_tank_impact(
    previous: Vec3,
    current: Vec3,
    velocity: Vec3,
    tanks: &[TraceTank],
) -> Option<SegmentImpact> {
    tanks.iter().filter_map(|tank| tank_segment_hit(previous, current, velocity, tank)).min_by(
        |left, right| {
            left.point()
                .distance_squared(previous)
                .total_cmp(&right.point().distance_squared(previous))
        },
    )
}

fn tank_segment_hit(
    previous: Vec3,
    current: Vec3,
    velocity: Vec3,
    tank: &TraceTank,
) -> Option<SegmentImpact> {
    let hitbox = tank.hitbox;
    let half = Vec3::new(hitbox.half_width_m, hitbox.half_height_m, hitbox.half_length_m);
    let start = world_to_tank_local(previous, tank.position, hitbox.center_y_m, tank.yaw_rad);
    let end = world_to_tank_local(current, tank.position, hitbox.center_y_m, tank.yaw_rad);
    let hit_t = segment_box_entry(start, end, -half, half)?;
    let local_hit = start.lerp(end, hit_t);
    let hit_position = previous.lerp(current, hit_t);
    let zone = armor_zone_for_local_hit(local_hit, half, hitbox.turret_min_y_m);
    let facing = zone.facing();
    let normal = armor_normal(tank.yaw_rad, tank.turret_yaw_rad, facing, local_hit.x);
    let direction = velocity.normalize_or_zero();
    let impact_angle_degrees = (-direction).dot(normal).clamp(-1.0, 1.0).acos().to_degrees();
    Some(SegmentImpact::Tank { id: tank.id, facing, zone, impact_angle_degrees, hit_position })
}

fn armor_zone_for_local_hit(local_hit: Vec3, half: Vec3, turret_min_y_m: f32) -> ArmorZone {
    let is_turret_hit = local_hit.y >= turret_min_y_m;
    let x_reach = local_hit.x.abs() / half.x.max(0.01);
    let z_reach = local_hit.z.abs() / half.z.max(0.01);
    if z_reach >= x_reach {
        if local_hit.z >= 0.0 {
            if is_turret_hit {
                turret_front_zone(local_hit, half)
            } else {
                hull_front_zone(local_hit)
            }
        } else if is_turret_hit {
            ArmorZone::TurretRear
        } else {
            ArmorZone::HullRear
        }
    } else if is_turret_hit {
        ArmorZone::TurretSide
    } else if local_hit.y <= -half.y * 0.25 {
        if local_hit.x < 0.0 { ArmorZone::LeftTrack } else { ArmorZone::RightTrack }
    } else {
        ArmorZone::HullSide
    }
}

fn hull_front_zone(local_hit: Vec3) -> ArmorZone {
    if local_hit.y < -0.15 { ArmorZone::LowerPlate } else { ArmorZone::UpperGlacis }
}

fn turret_front_zone(local_hit: Vec3, half: Vec3) -> ArmorZone {
    if local_hit.y >= half.y * 0.88 {
        ArmorZone::Roof
    } else if local_hit.x.abs() <= half.x * 0.32 {
        ArmorZone::Mantlet
    } else {
        ArmorZone::TurretFront
    }
}
