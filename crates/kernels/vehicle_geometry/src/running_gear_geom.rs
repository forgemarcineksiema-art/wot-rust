//! Geometry for the animatable running gear: the closed belt path sampling and the unit meshes
//! (one road wheel, one end wheel, one shoe link) the renderer instances. Split from
//! [`crate::running_gear`] to keep each module small; the kinematics and placement live there.

use std::f32::consts::PI;

use glam::{Vec2, Vec3};

use crate::running_gear::RunningGearKinematics;
use crate::{Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, SmoothingGroup};

const SG_HARD: SmoothingGroup = SmoothingGroup::hard_edges();

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

/// Rotation about X mapping a link's local +Z onto the belt tangent `(dz, dy)`. A rotation of `θ`
/// about X sends local +Z to world `(y, z) = (-sin θ, cos θ)`, so to land on the tangent `(dy, dz)`
/// the angle is `atan2(-dy, dz)` — negating `dy` here keeps the shoe following the belt the right way
/// up around the end wraps and along the sagging top run (an un-negated `dy` flips it, splaying the
/// guide horns outward on the bends).
fn tangent_rot(dz: f32, dy: f32) -> f32 {
    (-dy).atan2(dz)
}

/// One shoe link, centred at the origin: a short box spanning the belt width (X) whose cross
/// section lies in the Z/Y plane, so a rotation about X aligns it with the belt tangent.
pub fn track_link_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let half_z = kin.link_half_length();
    let plate_half_x = kin.link_half_width * 1.25;
    let guide_half_x = (kin.link_half_width * 0.18).max(0.012);
    let pin_half_z = (half_z * 0.07).max(0.010);

    MeshBuilder::new()
        .append(&box_prism(Vec3::new(0.0, -0.004, 0.0), plate_half_x, 0.026, half_z))
        .append(&box_prism(
            Vec3::new(0.0, -0.031, -half_z * 0.26),
            guide_half_x,
            0.005,
            half_z * 0.12,
        ))
        .append(&box_prism(
            Vec3::new(0.0, -0.031, half_z * 0.26),
            guide_half_x,
            0.005,
            half_z * 0.12,
        ))
        .append(&box_prism(
            Vec3::new(0.0, 0.018, -half_z * 0.78),
            plate_half_x * 0.88,
            0.010,
            pin_half_z,
        ))
        .append(&box_prism(
            Vec3::new(0.0, 0.018, half_z * 0.78),
            plate_half_x * 0.88,
            0.010,
            pin_half_z,
        ))
        .append(&box_prism(
            Vec3::new(-plate_half_x * 0.54, -0.026, 0.0),
            plate_half_x * 0.12,
            0.010,
            half_z * 0.78,
        ))
        .append(&box_prism(
            Vec3::new(plate_half_x * 0.54, -0.026, 0.0),
            plate_half_x * 0.12,
            0.010,
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
