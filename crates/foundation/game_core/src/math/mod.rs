//! Small shared geometry and angle helpers used across the `physics`, `sim`, and `client` crates.
//!
//! These are deliberately data-free, allocation-free leaf functions. They live in `game_core` —
//! the data crate every gameplay crate already depends on — so the turret-direction, angle-wrap,
//! and ray/box math has a single source of truth instead of being re-derived (and drifting) in
//! each crate.

use glam::{Mat3, Vec3};

use crate::MountFrames;

mod plates;

pub use plates::plate_normal;

/// Mildly exaggerated gravity so shell arcs read at map scale (real is ~9.81 m/s^2).
pub const GRAVITY_MPS2: f32 = 12.0;

/// One semi-implicit Euler step of shell flight: linear drag bleeds speed, then gravity pulls.
/// This is the ONE integration every consumer shares — the authoritative server step, the
/// reticle's ballistic preview, and the gun-elevation solver — so a previewed arc is exactly
/// the arc the server flies.
pub fn integrate_shell_step(velocity: &mut Vec3, drag_per_s: f32, dt_seconds: f32) {
    *velocity *= (1.0 - drag_per_s * dt_seconds).max(0.0);
    velocity.y -= GRAVITY_MPS2 * dt_seconds;
}

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

/// The hull's world basis from its authoritative orientation: yaw about Y, then pitch, then
/// roll. `+pitch` is nose up (glam's `Rx(+t)` tips local `+Z` downward, so pitch enters
/// negated); `+roll` is right side up (`Rz(+t)` lifts local `+X` directly).
///
/// This is the ONE hull-orientation convention: the simulation (muzzle chain, armor normals,
/// hitbox frame) and the render pose (`client`'s `VehiclePose`) must both build their hull frame
/// through it, or the visible tank and the simulated tank tilt apart on every slope.
pub fn hull_basis(yaw_rad: f32, pitch_rad: f32, roll_rad: f32) -> Mat3 {
    Mat3::from_rotation_y(yaw_rad)
        * Mat3::from_rotation_x(-pitch_rad)
        * Mat3::from_rotation_z(roll_rad)
}

/// The hull's full authoritative orientation. Everything that hangs off the hull — the muzzle
/// chain, armor-plate normals, the hitbox frame — takes this instead of a bare yaw, so hull
/// tilt reaches every combat computation through one type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HullPose {
    pub yaw_rad: f32,
    /// +nose up — see [`hull_basis`].
    pub pitch_rad: f32,
    /// +right side up — see [`hull_basis`].
    pub roll_rad: f32,
}

impl HullPose {
    /// A level hull (the planar pre-attitude pose; tests and flat-ground fixtures).
    pub fn level(yaw_rad: f32) -> Self {
        Self { yaw_rad, pitch_rad: 0.0, roll_rad: 0.0 }
    }

    pub fn basis(&self) -> Mat3 {
        hull_basis(self.yaw_rad, self.pitch_rad, self.roll_rad)
    }
}

/// World firing direction of a gun on a posed hull: the hull-relative gun direction (turret yaw
/// plus gun pitch, both hull-frame state) carried through the hull basis. On a level hull this
/// degenerates to `gun_direction(hull_yaw + turret_yaw, gun_pitch)`; on a pitched hull the
/// world arc gains the hull's tilt — which is exactly why hull-down works.
pub fn gun_direction_world(hull: HullPose, turret_yaw_rad: f32, gun_pitch_rad: f32) -> Vec3 {
    hull.basis() * gun_direction(turret_yaw_rad, gun_pitch_rad)
}

/// Inverse of [`gun_direction_world`]: the hull-relative `(turret_yaw, gun_pitch)` that points
/// the gun along `world_direction` from a posed hull. The client's gun-laying solves its ballistic
/// arc in world space, then converts through this so the commands it sends respect the hull-frame
/// gun limits.
pub fn world_direction_to_turret(hull: HullPose, world_direction: Vec3) -> (f32, f32) {
    let local = hull.basis().transpose() * world_direction.normalize_or_zero();
    (local.x.atan2(local.z), local.y.clamp(-1.0, 1.0).asin())
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
    hull: HullPose,
    turret_yaw_rad: f32,
    gun_pitch_rad: f32,
) -> Vec3 {
    muzzle_world_position_scaled(mounts, position, hull, turret_yaw_rad, gun_pitch_rad, 1.0)
}

