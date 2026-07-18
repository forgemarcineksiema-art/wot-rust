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
const CHRISTIE_REACH_M: f32 = 0.14;
const CHRISTIE_RISE_M: f32 = 0.22;

/// One trailing swing arm, authored with the HULL PIVOT at the origin and the axle tip at
/// `(0, -ARM_RISE_M, -ARM_REACH_M)`: a tapered forged arm, a pivot boss, and an axle stub that
/// reaches outboard into the road wheel's hub.
pub fn swing_arm_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    match kin.suspension {
        game_core::SuspensionKind::TorsionBar => torsion_arm_unit_mesh(kin),
        game_core::SuspensionKind::Christie => christie_crank_unit_mesh(kin),
        game_core::SuspensionKind::Horstmann => horstmann_bogie_unit_mesh(kin),
    }
}

fn torsion_arm_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
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

/// T-34 Christie cue: a compact bell-crank rising steeply into the hull side. The long coil spring
/// itself is internal, so drawing the fleet's generic trailing torsion arm here would advertise the
/// wrong mechanism.
fn christie_crank_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments.max(12);
    let tip = Vec2::new(-CHRISTIE_RISE_M, -CHRISTIE_REACH_M);
    let along = tip.normalize_or_zero();
    let across = Vec2::new(-along.y, along.x);
    let section = vec![-across * 0.060, across * 0.060, tip + across * 0.045, tip - across * 0.045];
    MeshBuilder::new()
        .extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section,
                axis: Axis::X,
                half_depth: ARM_HALF_X * 1.1,
                material: MaterialRole::TrackMetal,
                smoothing: SG_HARD,
            },
        )
        .append(&stub(Vec3::ZERO, 0.082, ARM_HALF_X * 1.6, seg))
        .append(&stub(Vec3::new(0.0, tip.x, tip.y), 0.058, ARM_HALF_X * 2.5, seg))
        .build()
}

/// Centurion Horstmann cue: one shared rocker and central spring housing for each adjacent wheel
/// pair. The mesh is authored around the pair midpoint; placement tilts it with differential wheel
/// travel, so Studio and runtime show three bogies per side rather than six fictional torsion arms.
fn horstmann_bogie_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let pair_half_span =
        kin.wheel_zs.chunks_exact(2).map(|pair| (pair[1] - pair[0]).abs() * 0.5).sum::<f32>()
            / (kin.wheel_zs.len() / 2).max(1) as f32;
    let half_span = pair_half_span.max(0.20);
    let beam = vec![
        Vec2::new(-half_span, -0.055),
        Vec2::new(half_span, -0.055),
        Vec2::new(half_span * 0.86, 0.040),
        Vec2::new(-half_span * 0.86, 0.040),
    ];
    let spring_box = vec![
        Vec2::new(-0.11, 0.025),
        Vec2::new(0.11, 0.025),
        Vec2::new(0.085, 0.34),
        Vec2::new(-0.085, 0.34),
    ];
    MeshBuilder::new()
        .extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section: beam,
                axis: Axis::X,
                half_depth: ARM_HALF_X * 1.35,
                material: MaterialRole::TrackMetal,
                smoothing: SG_HARD,
            },
        )
        .extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section: spring_box,
                axis: Axis::X,
                half_depth: ARM_HALF_X * 1.55,
                material: MaterialRole::TrackMetal,
                smoothing: SG_HARD,
            },
        )
        .append(&stub(Vec3::new(0.0, 0.055, 0.0), 0.075, ARM_HALF_X * 2.0, kin.segments.max(12)))
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
fn torsion_arm_transform(
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

fn christie_crank_transform(
    kin: &RunningGearKinematics,
    side_sign: f32,
    wheel_z: f32,
    travel: f32,
) -> Mat4 {
    let arm_x = side_sign * (kin.wheel_x - kin.wheel_half_width - ARM_HALF_X);
    let pivot = Vec3::new(arm_x, kin.cy + CHRISTIE_RISE_M, wheel_z + CHRISTIE_REACH_M);
    let swing = (travel / CHRISTIE_REACH_M).clamp(-1.0, 1.0).asin();
    Mat4::from_translation(pivot) * Mat4::from_rotation_x(swing)
}

pub(crate) fn suspension_transforms(
    kin: &RunningGearKinematics,
    side_sign: f32,
    travel: &[f32],
) -> Vec<Mat4> {
    let travel_at = |index: usize| travel.get(index).copied().unwrap_or(0.0).clamp(-0.08, 0.20);
    match kin.suspension {
        game_core::SuspensionKind::TorsionBar => kin
            .wheel_zs
            .iter()
            .enumerate()
            .map(|(index, &z)| torsion_arm_transform(kin, side_sign, z, travel_at(index)))
            .collect(),
        game_core::SuspensionKind::Christie => kin
            .wheel_zs
            .iter()
            .enumerate()
            .map(|(index, &z)| christie_crank_transform(kin, side_sign, z, travel_at(index)))
            .collect(),
        game_core::SuspensionKind::Horstmann => kin
            .wheel_zs
            .chunks_exact(2)
            .enumerate()
            .map(|(pair_index, pair)| {
                let first = pair_index * 2;
                let rear_lift = travel_at(first);
                let front_lift = travel_at(first + 1);
                let dz = pair[1] - pair[0];
                let tilt = (front_lift - rear_lift).atan2(dz.abs().max(0.05));
                let arm_x = side_sign * (kin.wheel_x - kin.wheel_half_width - ARM_HALF_X);
                Mat4::from_translation(Vec3::new(
                    arm_x,
                    kin.cy + (rear_lift + front_lift) * 0.5,
                    (pair[0] + pair[1]) * 0.5,
                )) * Mat4::from_rotation_x(tilt)
            })
            .collect(),
    }
}
