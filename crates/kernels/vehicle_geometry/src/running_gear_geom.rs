//! Geometry for the animatable running gear: the closed belt path sampling and the unit meshes
//! (one road wheel, one end wheel, one shoe link) the renderer instances. Split from
//! [`crate::running_gear`] to keep each module small; the kinematics and placement live there.

use glam::{Vec2, Vec3};

use crate::running_gear::RunningGearKinematics;
use crate::{Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, SmoothingGroup};

const SG_HARD: SmoothingGroup = SmoothingGroup::hard_edges();

/// One shoe link, centred at the origin: a short box spanning the belt width (X) whose cross
/// section lies in the Z/Y plane, so a rotation about X aligns it with the belt tangent.
pub fn track_link_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let half_z = kin.link_half_length();
    // The shoe plate spans the full belt band, so its outer face sits AT the blueprint's
    // `outer_x` — the documented "width over tracks". (It used to be `link_half_width * 1.25`,
    // which left the band underfilled and pushed the sprocket rings proud of the real width.)
    let plate_half_x = kin.band_half_width;
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