/// As [`muzzle_world_position`], but with the barrel scaled by `barrel_scale` about the trunnion so
/// a longer/shorter installed gun fires from — and is drawn to — its real tip. `barrel_scale` is
/// the installed barrel length over the vehicle's stock barrel length (1.0 = stock).
pub fn muzzle_world_position_scaled(
    mounts: &MountFrames,
    position: Vec3,
    hull: HullPose,
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
    position + hull.basis() * traversed
}

/// Express a world position in a tank's hull-local frame: `x` = right, `y` = up relative to the
/// hitbox center plane, `z` = forward. `center_y_m` lifts the local origin to the hitbox center,
/// riding the hull basis so the frame tilts with the hull.
pub fn world_to_tank_local(
    position: Vec3,
    tank_position: Vec3,
    center_y_m: f32,
    hull: HullPose,
) -> Vec3 {
    let basis = hull.basis();
    let center = tank_position + basis * (Vec3::Y * center_y_m);
    basis.transpose() * (position - center)
}

/// Superellipse coordinate: `sign(c) * |c|^e`, where `e = 2 / exponent`.
///
/// A cast turret's cross-section is a superellipse, and TWO parts of the project have to agree
/// about what that means: the `cast_loft` kernel that skins the visible casting, and the armour
/// volume that a shell is resolved against (`TurretLoftVisual::support`). Two copies of this
/// three-line function would be two definitions of the same steel, so it lives here.
pub fn superlerp(c: f32, e: f32) -> f32 {
    c.signum() * c.abs().powf(e)
}

/// Wrap an angle into the half-open range `(-PI, PI]`.
pub fn wrap_angle(radians: f32) -> f32 {
    let wrapped = radians.rem_euclid(std::f32::consts::TAU);
    if wrapped > std::f32::consts::PI { wrapped - std::f32::consts::TAU } else { wrapped }
}

