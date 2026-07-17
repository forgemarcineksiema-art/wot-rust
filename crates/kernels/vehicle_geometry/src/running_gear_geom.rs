//! Geometry for the animatable running gear: the closed belt path sampling and the unit meshes
//! (one road wheel, one end wheel, one shoe link) the renderer instances. Split from
//! [`crate::running_gear`] to keep each module small; the kinematics and placement live there.

use glam::{Vec2, Vec3};

use crate::running_gear::RunningGearKinematics;
use crate::{Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, SmoothingGroup};

const SG_HARD: SmoothingGroup = SmoothingGroup::hard_edges();

/// One shoe link, centred at the origin: a short box spanning the belt width (X) whose cross
/// section lies in the Z/Y plane, so a rotation about X aligns it with the belt tangent.
/// The PATTERN is per family (audit #14): the same generator used to run on every vehicle
/// across three nations, so the Germans, the Centurion and the T-34 read as wearing the
/// T-54's track. Negative Y is the wheel side (guide horns); positive Y the ground face.
pub fn track_link_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    match kin.shoe {
        game_core::ShoePattern::Omsh => omsh_link(kin),
        game_core::ShoePattern::Kgs => kgs_link(kin),
        game_core::ShoePattern::Waffle => waffle_link(kin),
        game_core::ShoePattern::BritishCast => british_link(kin),
    }
}

/// Soviet small-pitch OMSh (T-54 family, IS-3): flat plate, twin inner guide pads, pin bars
/// at the joints, edge rails.
fn omsh_link(kin: &RunningGearKinematics) -> GeometryMesh {
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

/// German Kgs 63/725 double-pin shoe (Tiger I/II, Jagdtiger, Panther II): a wide plate with
/// one TALL centre guide horn that rides between the interleaved wheel rows, two transverse
/// grouser cleats on the ground face, and prominent pin tubes at both joints.
fn kgs_link(kin: &RunningGearKinematics) -> GeometryMesh {
    let half_z = kin.link_half_length();
    let plate_half_x = kin.band_half_width;
    let pin_half_z = (half_z * 0.09).max(0.012);
    MeshBuilder::new()
        .append(&box_prism(Vec3::new(0.0, -0.004, 0.0), plate_half_x, 0.026, half_z))
        // The single tall centre horn — the Schachtellaufwerk's guide between the wheel rows.
        .append(&box_prism(Vec3::new(0.0, -0.052, 0.0), 0.032, 0.024, half_z * 0.30))
        // Two transverse grouser cleats gripping the ground.
        .append(&box_prism(
            Vec3::new(0.0, 0.022, -half_z * 0.38),
            plate_half_x * 0.96,
            0.008,
            half_z * 0.10,
        ))
        .append(&box_prism(
            Vec3::new(0.0, 0.022, half_z * 0.38),
            plate_half_x * 0.96,
            0.008,
            half_z * 0.10,
        ))
        // Pin tubes at the joints.
        .append(&box_prism(
            Vec3::new(0.0, 0.014, -half_z * 0.82),
            plate_half_x * 0.90,
            0.012,
            pin_half_z,
        ))
        .append(&box_prism(
            Vec3::new(0.0, 0.014, half_z * 0.82),
            plate_half_x * 0.90,
            0.012,
            pin_half_z,
        ))
        .build()
}

/// The T-34's stamped "waffle" plate: three low transverse ridges on the ground face under a
/// low broad centre horn — pressed steel, no separate pin tubes standing proud.
fn waffle_link(kin: &RunningGearKinematics) -> GeometryMesh {
    let half_z = kin.link_half_length();
    let plate_half_x = kin.band_half_width;
    let mut b = MeshBuilder::new()
        .append(&box_prism(Vec3::new(0.0, -0.004, 0.0), plate_half_x, 0.026, half_z))
        // Low broad centre horn.
        .append(&box_prism(Vec3::new(0.0, -0.044, 0.0), 0.045, 0.016, half_z * 0.22));
    // The waffle: three low ridges across the ground face.
    for i in -1..=1 {
        b = b.append(&box_prism(
            Vec3::new(0.0, 0.020, half_z * 0.52 * i as f32),
            plate_half_x * 0.92,
            0.006,
            half_z * 0.11,
        ));
    }
    b.build()
}

/// The Centurion's cast manganese shoe: TWIN spaced guide horns riding either side of the
/// centreline and one heavy transverse bar across the ground face.
fn british_link(kin: &RunningGearKinematics) -> GeometryMesh {
    let half_z = kin.link_half_length();
    let plate_half_x = kin.band_half_width;
    let pin_half_z = (half_z * 0.08).max(0.010);
    MeshBuilder::new()
        .append(&box_prism(Vec3::new(0.0, -0.004, 0.0), plate_half_x, 0.026, half_z))
        // Twin spaced horns.
        .append(&box_prism(
            Vec3::new(-plate_half_x * 0.28, -0.048, 0.0),
            0.026,
            0.020,
            half_z * 0.24,
        ))
        .append(&box_prism(
            Vec3::new(plate_half_x * 0.28, -0.048, 0.0),
            0.026,
            0.020,
            half_z * 0.24,
        ))
        // One heavy transverse bar.
        .append(&box_prism(Vec3::new(0.0, 0.024, 0.0), plate_half_x * 0.94, 0.010, half_z * 0.16))
        .append(&box_prism(
            Vec3::new(0.0, 0.012, -half_z * 0.80),
            plate_half_x * 0.86,
            0.010,
            pin_half_z,
        ))
        .append(&box_prism(
            Vec3::new(0.0, 0.012, half_z * 0.80),
            plate_half_x * 0.86,
            0.010,
            pin_half_z,
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
