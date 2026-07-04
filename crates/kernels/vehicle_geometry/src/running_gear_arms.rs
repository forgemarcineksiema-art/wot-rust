//! Trailing swing arms (torsion-bar suspension) for the animatable running gear: the visible
//! link between the hull tub and each road wheel. The arm pivots at its hull boss and rotates
//! with the wheel's live vertical travel, so the suspension visibly WORKS over terrain instead
//! of the wheels floating beside the hull.

use glam::{Mat4, Vec2, Vec3};

use crate::running_gear::RunningGearKinematics;
use crate::{
    Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec,
    SmoothingGroup,
};

const SG_HARD: SmoothingGroup = SmoothingGroup::hard_edges();
const SG_ARM: SmoothingGroup = SmoothingGroup(6);

/// Horizontal reach from the hull pivot back to the axle (trailing arm, T-54 style).
const ARM_REACH_M: f32 = 0.26;
/// How far the pivot sits above the axle line at rest.
const ARM_RISE_M: f32 = 0.13;
/// Arm plate half-thickness along the axle.
const ARM_HALF_X: f32 = 0.045;

/// One trailing swing arm, authored with the HULL PIVOT at the origin and the axle tip at
/// `(0, -ARM_RISE_M, -ARM_REACH_M)`: a tapered forged arm, a pivot boss, and an axle stub that
/// reaches outboard into the road wheel's hub.
pub fn swing_arm_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments.max(12);
    let tip = Vec2::new(-ARM_RISE_M, -ARM_REACH_M);
    let along = tip.normalize_or_zero();
    let across = Vec2::new(-along.y, along.x);
    // Tapered arm plate in the (y, z) plane: wide at the boss, narrower at the axle.
    let section = vec![-across * 0.055, across * 0.055, tip + across * 0.040, tip - across * 0.040];
    MeshBuilder::new()
        .extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section,
                axis: Axis::X,
                half_depth: ARM_HALF_X,
                material: MaterialRole::TrackMetal,
                smoothing: SG_HARD,
            },
        )
        .append(&stub(Vec3::ZERO, 0.075, ARM_HALF_X * 1.5, seg))
        .append(&stub(Vec3::new(0.0, tip.x, tip.y), 0.055, ARM_HALF_X * 2.4, seg))
        .build()
}

/// A short capped cylinder along the axle axis: the pivot boss / axle stub.
fn stub(center: Vec3, radius: f32, half_width: f32, segments: usize) -> GeometryMesh {
    MeshBuilder::new()
        .capped_revolve_at(
            center,
            RevolveSpec {
                profile: vec![
                    ProfilePoint::new(radius, -half_width),
                    ProfilePoint::new(radius, half_width),
                ],
                axis: Axis::X,
                segments,
                material: MaterialRole::TrackMetal,
                smoothing: SG_ARM,
            },
        )
        .build()
}

/// Placement of the swing arm for the wheel at hull-local `wheel_z` with live vertical `travel`:
/// the pivot sits inboard of the wheel face, ahead of and above the axle, and the arm rotates
/// about it so the authored tip lands on the wheel's current axle height.
pub(crate) fn swing_arm_transform(
    kin: &RunningGearKinematics,
    side_sign: f32,
    wheel_z: f32,
    travel: f32,
) -> Mat4 {
    let arm_x = side_sign * (kin.wheel_x - kin.wheel_half_width - ARM_HALF_X);
    let pivot = Vec3::new(arm_x, kin.cy + ARM_RISE_M, wheel_z + ARM_REACH_M);
    let swing = (travel / ARM_REACH_M).clamp(-1.0, 1.0).asin();
    Mat4::from_translation(pivot) * Mat4::from_rotation_x(swing)
}
