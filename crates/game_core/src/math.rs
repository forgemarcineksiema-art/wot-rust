//! Small shared geometry and angle helpers used across the `physics`, `sim`, and `client` crates.
//!
//! These are deliberately data-free, allocation-free leaf functions. They live in `game_core` —
//! the data crate every gameplay crate already depends on — so the turret-direction, angle-wrap,
//! and ray/box math has a single source of truth instead of being re-derived (and drifting) in
//! each crate.

use glam::{Mat3, Vec3};

use crate::{ArmorFacing, MountFrames};

/// Mildly exaggerated gravity so shell arcs read at map scale (real is ~9.81 m/s^2).
pub const GRAVITY_MPS2: f32 = 12.0;

/// Horizontal unit heading for a yaw angle, in the XZ plane (+Z at yaw 0).
pub fn horizontal_forward(yaw_rad: f32) -> Vec3 {
    Vec3::new(yaw_rad.sin(), 0.0, yaw_rad.cos())
}

/// Unit firing direction for a turret yaw + gun pitch (+pitch elevates the muzzle).
///
/// The result is already unit length (`cos²pitch·(sin²yaw + cos²yaw) + sin²pitch == 1`), so
/// callers do not need to normalize.
pub fn gun_direction(yaw_rad: f32, pitch_rad: f32) -> Vec3 {
    let horizontal = pitch_rad.cos();
    Vec3::new(yaw_rad.sin() * horizontal, pitch_rad.sin(), yaw_rad.cos() * horizontal)
}

/// Rotate `point` about `pivot` by `rotation`.
pub fn rotate_around(point: Vec3, pivot: Vec3, rotation: Mat3) -> Vec3 {
    pivot + rotation * (point - pivot)
}

/// World-space muzzle position for a tank pose.
///
/// The muzzle mount pivots about the trunnion for gun pitch, about the turret ring for turret
/// traverse, and about the hull origin for hull yaw — the same chain the renderer applies to the
/// gun submesh. This keeps the ballistic origin on the visible muzzle: approximating the pivot at
/// the hull centre instead (the old `position + Y·muzzle.y + direction·muzzle.z`) drifts by
/// `sin(pitch) · trunnion.z` vertically, ~26 cm on a Jagdtiger at full elevation.
///
/// `turret_yaw_rad` is the *effective* traverse: casemate vehicles hold it at zero (the sim
/// already enforces this on its state, and snapshots carry the held value).
pub fn muzzle_world_position(
    mounts: &MountFrames,
    position: Vec3,
    hull_yaw_rad: f32,
    turret_yaw_rad: f32,
    gun_pitch_rad: f32,
) -> Vec3 {
    muzzle_world_position_scaled(
        mounts,
        position,
        hull_yaw_rad,
        turret_yaw_rad,
        gun_pitch_rad,
        1.0,
    )
}

/// As [`muzzle_world_position`], but with the barrel scaled by `barrel_scale` about the trunnion so
/// a longer/shorter installed gun fires from — and is drawn to — its real tip. `barrel_scale` is
/// the installed barrel length over the vehicle's stock barrel length (1.0 = stock).
pub fn muzzle_world_position_scaled(
    mounts: &MountFrames,
    position: Vec3,
    hull_yaw_rad: f32,
    turret_yaw_rad: f32,
    gun_pitch_rad: f32,
    barrel_scale: f32,
) -> Vec3 {
    let trunnion = mounts.gun_trunnion.translation;
    let muzzle = trunnion + (mounts.muzzle.translation - trunnion) * barrel_scale;
    let pitched = rotate_around(muzzle, trunnion, Mat3::from_rotation_x(-gun_pitch_rad));
    let traversed = rotate_around(
        pitched,
        mounts.turret_ring.translation,
        Mat3::from_rotation_y(turret_yaw_rad),
    );
    position + Mat3::from_rotation_y(hull_yaw_rad) * traversed
}