/// One splitmix64 step: fold `value` (with the golden-ratio increment) into a well-mixed `u64`.
/// The shared deterministic hash for the whole workspace — aim dispersion, FX spread, and the
/// turret pop-off all seed reproducible variation from it, so it lives here rather than being
/// copy-pasted per crate.
pub fn splitmix64(value: u64) -> u64 {
    let mut z = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Deterministically map a seed to `[0, 1)` using the high 24 bits of the shared mixer.
pub fn hash_unit(value: u64) -> f32 {
    ((splitmix64(value) >> 40) as f32) / ((1_u64 << 24) as f32)
}

/// Advance a deterministic stream and return its next unit sample.
pub fn next_hash_unit(state: &mut u64) -> f32 {
    *state = splitmix64(*state);
    ((*state >> 40) as f32) / ((1_u64 << 24) as f32)
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

/// Broadphase reject for segment-vs-AABB queries: whether the segment's XZ bounding rectangle
/// and the box's XZ footprint are disjoint. When they are, no exact slab test can hit — the
/// slab's t-interval is already empty on that axis — so callers walking MANY boxes (an urban
/// map carries 100+) may skip the exact test without changing any result. Four comparisons,
/// no divisions.
#[inline]
pub fn segment_xz_disjoint(
    p0: Vec3,
    p1: Vec3,
    center_x: f32,
    center_z: f32,
    half_x: f32,
    half_z: f32,
) -> bool {
    let (min_x, max_x) = if p0.x <= p1.x { (p0.x, p1.x) } else { (p1.x, p0.x) };
    if center_x - half_x > max_x || center_x + half_x < min_x {
        return true;
    }
    let (min_z, max_z) = if p0.z <= p1.z { (p0.z, p1.z) } else { (p1.z, p0.z) };
    center_z - half_z > max_z || center_z + half_z < min_z
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
    fn hull_basis_lifts_the_nose_with_pitch_and_the_right_side_with_roll() {
        // +pitch: the hull's forward axis (+Z at yaw 0) gains height.
        let nosed_up = hull_basis(0.0, 0.2, 0.0) * Vec3::Z;
        assert!(nosed_up.y > 0.19, "nose up, got {nosed_up:?}");
        // +roll: the hull's right axis (+X at yaw 0) gains height.
        let rolled = hull_basis(0.0, 0.0, 0.2) * Vec3::X;
        assert!(rolled.y > 0.19, "right side up, got {rolled:?}");
        // Yaw-only degenerates to the planar heading.
        let heading = hull_basis(0.7, 0.0, 0.0) * Vec3::Z;
        assert!((heading - horizontal_forward(0.7)).length() < 1.0e-6);
    }

    #[test]
    fn hull_pitch_adds_to_the_world_gun_arc() {
        // The hull-down identity: nose-down hull pitch plus hull-frame gun depression compose
        // into a steeper world arc (at zero turret yaw the elevations simply add).
        let hull = HullPose { yaw_rad: 0.0, pitch_rad: -0.10, roll_rad: 0.0 };
        let dir = gun_direction_world(hull, 0.0, -0.14);
        let world_elevation = dir.y.asin();
        assert!(
            (world_elevation + 0.24).abs() < 1.0e-4,
            "-0.10 hull + -0.14 gun must reach -0.24 world, got {world_elevation}"
        );
    }

    #[test]
    fn world_direction_to_turret_inverts_gun_direction_world() {
        let hull = HullPose { yaw_rad: 0.6, pitch_rad: 0.12, roll_rad: -0.08 };
        let (turret_yaw, gun_pitch) = (0.9, -0.05);
        let dir = gun_direction_world(hull, turret_yaw, gun_pitch);
        let (recovered_yaw, recovered_pitch) = world_direction_to_turret(hull, dir);
        assert!((recovered_yaw - turret_yaw).abs() < 1.0e-5, "yaw {recovered_yaw}");
        assert!((recovered_pitch - gun_pitch).abs() < 1.0e-5, "pitch {recovered_pitch}");
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
    fn world_to_tank_local_maps_forward_right_and_up() {
        // Tank at origin facing +z (yaw 0), hitbox center 1.0 up: a point straight ahead and
        // level with the center maps to pure forward (+z local).
        let ahead =
            world_to_tank_local(Vec3::new(0.0, 1.0, 5.0), Vec3::ZERO, 1.0, HullPose::level(0.0));
        assert!((ahead - Vec3::new(0.0, 0.0, 5.0)).length() < 1.0e-5);

        // Yaw 90° (facing +x): a world point off the tank's +x maps to local forward (+z).
        let yawed = world_to_tank_local(
            Vec3::new(5.0, 1.0, 0.0),
            Vec3::ZERO,
            1.0,
            HullPose::level(FRAC_PI_2),
        );
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
        let stock =
            muzzle_world_position_scaled(&mounts, Vec3::ZERO, HullPose::level(0.0), 0.0, 0.0, 1.0);
        let long =
            muzzle_world_position_scaled(&mounts, Vec3::ZERO, HullPose::level(0.0), 0.0, 0.0, 1.2);
        assert!(
            (long - trunnion).length() > (stock - trunnion).length() + 0.5,
            "a +20% barrel must reach noticeably further"
        );
        // The default helper still matches scale 1.0 exactly.
        let default = muzzle_world_position(&mounts, Vec3::ZERO, HullPose::level(0.0), 0.0, 0.0);
        assert!((default - stock).length() < 1.0e-6);
    }

    #[test]
    fn muzzle_world_position_pivots_about_the_trunnion_not_the_hull_centre() {
        use crate::VehicleKind;
        let mounts = MountFrames::for_vehicle(VehicleKind::T54_1951);
        let trunnion = mounts.gun_trunnion.translation;
        let muzzle = mounts.muzzle.translation;
        let barrel = muzzle.z - trunnion.z;

        // Level gun, no traverse: the muzzle sits exactly on its authored mount.
        let level = muzzle_world_position(&mounts, Vec3::ZERO, HullPose::level(0.0), 0.0, 0.0);
        assert!((level - muzzle).length() < 1.0e-5);

        // Pitched gun: the muzzle rises by sin(pitch) over the *barrel* length from the trunnion
        // — not over the full muzzle.z from the hull centre.
        let pitch = 0.14_f32;
        let pitched = muzzle_world_position(&mounts, Vec3::ZERO, HullPose::level(0.0), 0.0, pitch);
        let expected =
            Vec3::new(0.0, trunnion.y + barrel * pitch.sin(), trunnion.z + barrel * pitch.cos());
        assert!((pitched - expected).length() < 1.0e-4, "{pitched:?} vs {expected:?}");

        // Turret traverse swings the muzzle about the ring; the radius from the ring axis is
        // preserved.
        let yawed =
            muzzle_world_position(&mounts, Vec3::ZERO, HullPose::level(0.0), FRAC_PI_2, 0.0);
        let ring = mounts.turret_ring.translation;
        let radius = (muzzle - Vec3::new(0.0, muzzle.y, ring.z)).length();
        let swung = yawed - Vec3::new(ring.x, yawed.y, ring.z);
        assert!((swung.length() - radius).abs() < 1.0e-4);
        assert!(yawed.x > 0.0, "positive traverse swings the muzzle to +x");

        // Hull yaw rotates the whole chain about the tank position.
        let hull_yawed = muzzle_world_position(
            &mounts,
            Vec3::new(3.0, 0.0, -2.0),
            HullPose::level(FRAC_PI_2),
            0.0,
            0.0,
        );
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
