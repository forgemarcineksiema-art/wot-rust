//! Armor-plate geometry: the TRUE 3D outward normal of a hit zone, plate slope included. The
//! impact angle against this normal is the real angle of incidence — slope must never be added
//! to an angle anywhere downstream.

use glam::{Mat3, Vec3};

use super::HullPose;
use crate::ArmorZone;

/// Outward armor-plate normal for a hit zone, in world space — the TRUE 3D normal, plate slope
/// included.
///
/// The facet's `slope_degrees` tilts the plate normal upward out of the vertical (a 60° glacis
/// leans back, so its normal points up-and-forward). The impact angle against this normal is the
/// real angle of incidence: a flat shot meets the glacis at its slope, a plunging shell meets it
/// far squarer (plunging fire DEFEATS sloped armor), and a nose-up hull-down pose steepens it.
/// Slope must NOT be added to the impact angle anywhere downstream — it lives here, in geometry.
///
/// Turret plates rotate with the turret about the ring axis; hull plates stay hull-aligned; the
/// roof faces straight up. `local_x` (the hit's sideways offset in the plate's frame) picks which
/// side a side hit faces.
pub fn plate_normal(
    hull: HullPose,
    turret_yaw_rad: f32,
    zone: ArmorZone,
    local_x: f32,
    slope_degrees: f32,
) -> Vec3 {
    let (sin, cos) = slope_degrees.abs().to_radians().sin_cos();
    let side = if local_x >= 0.0 { 1.0 } else { -1.0 };
    let turret = Mat3::from_rotation_y(turret_yaw_rad);
    let local = match zone {
        ArmorZone::UpperGlacis | ArmorZone::LowerPlate => Vec3::new(0.0, sin, cos),
        ArmorZone::HullRear => Vec3::new(0.0, sin, -cos),
        ArmorZone::HullSide | ArmorZone::LeftTrack | ArmorZone::RightTrack | ArmorZone::Skirt => {
            Vec3::new(side * cos, sin, 0.0)
        }
        ArmorZone::Roof => Vec3::Y,
        ArmorZone::TurretFront | ArmorZone::Mantlet => turret * Vec3::new(0.0, sin, cos),
        ArmorZone::TurretSide => turret * Vec3::new(side * cos, sin, 0.0),
        ArmorZone::TurretRear => turret * Vec3::new(0.0, sin, -cos),
    };
    hull.basis() * local
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use super::super::hull_basis;
    use super::*;

    #[test]
    fn plate_normals_tilt_with_the_hull() {
        // A nose-up hull points its glacis normal upward: shells arriving level meet the plate
        // at a steeper effective angle — terrain angling works on armor.
        let nosed_up = HullPose { yaw_rad: 0.0, pitch_rad: 0.3, roll_rad: 0.0 };
        let glacis = plate_normal(nosed_up, 0.0, ArmorZone::UpperGlacis, 0.0, 0.0);
        assert!(glacis.y > 0.29, "glacis normal gains lift with hull pitch, got {glacis:?}");
        // Level hulls with unsloped plates keep the planar normals.
        let level = plate_normal(HullPose::level(0.0), 0.0, ArmorZone::UpperGlacis, 0.0, 0.0);
        assert!((level - Vec3::Z).length() < 1.0e-6);
    }

    #[test]
    fn plate_slope_lives_in_the_normal_not_in_an_angle_sum() {
        // A 60° glacis on a level hull: the normal leans up-and-forward by exactly the slope.
        let sloped = plate_normal(HullPose::level(0.0), 0.0, ArmorZone::UpperGlacis, 0.0, 60.0);
        let expected = Vec3::new(0.0, 60.0f32.to_radians().sin(), 60.0f32.to_radians().cos());
        assert!((sloped - expected).length() < 1.0e-6, "got {sloped:?}");

        // A level flat shot into that glacis meets it at the slope angle...
        let flat_shot = -Vec3::Z;
        let flat_angle = (-flat_shot).dot(sloped).clamp(-1.0, 1.0).acos().to_degrees();
        assert!((flat_angle - 60.0).abs() < 1.0e-3, "flat shot angle {flat_angle}");

        // ...while a 45° plunging shell meets the reclined plate far squarer. Plunging fire
        // DEFEATS sloped armor — the exact case the old angle-addition model inverted.
        let plunging = Vec3::new(0.0, -1.0, -1.0).normalize();
        let plunge_angle = (-plunging).dot(sloped).clamp(-1.0, 1.0).acos().to_degrees();
        assert!((plunge_angle - 15.0).abs() < 1.0e-3, "plunging angle {plunge_angle}");

        // The roof faces straight up regardless of slope data or turret traverse.
        let roof = plate_normal(HullPose::level(0.7), 1.1, ArmorZone::Roof, 0.3, 30.0);
        assert!((roof - hull_basis(0.7, 0.0, 0.0) * Vec3::Y).length() < 1.0e-6);
    }

    #[test]
    fn plate_normal_follows_turret_rotation_while_hull_stays_put() {
        // Hull points down +z; turret traversed 90° so its front faces the hull's right (+x).
        let turret_front =
            plate_normal(HullPose::level(0.0), FRAC_PI_2, ArmorZone::TurretFront, 0.0, 0.0);
        let hull_front =
            plate_normal(HullPose::level(0.0), FRAC_PI_2, ArmorZone::UpperGlacis, 0.0, 0.0);
        assert!((turret_front - Vec3::new(1.0, 0.0, 0.0)).length() < 1.0e-5);
        assert!((hull_front - Vec3::new(0.0, 0.0, 1.0)).length() < 1.0e-5);

        // Side hits pick the side from the hull-local x sign; tracks share the side normal.
        let right = plate_normal(HullPose::level(0.0), 0.0, ArmorZone::HullSide, 1.0, 0.0);
        let left = plate_normal(HullPose::level(0.0), 0.0, ArmorZone::LeftTrack, -1.0, 0.0);
        assert!((right - Vec3::new(1.0, 0.0, 0.0)).length() < 1.0e-5);
        assert!((left - Vec3::new(-1.0, 0.0, 0.0)).length() < 1.0e-5);
    }
}
