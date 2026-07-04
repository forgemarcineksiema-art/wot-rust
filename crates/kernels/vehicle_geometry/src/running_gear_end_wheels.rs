//! Unit meshes for the belt's end wheels — the smooth front idler and the toothed rear drive
//! sprocket. Split from `running_gear_wheels` (the road wheel) to stay within the reviewability
//! budget.

use glam::{Vec2, Vec3};

use crate::running_gear::RunningGearKinematics;
use crate::running_gear_wheels::wheel_disc_at;
use crate::{
    Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec,
    SmoothingGroup,
};

const SG_HARD: SmoothingGroup = SmoothingGroup::hard_edges();
const SG_WHEEL: SmoothingGroup = SmoothingGroup(5);

/// The larger end wheel (drive sprocket / idler), centred at the origin with its axle along X.
pub fn end_wheel_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    idler_unit_mesh(kin)
}

/// Smooth front idler wheel, centred at the origin with its axle along X. Built as a steel disc
/// wheel with a rubber tire rim and a proud hub — a *smooth* sibling of the road wheel, so the front
/// of the track reads as a plain wheel against the toothed drive sprocket at the rear.
pub fn idler_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments.max(20);
    let r = kin.end_radius;
    let half_w = kin.wheel_half_width;
    MeshBuilder::new()
        .append(&wheel_disc_at(0.0, r * 0.86, half_w, seg, MaterialRole::TrackMetal))
        .append(&tread_band(0.0, r, half_w * 0.9, seg))
        .append(&wheel_disc_at(0.0, r * 0.28, half_w * 1.12, seg, MaterialRole::TrackMetal))
        .build()
}

/// A rubber tire tread ring at `radius`, spanning `center_x ± half_width` along the axle, with
/// side lips running inward to 0.82·radius. The lips radially overlap the idler's steel disc
/// (0.86 r, on wider planes), so the band reads as a solid tire instead of opening an annular
/// see-through window between tread and disc.
fn tread_band(center_x: f32, radius: f32, half_width: f32, segments: usize) -> GeometryMesh {
    MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![
                ProfilePoint::new(radius * 0.82, center_x - half_width),
                ProfilePoint::new(radius, center_x - half_width),
                ProfilePoint::new(radius, center_x + half_width),
                ProfilePoint::new(radius * 0.82, center_x + half_width),
            ],
            axis: Axis::X,
            segments,
            material: MaterialRole::Rubber,
            smoothing: SG_WHEEL,
        })
        .build()
}

/// Rear drive sprocket: a small steel hub plate ringed by distinct teeth, so it reads as a toothed
/// drive wheel — visibly different from the smooth front idler. The teeth stand clear of a smaller
/// central plate (rather than a full end-wheel disc) so they are not swallowed by the wrapping links.
pub fn sprocket_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments.max(16);
    let r = kin.end_radius;
    let half_w = kin.wheel_half_width;
    let mut builder = MeshBuilder::new()
        .append(&wheel_disc_at(0.0, r * 0.62, half_w, seg, MaterialRole::TrackMetal))
        .append(&wheel_disc_at(0.0, r * 0.26, half_w * 1.15, seg, MaterialRole::TrackMetal));
    let teeth = 16usize;
    for i in 0..teeth {
        let angle = (i as f32 / teeth as f32) * std::f32::consts::TAU;
        builder = builder.append(&sprocket_tooth(angle, r * 0.60, r * 0.97, half_w * 0.82));
    }
    builder.build()
}

fn sprocket_tooth(angle: f32, inner_r: f32, outer_r: f32, half_width: f32) -> GeometryMesh {
    let (sin, cos) = angle.sin_cos();
    let radial = Vec2::new(sin, cos);
    let tangent = Vec2::new(cos, -sin);
    let section = vec![
        radial * inner_r - tangent * 0.060,
        radial * inner_r + tangent * 0.060,
        radial * outer_r + tangent * 0.038,
        radial * outer_r - tangent * 0.038,
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