/// Outward armor-plate normal for a hit, in world space.
///
/// Turret plates rotate with the turret, so their normal follows `hull_yaw + turret_yaw`; hull
/// plates stay aligned to the hull. `local_x` (the hit's sideways offset in hull-local space)
/// picks which side a side hit faces.
pub fn armor_normal(
    hull_yaw_rad: f32,
    turret_yaw_rad: f32,
    facing: ArmorFacing,
    local_x: f32,
) -> Vec3 {
    let yaw_rad = match facing {
        ArmorFacing::TurretFront | ArmorFacing::TurretRear | ArmorFacing::TurretSide => {
            hull_yaw_rad + turret_yaw_rad
        }
        ArmorFacing::HullFront | ArmorFacing::HullRear | ArmorFacing::HullSide => hull_yaw_rad,
    };
    let forward = horizontal_forward(yaw_rad);
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    match facing {
        ArmorFacing::HullFront | ArmorFacing::TurretFront => forward,
        ArmorFacing::HullRear | ArmorFacing::TurretRear => -forward,
        ArmorFacing::HullSide | ArmorFacing::TurretSide if local_x >= 0.0 => right,
        ArmorFacing::HullSide | ArmorFacing::TurretSide => -right,
    }
}

/// Express a world position in a tank's hull-local frame: `x` = right, `y` = up relative to the
/// hitbox center plane, `z` = forward. `center_y_m` lifts the local origin to the hitbox center.
pub fn world_to_tank_local(
    position: Vec3,
    tank_position: Vec3,
    center_y_m: f32,
    hull_yaw_rad: f32,
) -> Vec3 {
    let center = tank_position + Vec3::Y * center_y_m;
    let rel = position - center;
    let forward = horizontal_forward(hull_yaw_rad);
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    Vec3::new(rel.dot(right), rel.y, rel.dot(forward))
}

/// Wrap an angle into the half-open range `(-PI, PI]`.
pub fn wrap_angle(radians: f32) -> f32 {
    let wrapped = radians.rem_euclid(std::f32::consts::TAU);
    if wrapped > std::f32::consts::PI { wrapped - std::f32::consts::TAU } else { wrapped }
}

/// Shortest-arc interpolation between two angles, so a wrap across +/-PI does not spin the long
/// way round between the endpoints.
pub fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let mut diff = (b - a) % TAU;
    if diff > PI {
        diff -= TAU;
    } else if diff < -PI {
        diff += TAU;
    }
    a + diff * t
}

/// Slab test: the parametric entry `t` in `[0, 1]` where the segment `p0 -> p1` first crosses
/// into the AABB `[min, max]`, or `None` if it never overlaps. A segment starting inside the box
/// returns `0`.
pub fn segment_box_entry(p0: Vec3, p1: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let direction = p1 - p0;
    let mut enter = 0.0f32;
    let mut exit = 1.0f32;
    for axis in 0..3 {
        let origin = p0[axis];
        let delta = direction[axis];
        if delta.abs() < 1.0e-6 {
            if origin < min[axis] || origin > max[axis] {
                return None;
            }
        } else {
            let inv_delta = 1.0 / delta;
            let mut near = (min[axis] - origin) * inv_delta;
            let mut far = (max[axis] - origin) * inv_delta;
            if near > far {
                std::mem::swap(&mut near, &mut far);
            }
            enter = enter.max(near);
            exit = exit.min(far);
            if enter > exit {
                return None;
            }
        }
    }
    Some(enter)
}

#[cfg(test)]
mod tests {
    use std::f32::consts::{FRAC_PI_2, PI, TAU};

    use super::*;

    #[test]
    fn horizontal_forward_points_down_positive_z_at_zero_yaw() {
        assert!((horizontal_forward(0.0) - Vec3::new(0.0, 0.0, 1.0)).length() < 1.0e-6);
        assert!((horizontal_forward(FRAC_PI_2) - Vec3::new(1.0, 0.0, 0.0)).length() < 1.0e-6);
    }

