//! The legacy two-box hit model for vehicles not yet migrated onto blueprint armor volumes:
//! a full-plan hull slab below the armor split, a traversing turret box above it, and
//! classification BANDS mapping the entry point to an armor zone. Migrated vehicles resolve
//! against their baked convex volumes instead (see `tank::armor_volume_hit`); this file shrinks
//! one vehicle at a time as the fleet migrates.

use game_core::ArmorZone;
use game_core::math::segment_box_entry;
use glam::{Mat3, Vec3};

use super::TraceTank;

/// Entry faces are detected by proximity to a box plane; comfortably larger than float noise,
/// far smaller than any armor band.
const FACE_EPS: f32 = 1.0e-3;

/// Entry into the hull slab: full plan, capped at the armor split. Returns the parametric entry
/// and the hull-local hit point.
pub(super) fn hull_volume_entry(
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
pub(super) fn turret_volume_entry(
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
pub(super) fn classify_hull(local_hit: Vec3, half: Vec3, turret_min_y_m: f32) -> (ArmorZone, f32) {
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
pub(super) fn classify_turret(
    local_hit: Vec3,
    hitbox: &game_core::HitboxProfile,
) -> (ArmorZone, f32) {
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
