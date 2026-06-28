//! Geometry for the animatable running gear: the closed belt path sampling and the unit meshes
//! (one road wheel, one end wheel, one shoe link) the renderer instances. Split from
//! [`crate::running_gear`] to keep each module small; the kinematics and placement live there.

use std::f32::consts::PI;

use glam::{Vec2, Vec3};

use crate::running_gear::RunningGearKinematics;
use crate::{
    Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec,
    SmoothingGroup,
};

const SG_HARD: SmoothingGroup = SmoothingGroup::hard_edges();
const SG_WHEEL: SmoothingGroup = SmoothingGroup(5);

/// A sampled point on the belt loop: position in the side plane and the link rotation about X.
pub(crate) struct BeltSample {
    pub y: f32,
    pub z: f32,
    /// Rotation about X that aligns a link's local +Z with the belt tangent.
    pub rot_x: f32,
}

/// Sample the closed belt loop at arc length `s` in `[0, belt_length)`. The loop runs: bottom run
/// (front→rear) → rear semicircle → top run (rear→front) → front semicircle.
pub(crate) fn sample_belt(kin: &RunningGearKinematics, s: f32) -> BeltSample {
    let r = kin.belt_wrap_radius();
    let run = 2.0 * kin.half_run;
    let arc = PI * r;
    let front_z = kin.cz + kin.half_run;
    let rear_z = kin.cz - kin.half_run;
    let bottom_y = kin.cy - r;
    let top_y = kin.cy + r;

    if s < run {
        // Bottom run: front -> rear, tangent toward -Z.
        BeltSample { y: bottom_y, z: front_z - s, rot_x: tangent_rot(-1.0, 0.0) }
    } else if s < run + arc {
        // Rear semicircle around (rear_z, cy): bottom -> top through the rear (-Z).
        let theta = -PI / 2.0 - (s - run) / r; // -90deg sweeping to -270deg (through rear)
        let z = rear_z + r * theta.cos();
        let y = kin.cy + r * theta.sin();
        // theta decreases with s (dtheta/ds = -1/r), so the unit tangent is (sin theta, -cos theta).
        BeltSample { y, z, rot_x: tangent_rot(theta.sin(), -theta.cos()) }
    } else if s < run + arc + run {
        // Top run: rear -> front, tangent toward +Z.
        let t = s - run - arc;
        BeltSample { y: top_y, z: rear_z + t, rot_x: tangent_rot(1.0, 0.0) }
    } else {
        // Front semicircle around (front_z, cy): top -> bottom through the front (+Z).
        let theta = PI / 2.0 - (s - run - arc - run) / r;
        let z = front_z + r * theta.cos();
        let y = kin.cy + r * theta.sin();
        BeltSample { y, z, rot_x: tangent_rot(theta.sin(), -theta.cos()) }
    }
}

/// Rotation about X mapping a link's local +Z onto the belt tangent `(dz, dy)`.
fn tangent_rot(dz: f32, dy: f32) -> f32 {
    dy.atan2(dz)
}

/// One road-wheel disc, centred at the origin with its axle along X (so a rotation about X spins
/// it in place). The client caches this once per vehicle and instances it via the placements.
pub fn road_wheel_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    wheel_disc(kin.wheel_radius, kin.wheel_half_width, kin.segments)
}

/// The larger end wheel (drive sprocket / idler), centred at the origin with its axle along X.
pub fn end_wheel_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    wheel_disc(kin.end_radius, kin.wheel_half_width, kin.segments)
}

fn wheel_disc(radius: f32, half_width: f32, segments: usize) -> GeometryMesh {
    MeshBuilder::new()
        .capped_revolve_at(
            Vec3::ZERO,
            RevolveSpec {
                profile: vec![
                    ProfilePoint::new(radius, -half_width),
                    ProfilePoint::new(radius, half_width),
                ],
                axis: Axis::X,
                segments,
                material: MaterialRole::Rubber,
                smoothing: SG_WHEEL,
            },
        )
        .build()
}

/// One shoe link, centred at the origin: a short box spanning the belt width (X) whose cross
/// section lies in the Z/Y plane, so a rotation about X aligns it with the belt tangent.
pub fn track_link_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let half_z = kin.link_half_length();
    let half_y = 0.07;
    let section = vec![
        Vec2::new(-half_z, -half_y),
        Vec2::new(half_z, -half_y),
        Vec2::new(half_z, half_y),
        Vec2::new(-half_z, half_y),
    ];
    MeshBuilder::new()
        .extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section,
                axis: Axis::X,
                half_depth: kin.link_half_width,
                material: MaterialRole::TrackMetal,
                smoothing: SG_HARD,
            },
        )
        .build()
}
