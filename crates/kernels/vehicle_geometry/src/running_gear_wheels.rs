//! Unit meshes for animatable road wheels, idlers, and drive sprockets.

use glam::{Vec2, Vec3};

use crate::running_gear::RunningGearKinematics;
use crate::{
    Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec,
    SmoothingGroup,
};

const SG_HARD: SmoothingGroup = SmoothingGroup::hard_edges();
const SG_WHEEL: SmoothingGroup = SmoothingGroup(5);

/// One T-54 road wheel, centred at the origin with its axle along X: a *dual* steel disc wheel — two
/// steel faces side by side, each carrying a rubber tire only at the rim, with a proud hub cap in the
/// middle. The steel faces (the lighter material) dominate what you see; the rubber shows only as the
/// dark outer ring. Earlier the rubber was a pair of full-radius coins that buried the steel and read
/// as one black ball, so this exposes the steel disc and keeps the rubber as a tread band.
pub fn road_wheel_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments.max(22);
    let r = kin.wheel_radius;
    let half_w = kin.wheel_half_width;
    // Everything here is *centred* on the axle. Earlier the rubber sat in two x-offset bands, so the
    // far band peeked out behind the steel as a dark ring/crescent that shifted with the view angle.
    // Now one centred steel body occludes its own far face, and one centred tire (with a centre
    // groove for the dual-wheel cue) rings the rim — so nothing sticks out sideways from any angle.
    let body_half = half_w * 0.92;

    MeshBuilder::new()
        // Solid steel disc body, centred — reaching almost to the rim (0.94 r) so it occludes the
        // tire's far edge, leaving the rubber as only a thin rim band instead of a peeking ring.
        .append(&wheel_disc_at(0.0, r * 0.94, body_half, seg, MaterialRole::TrackMetal))
        // A raised concentric flange (symmetric ring, never an offset crescent): the pressed dish.
        .append(&wheel_disc_at(0.0, r * 0.46, body_half * 1.10, seg, MaterialRole::TrackMetal))
        // Proud central hub cap; its capped fan reads as the hub.
        .append(&wheel_disc_at(0.0, r * 0.22, half_w * 1.05, seg, MaterialRole::TrackMetal))
        // One centred rubber tire grooved down the middle — the dual-tire look without offset bands.
        .append(&dual_tire(r, half_w, seg))
        .build()
}

/// A single centred rubber tire whose tread dips to a groove in the middle, giving the T-54 dual-tire
/// read as *one* concentric piece. Built as an uncapped surface of revolution so it rings the rim
/// without a disc cap that would cover the steel face.
fn dual_tire(r: f32, half_w: f32, segments: usize) -> GeometryMesh {
    MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![
                ProfilePoint::new(r * 0.91, -half_w),
                ProfilePoint::new(r, -half_w * 0.80),
                ProfilePoint::new(r, -half_w * 0.34),
                ProfilePoint::new(r * 0.95, 0.0),
                ProfilePoint::new(r, half_w * 0.34),
                ProfilePoint::new(r, half_w * 0.80),
                ProfilePoint::new(r * 0.91, half_w),
            ],
            axis: Axis::X,
            segments,
            material: MaterialRole::Rubber,
            smoothing: SG_WHEEL,
        })
        .build()
}

/// A rubber tire tread: an *uncapped* surface of revolution at `radius`, spanning `center_x ±
/// half_width` along the axle. Uncapped so it reads as the tire ring around the steel face rather
/// than a solid coin that hides it.
fn tread_band(center_x: f32, radius: f32, half_width: f32, segments: usize) -> GeometryMesh {
    MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![
                ProfilePoint::new(radius, center_x - half_width),
                ProfilePoint::new(radius, center_x + half_width),
            ],
            axis: Axis::X,
            segments,
            material: MaterialRole::Rubber,
            smoothing: SG_WHEEL,
        })
        .build()
}

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
        builder = builder.append(&sprocket_tooth(
            angle,
            r * 0.60,
            r * 0.97,
            half_w * 0.82,
        ));
    }
    builder.build()
}

fn wheel_disc_at(
    center_x: f32,
    radius: f32,
    half_width: f32,
    segments: usize,
    material: MaterialRole,
) -> GeometryMesh {
    MeshBuilder::new()
        .capped_revolve_at(
            Vec3::new(center_x, 0.0, 0.0),
            RevolveSpec {
                profile: vec![
                    ProfilePoint::new(radius, -half_width),
                    ProfilePoint::new(radius, half_width),
                ],
                axis: Axis::X,
                segments,
                material,
                smoothing: SG_WHEEL,
            },
        )
        .build()
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
