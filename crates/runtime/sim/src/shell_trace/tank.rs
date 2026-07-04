use game_core::ArmorZone;
use game_core::math::{armor_normal, segment_box_entry, world_to_tank_local};
use glam::{Mat3, Vec3};

use super::{SegmentImpact, TraceTank};

/// Entry faces are detected by proximity to a box plane; comfortably larger than float noise,
/// far smaller than any armor band.
const FACE_EPS: f32 = 1.0e-3;

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

/// A tank is two volumes, not one box: the full-plan hull slab below the armor split, and the
/// narrower turret box above it. The turret box traverses with the turret about the ring axis, so
/// a shot at turret height that visually passes beside the turret really passes — it no longer
/// connects with deck air the old full-plan box put there.
fn tank_segment_hit(
    previous: Vec3,
    current: Vec3,
    velocity: Vec3,
    tank: &TraceTank,
) -> Option<SegmentImpact> {
    let hitbox = tank.hitbox;
    let half = Vec3::new(hitbox.half_width_m, hitbox.half_height_m, hitbox.half_length_m);
    let start = world_to_tank_local(previous, tank.position, hitbox.center_y_m, tank.hull);
    let end = world_to_tank_local(current, tank.position, hitbox.center_y_m, tank.hull);

    let hull = hull_volume_entry(start, end, &hitbox);
    let turret = turret_volume_entry(start, end, tank, &hitbox);

    let (hit_t, zone, side_x) = match (hull, turret) {
        (Some((hull_t, hull_local)), Some((turret_t, turret_local))) => {
            if turret_t <= hull_t {
                let (zone, x) = classify_turret(turret_local, &hitbox);
                (turret_t, zone, x)
            } else {
                let (zone, x) = classify_hull(hull_local, half, hitbox.turret_min_y_m);
                (hull_t, zone, x)
            }
        }
        (Some((hull_t, hull_local)), None) => {
            let (zone, x) = classify_hull(hull_local, half, hitbox.turret_min_y_m);
            (hull_t, zone, x)
        }
        (None, Some((turret_t, turret_local))) => {
            let (zone, x) = classify_turret(turret_local, &hitbox);
            (turret_t, zone, x)
        }
        (None, None) => return None,
    };

    let hit_position = previous.lerp(current, hit_t);
    let facing = zone.facing();
    let normal = armor_normal(tank.hull, tank.turret_yaw_rad, facing, side_x);
    let direction = velocity.normalize_or_zero();
    let impact_angle_degrees = (-direction).dot(normal).clamp(-1.0, 1.0).acos().to_degrees();
    Some(SegmentImpact::Tank { id: tank.id, facing, zone, impact_angle_degrees, hit_position })
}

/// Entry into the hull slab: full plan, capped at the armor split. Returns the parametric entry
/// and the hull-local hit point.
fn hull_volume_entry(
    start: Vec3,
    end: Vec3,
    hitbox: &game_core::HitboxProfile,
) -> Option<(f32, Vec3)> {
    let min = Vec3::new(-hitbox.half_width_m, -hitbox.half_height_m, -hitbox.half_length_m);
    let max = Vec3::new(hitbox.half_width_m, hitbox.turret_min_y_m, hitbox.half_length_m);
    let t = segment_box_entry(start, end, min, max)?;
    Some((t, start.lerp(end, t)))
}

/// Entry into the turret box, evaluated in the turret frame (hull frame yawed by turret traverse
/// about the ring axis). Returns the parametric entry and the *turret-frame* hit point, which is
/// the armor frame for turret plates — the mantlet band follows the gun, not the hull.
fn turret_volume_entry(
    start: Vec3,
    end: Vec3,
    tank: &TraceTank,
    hitbox: &game_core::HitboxProfile,
) -> Option<(f32, Vec3)> {
    let pivot = Vec3::new(0.0, 0.0, tank.turret_ring_z_m);
    let to_turret = Mat3::from_rotation_y(-tank.turret_yaw_rad);
    let turret_start = pivot + to_turret * (start - pivot);
    let turret_end = pivot + to_turret * (end - pivot);
    let min = Vec3::new(
        -hitbox.turret_half_width_m,
        hitbox.turret_min_y_m,
        hitbox.turret_center_z_m - hitbox.turret_half_length_m,
    );
    let max = Vec3::new(
        hitbox.turret_half_width_m,
        hitbox.half_height_m,
        hitbox.turret_center_z_m + hitbox.turret_half_length_m,
    );
    let t = segment_box_entry(turret_start, turret_end, min, max)?;
    Some((t, turret_start.lerp(turret_end, t)))
}

/// Hull-volume zones. The slab's top face is the deck: a plunging hit beside the turret lands on
/// thin roof plate, not on a phantom turret side. Below deck the bands are unchanged from the
/// single-box model.
fn classify_hull(local_hit: Vec3, half: Vec3, turret_min_y_m: f32) -> (ArmorZone, f32) {
    if local_hit.y >= turret_min_y_m - FACE_EPS {
        return (ArmorZone::Roof, local_hit.x);
    }
    let x_reach = local_hit.x.abs() / half.x.max(0.01);
    let z_reach = local_hit.z.abs() / half.z.max(0.01);
    let zone = if z_reach >= x_reach {
        if local_hit.z >= 0.0 { hull_front_zone(local_hit) } else { ArmorZone::HullRear }
    } else if local_hit.y <= -half.y * 0.25 {
        if local_hit.x < 0.0 { ArmorZone::LeftTrack } else { ArmorZone::RightTrack }
    } else {
        ArmorZone::HullSide
    };
    (zone, local_hit.x)
}

/// Turret-volume zones, in the turret frame. The roof and mantlet bands keep their pre-turret-box
/// absolute sizes (the roof band hangs off the box top, the mantlet band off the full hull width),
/// so narrowing the volume did not retune any armor band.
fn classify_turret(local_hit: Vec3, hitbox: &game_core::HitboxProfile) -> (ArmorZone, f32) {
    let dz = local_hit.z - hitbox.turret_center_z_m;
    let x_reach = local_hit.x.abs() / hitbox.turret_half_width_m.max(0.01);
    let z_reach = dz.abs() / hitbox.turret_half_length_m.max(0.01);
    let zone = if z_reach >= x_reach {
        if dz >= 0.0 { turret_front_zone(local_hit, hitbox) } else { ArmorZone::TurretRear }
    } else {
        ArmorZone::TurretSide
    };
    (zone, local_hit.x)
}

fn hull_front_zone(local_hit: Vec3) -> ArmorZone {
    if local_hit.y < -0.15 { ArmorZone::LowerPlate } else { ArmorZone::UpperGlacis }
}

fn turret_front_zone(local_hit: Vec3, hitbox: &game_core::HitboxProfile) -> ArmorZone {
    if local_hit.y >= hitbox.half_height_m * 0.88 {
        ArmorZone::Roof
    } else if local_hit.x.abs() <= hitbox.half_width_m * 0.32 {
        ArmorZone::Mantlet
    } else {
        ArmorZone::TurretFront
    }
}
