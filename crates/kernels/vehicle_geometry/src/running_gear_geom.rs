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
        let u = (t / run.max(0.001)).clamp(0.0, 1.0);
        let sag = kin.top_sag_m * (PI * u).sin();
        let dy_dz = -kin.top_sag_m * PI / run.max(0.001) * (PI * u).cos();
        BeltSample { y: top_y - sag, z: rear_z + t, rot_x: tangent_rot(1.0, dy_dz) }
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
    idler_unit_mesh(kin)
}

/// Smooth front idler wheel, centred at the origin with its axle along X.
pub fn idler_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    wheel_disc(kin.end_radius, kin.wheel_half_width, kin.segments)
}

/// Rear drive sprocket with visible teeth beyond the smooth end-wheel radius.
pub fn sprocket_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let mut builder = MeshBuilder::new().append(&wheel_disc(
        kin.end_radius * 0.88,
        kin.wheel_half_width,
        kin.segments,
    ));
    let teeth = 14usize;
    for i in 0..teeth {
        let angle = (i as f32 / teeth as f32) * std::f32::consts::TAU;
        builder = builder.append(&sprocket_tooth(
            angle,
            kin.end_radius * 0.92,
            kin.end_radius * 1.16,
            kin.wheel_half_width * 1.08,
        ));
    }
    builder.build()
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
    let plate_half_x = kin.link_half_width * 1.45;
    let guide_half_x = (kin.link_half_width * 0.35).max(0.012);
    let pin_half_z = (half_z * 0.10).max(0.012);

    MeshBuilder::new()
        .append(&box_prism(Vec3::new(0.0, -0.02, 0.0), plate_half_x, 0.035, half_z))
        .append(&box_prism(
            Vec3::new(0.0, -0.088, -half_z * 0.28),
            guide_half_x,
            0.052,
            half_z * 0.16,
        ))
        .append(&box_prism(
            Vec3::new(0.0, -0.088, half_z * 0.28),
            guide_half_x,
            0.052,
            half_z * 0.16,
        ))
        .append(&box_prism(
            Vec3::new(0.0, 0.018, -half_z * 0.78),
            plate_half_x * 0.88,
            0.018,
            pin_half_z,
        ))
        .append(&box_prism(
            Vec3::new(0.0, 0.018, half_z * 0.78),
            plate_half_x * 0.88,
            0.018,
            pin_half_z,
        ))
        .append(&box_prism(
            Vec3::new(-plate_half_x * 0.58, -0.055, 0.0),
            plate_half_x * 0.16,
            0.018,
            half_z * 0.78,
        ))
        .append(&box_prism(
            Vec3::new(plate_half_x * 0.58, -0.055, 0.0),
            plate_half_x * 0.16,
            0.018,
            half_z * 0.78,
        ))
        .build()
}

fn box_prism(center: Vec3, half_x: f32, half_y: f32, half_z: f32) -> GeometryMesh {
    MeshBuilder::new()
        .extrude(
            center,
            ExtrudeSpec {
                section: vec![
                    Vec2::new(-half_z, -half_y),
                    Vec2::new(half_z, -half_y),
                    Vec2::new(half_z, half_y),
                    Vec2::new(-half_z, half_y),
                ],
                axis: Axis::X,
                half_depth: half_x,
                material: MaterialRole::TrackMetal,
                smoothing: SG_HARD,
            },
        )
        .build()
}

fn sprocket_tooth(angle: f32, inner_r: f32, outer_r: f32, half_width: f32) -> GeometryMesh {
    let (sin, cos) = angle.sin_cos();
    let radial = Vec2::new(sin, cos);
    let tangent = Vec2::new(cos, -sin);
    let root_half = 0.060;
    let tip_half = 0.038;
    let section = vec![
        radial * inner_r - tangent * root_half,
        radial * inner_r + tangent * root_half,
        radial * outer_r + tangent * tip_half,
        radial * outer_r - tangent * tip_half,
    ];
    MeshBuilder::new()
        .extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section,
                axis: Axis::X,
                half_depth: half_width,
                material: MaterialRole::TrackMetal,
                smoothing: SG_HARD,
            },
        )
        .build()
}
