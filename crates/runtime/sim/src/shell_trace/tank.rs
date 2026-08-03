use game_core::math::{plate_normal, world_to_tank_local};
use game_core::{TaggedPlane, VehicleArmorVolumes, segment_volume_entry_with_margin};
use glam::{Mat3, Vec3};

use super::legacy_boxes::{classify_hull, classify_turret, hull_volume_entry, turret_volume_entry};
use super::{SegmentImpact, TraceTank};

/// Nearest tank the segment `previous -> current` enters (analytic ray vs hull-local AABB). The
/// caller pre-filters the slice (owner, dead, friendly), so this is pure geometry + classification.
pub(super) fn first_tank_impact(
    previous: Vec3,
    current: Vec3,
    velocity: Vec3,
    tanks: &[TraceTank],
    radius_m: f32,
) -> Option<SegmentImpact> {
    tanks
        .iter()
        .filter_map(|tank| tank_segment_hit(previous, current, velocity, tank, radius_m))
        .min_by(|left, right| {
            left.point()
                .distance_squared(previous)
                .total_cmp(&right.point().distance_squared(previous))
        })
}

/// A tank is two volumes, not one box: the full-plan hull slab below the armor split, and the
/// narrower turret box above it. The turret box traverses with the turret about the ring axis, so
/// a shot at turret height that visually passes beside the turret really passes — it no longer
/// connects with deck air the old full-plan box put there.
///
/// Blueprint vehicles go further: their baked [`VehicleArmorVolumes`] replace the boxes and the
/// classification bands entirely — the entering PLANE of a convex armor volume is the struck
/// plate, carrying its own true normal and zone.
fn tank_segment_hit(
    previous: Vec3,
    current: Vec3,
    velocity: Vec3,
    tank: &TraceTank,
    radius_m: f32,
) -> Option<SegmentImpact> {
    let hitbox = tank.hitbox;
    let mut swept_hitbox = hitbox;
    swept_hitbox.half_width_m += radius_m;
    swept_hitbox.half_height_m += radius_m;
    swept_hitbox.half_length_m += radius_m;
    swept_hitbox.turret_half_width_m += radius_m;
    swept_hitbox.turret_half_length_m += radius_m;
    swept_hitbox.turret_min_y_m -= radius_m;
    let half = Vec3::new(hitbox.half_width_m, hitbox.half_height_m, hitbox.half_length_m);
    let start = world_to_tank_local(previous, tank.position, hitbox.center_y_m, tank.hull);
    let end = world_to_tank_local(current, tank.position, hitbox.center_y_m, tank.hull);

    if let Some(volumes) = tank.armor_volumes {
        return armor_volume_hit(start, end, previous, current, velocity, tank, volumes, radius_m);
    }

    let hull = hull_volume_entry(start, end, &swept_hitbox);
    // A decapitated wreck has no turret box to strike: the shot passes over the hull.
    let turret = if tank.turret_detached {
        None
    } else {
        turret_volume_entry(start, end, tank, &swept_hitbox)
    };
    // (Legacy band model below — blueprint vehicles returned through the volume path above.)

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

    let center_position = previous.lerp(current, hit_t);
    let facing = zone.facing();
    // The impact angle is measured against the plate's TRUE normal: facet slope, hull attitude,
    // and turret traverse all live in the geometry. Nothing downstream may add slope again.
    let slope_degrees = tank.armor.plate(zone).slope_degrees;
    let normal = plate_normal(tank.hull, tank.turret_yaw_rad, zone, side_x, slope_degrees);
    let direction = velocity.normalize_or_zero();
    let impact_angle_degrees = (-direction).dot(normal).clamp(-1.0, 1.0).acos().to_degrees();
    let hit_position = center_position - normal * radius_m;
    Some(SegmentImpact::Tank {
        id: tank.id,
        facing,
        zone,
        impact_angle_degrees,
        hit_position,
        plate_normal: normal,
        // The legacy band path has no per-plate geometry: the zone's facet IS the plate.
        thickness_scale: 1.0,
    })
}