    #[test]
    fn rotate_around_pivots_in_place() {
        let rotation = Mat3::from_rotation_y(FRAC_PI_2);
        // The pivot itself is fixed; a point +Z of it swings to +X (yaw 90°).
        assert!(
            (rotate_around(Vec3::new(0.0, 0.0, 1.0), Vec3::ZERO, rotation) - Vec3::X).length()
                < 1.0e-6
        );
        let pivot = Vec3::new(5.0, 0.0, 5.0);
        assert!((rotate_around(pivot, pivot, rotation) - pivot).length() < 1.0e-6);
    }

    #[test]
    fn armor_normal_follows_turret_rotation_while_hull_stays_put() {
        // Hull points down +z; turret traversed 90° so its front faces the hull's right (+x).
        let turret_front = armor_normal(0.0, FRAC_PI_2, ArmorFacing::TurretFront, 0.0);
        let hull_front = armor_normal(0.0, FRAC_PI_2, ArmorFacing::HullFront, 0.0);
        assert!((turret_front - Vec3::new(1.0, 0.0, 0.0)).length() < 1.0e-5);
        assert!((hull_front - Vec3::new(0.0, 0.0, 1.0)).length() < 1.0e-5);

        // Side hits pick the side from the hull-local x sign.
        let right = armor_normal(0.0, 0.0, ArmorFacing::HullSide, 1.0);
        let left = armor_normal(0.0, 0.0, ArmorFacing::HullSide, -1.0);
        assert!((right - Vec3::new(1.0, 0.0, 0.0)).length() < 1.0e-5);
        assert!((left - Vec3::new(-1.0, 0.0, 0.0)).length() < 1.0e-5);
    }

    #[test]
    fn world_to_tank_local_maps_forward_right_and_up() {
        // Tank at origin facing +z (yaw 0), hitbox center 1.0 up: a point straight ahead and
        // level with the center maps to pure forward (+z local).
        let ahead = world_to_tank_local(Vec3::new(0.0, 1.0, 5.0), Vec3::ZERO, 1.0, 0.0);
        assert!((ahead - Vec3::new(0.0, 0.0, 5.0)).length() < 1.0e-5);

        // Yaw 90° (facing +x): a world point off the tank's +x maps to local forward (+z).
        let yawed = world_to_tank_local(Vec3::new(5.0, 1.0, 0.0), Vec3::ZERO, 1.0, FRAC_PI_2);
        assert!((yawed - Vec3::new(0.0, 0.0, 5.0)).length() < 1.0e-5);
    }

    #[test]
    fn gun_direction_is_unit_length_and_elevates_with_pitch() {
        let dir = gun_direction(0.8, 0.3);
        assert!((dir.length() - 1.0).abs() < 1.0e-6, "gun direction must already be unit length");
        assert!(dir.y > 0.0, "positive pitch elevates the muzzle");
    }

    #[test]
    fn a_longer_barrel_scale_pushes_the_muzzle_further_from_the_trunnion() {
        use crate::VehicleKind;
        let mounts = MountFrames::for_vehicle(VehicleKind::T54_1951);
        let trunnion = mounts.gun_trunnion.translation;
        let stock = muzzle_world_position_scaled(&mounts, Vec3::ZERO, 0.0, 0.0, 0.0, 1.0);
        let long = muzzle_world_position_scaled(&mounts, Vec3::ZERO, 0.0, 0.0, 0.0, 1.2);
        assert!(
            (long - trunnion).length() > (stock - trunnion).length() + 0.5,
            "a +20% barrel must reach noticeably further"
        );
        // The default helper still matches scale 1.0 exactly.
        let default = muzzle_world_position(&mounts, Vec3::ZERO, 0.0, 0.0, 0.0);
        assert!((default - stock).length() < 1.0e-6);
    }