/// The baked-volume narrow phase: the nearest entering plane across the hull volumes (hull
/// frame) and the turret volume (turret frame, rotated about the ring axis) IS the struck
/// plate. Its normal carries the real slope, its zone comes from the plane (or a weakspot
/// patch riding on it — the mantlet ball), and both ride the full hull attitude.
#[allow(clippy::too_many_arguments)]
fn armor_volume_hit(
    start: Vec3,
    end: Vec3,
    previous: Vec3,
    current: Vec3,
    velocity: Vec3,
    tank: &TraceTank,
    volumes: &'static VehicleArmorVolumes,
    radius_m: f32,
) -> Option<SegmentImpact> {
    struct Entry {
        t: f32,
        plane: &'static TaggedPlane,
        frame_start: Vec3,
        frame_end: Vec3,
        turret_frame: bool,
    }
    let mut best: Option<Entry> = None;
    for volume in &volumes.hull {
        if let Some((t, index)) = segment_volume_entry_with_margin(start, end, volume, radius_m)
            && best.as_ref().is_none_or(|entry| t < entry.t)
        {
            best = Some(Entry {
                t,
                plane: &volume.planes[index],
                frame_start: start,
                frame_end: end,
                turret_frame: false,
            });
        }
    }
    // The turret volume lives in the turret frame: rotate the segment about the ring axis. A
    // decapitated wreck has no turret volume — skip it so the shot passes over the bare hull.
    if !tank.turret_detached {
        let pivot = Vec3::new(0.0, 0.0, volumes.turret_ring_z);
        let to_turret = Mat3::from_rotation_y(-tank.turret_yaw_rad);
        let turret_start = pivot + to_turret * (start - pivot);
        let turret_end = pivot + to_turret * (end - pivot);
        if let Some((t, index)) =
            segment_volume_entry_with_margin(turret_start, turret_end, &volumes.turret, radius_m)
            && best.as_ref().is_none_or(|entry| t < entry.t)
        {
            best = Some(Entry {
                t,
                plane: &volumes.turret.planes[index],
                frame_start: turret_start,
                frame_end: turret_end,
                turret_frame: true,
            });
        }
        // The commander's drum stands PROUD of the casting: a shell aimed at it enters the
        // drum before (or instead of) anything else, and resolves as the drum — not as the
        // roof plane it used to graze over.
        if let Some((t, index)) =
            segment_volume_entry_with_margin(turret_start, turret_end, &volumes.cupola, radius_m)
            && best.as_ref().is_none_or(|entry| t < entry.t)
        {
            best = Some(Entry {
                t,
                plane: &volumes.cupola.planes[index],
                frame_start: turret_start,
                frame_end: turret_end,
                turret_frame: true,
            });
        }
    }

    let entry = best?;
    let frame_center = entry.frame_start.lerp(entry.frame_end, entry.t);
    let frame_contact = frame_center - entry.plane.normal * radius_m;
    let patch = entry.plane.patch_at(frame_contact);
    let zone = patch.map_or(entry.plane.zone, |patch| patch.zone);
    // A patch may PRESENT differently than its carrier: a bow port's ball stands flat out of
    // the raked plate, so inside the disc the impact angle is the ball's, not the glacis'.
    let presented_normal =
        patch.and_then(|patch| patch.presents_normal).unwrap_or(entry.plane.normal);
    let hull_local_normal = if entry.turret_frame {
        Mat3::from_rotation_y(tank.turret_yaw_rad) * presented_normal
    } else {
        presented_normal
    };
    let normal = (tank.hull.basis() * hull_local_normal).normalize_or_zero();
    let direction = velocity.normalize_or_zero();
    let impact_angle_degrees = (-direction).dot(normal).clamp(-1.0, 1.0).acos().to_degrees();
    Some(SegmentImpact::Tank {
        id: tank.id,
        facing: zone.facing(),
        zone,
        impact_angle_degrees,
        hit_position: previous.lerp(current, entry.t) - normal * radius_m,
        plate_normal: normal,
        // A weakspot patch (the mantlet ball) resolves as its OWN zone, with its own derived
        // plate — the carrier sector's taper does not apply to it.
        thickness_scale: if zone == entry.plane.zone {
            entry.plane.thickness_scale.unwrap_or(1.0)
        } else {
            1.0
        },
    })
}