    #[test]
    fn muzzle_world_position_pivots_about_the_trunnion_not_the_hull_centre() {
        use crate::VehicleKind;
        let mounts = MountFrames::for_vehicle(VehicleKind::T55A);
        let trunnion = mounts.gun_trunnion.translation;
        let muzzle = mounts.muzzle.translation;
        let barrel = muzzle.z - trunnion.z;

        // Level gun, no traverse: the muzzle sits exactly on its authored mount.
        let level = muzzle_world_position(&mounts, Vec3::ZERO, 0.0, 0.0, 0.0);
        assert!((level - muzzle).length() < 1.0e-5);

        // Pitched gun: the muzzle rises by sin(pitch) over the *barrel* length from the trunnion
        // — not over the full muzzle.z from the hull centre.
        let pitch = 0.14_f32;
        let pitched = muzzle_world_position(&mounts, Vec3::ZERO, 0.0, 0.0, pitch);
        let expected =
            Vec3::new(0.0, trunnion.y + barrel * pitch.sin(), trunnion.z + barrel * pitch.cos());
        assert!((pitched - expected).length() < 1.0e-4, "{pitched:?} vs {expected:?}");

        // Turret traverse swings the muzzle about the ring; the radius from the ring axis is
        // preserved.
        let yawed = muzzle_world_position(&mounts, Vec3::ZERO, 0.0, FRAC_PI_2, 0.0);
        let ring = mounts.turret_ring.translation;
        let radius = (muzzle - Vec3::new(0.0, muzzle.y, ring.z)).length();
        let swung = yawed - Vec3::new(ring.x, yawed.y, ring.z);
        assert!((swung.length() - radius).abs() < 1.0e-4);
        assert!(yawed.x > 0.0, "positive traverse swings the muzzle to +x");

        // Hull yaw rotates the whole chain about the tank position.
        let hull_yawed =
            muzzle_world_position(&mounts, Vec3::new(3.0, 0.0, -2.0), FRAC_PI_2, 0.0, 0.0);
        let expected_hull = Vec3::new(3.0 + muzzle.z, muzzle.y, -2.0);
        assert!((hull_yawed - expected_hull).length() < 1.0e-4);
    }

    #[test]
    fn wrap_angle_folds_into_minus_pi_to_pi() {
        assert!((wrap_angle(0.0)).abs() < 1.0e-6);
        assert!((wrap_angle(TAU)).abs() < 1.0e-6);
        assert!((wrap_angle(PI) - PI).abs() < 1.0e-6);
        assert!((wrap_angle(3.0 * PI) - PI).abs() < 1.0e-5);
        assert!((wrap_angle(-3.0 * FRAC_PI_2) - FRAC_PI_2).abs() < 1.0e-5);
    }

    #[test]
    fn lerp_angle_takes_the_short_way_across_the_wrap() {
        // From just below +PI to just above -PI the short arc steps *up* through PI, not all the
        // way back down through zero.
        let result = lerp_angle(PI - 0.1, -PI + 0.1, 0.5);
        assert!(wrap_angle(result - PI).abs() < 1.0e-5, "midpoint should sit at the +/-PI seam");
    }

    #[test]
    fn segment_box_entry_reports_entry_inside_and_misses() {
        let min = Vec3::splat(-1.0);
        let max = Vec3::splat(1.0);

        // Crossing from outside on -x into the unit box: enters at the -1 face.
        let t = segment_box_entry(Vec3::new(-3.0, 0.0, 0.0), Vec3::new(3.0, 0.0, 0.0), min, max)
            .expect("segment crosses the box");
        assert!((t - (2.0 / 6.0)).abs() < 1.0e-6);

        // Starting inside returns 0.
        assert_eq!(segment_box_entry(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), min, max), Some(0.0));

        // Parallel miss above the box never overlaps.
        assert_eq!(
            segment_box_entry(Vec3::new(-3.0, 5.0, 0.0), Vec3::new(3.0, 5.0, 0.0), min, max),
            None
        );
    }
}
