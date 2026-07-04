//! Unit meshes for animatable road wheels, idlers, and drive sprockets.

use glam::{Vec2, Vec3};

use crate::running_gear::RunningGearKinematics;
use crate::{
    Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec,
    SmoothingGroup,
};

const SG_HARD: SmoothingGroup = SmoothingGroup::hard_edges();
const SG_WHEEL: SmoothingGroup = SmoothingGroup(5);

/// One T-54 "starfish" road wheel, centred at the origin with its axle along X: a steel rim ring
/// under the rubber tire, an openwork face of radial spoke arms over a RECESSED centre web (the
/// see-into depth between the arms is the starfish read), and a proud hub cap. The rubber shows
/// only as the dark outer band; the earlier solid disc face read as a blank plate.
pub fn road_wheel_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments.max(22);
    let r = kin.wheel_radius;
    let half_w = kin.wheel_half_width;
    let body_half = half_w * 0.92;

    let mut builder = MeshBuilder::new()
        // Full-width steel rim ring seated under the tire (closed rectangular profile revolved:
        // outer wall, both side annuli, inner wall) — solid at the rim, open inboard of it. The
        // tire's side lips overlap it RADIALLY (down to 0.86 r) but sit on wider planes, so there
        // is neither a see-through gap between tire and ring nor a coplanar face to z-fight.
        .append(&steel_ring(r * 0.66, r * 0.895, body_half, seg))
        // Recessed centre web the openwork reads against: thin, well inboard of the rim faces.
        .append(&wheel_disc_at(0.0, r * 0.72, body_half * 0.28, seg, MaterialRole::TrackMetal))
        // Proud central hub cap; its capped fan reads as the hub.
        .append(&wheel_disc_at(0.0, r * 0.22, half_w * 1.05, seg, MaterialRole::TrackMetal))
        // One centred rubber tire grooved down the middle — the dual-tire look without offset bands.
        .append(&dual_tire(r, half_w, seg));
    // Six radial starfish arms bridging hub to rim, proud of the recessed web. Their tips are
    // BURIED radially in the rim ring and the hub, and their faces sit just INBOARD of the ring's
    // side annuli (0.94 x body width vs the ring's full body width) — an arm face flush with the
    // ring's side plane z-fights across the whole overlap band.
    let arms = 6usize;
    for i in 0..arms {
        let angle = (i as f32 / arms as f32) * std::f32::consts::TAU;
        builder = builder.append(&spoke_arm(angle, r * 0.16, r * 0.80, r, body_half * 0.94));
    }
    builder.build()
}

/// A full-width steel ring (closed rectangular profile revolved about the axle): the wheel rim.
fn steel_ring(r_in: f32, r_out: f32, half_width: f32, segments: usize) -> GeometryMesh {
    MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![
                ProfilePoint::new(r_in, -half_width),
                ProfilePoint::new(r_out, -half_width),
                ProfilePoint::new(r_out, half_width),
                ProfilePoint::new(r_in, half_width),
                ProfilePoint::new(r_in, -half_width),
            ],
            axis: Axis::X,
            segments,
            material: MaterialRole::TrackMetal,
            smoothing: SG_WHEEL,
        })
        .build()
}

/// One radial starfish arm: a tapered prism from the hub seat out to the rim, spanning the full
/// body width so it stands proud of the recessed web behind it.
fn spoke_arm(angle: f32, inner_r: f32, outer_r: f32, wheel_r: f32, half_width: f32) -> GeometryMesh {
    let (sin, cos) = angle.sin_cos();
    let radial = Vec2::new(sin, cos);
    let tangent = Vec2::new(cos, -sin);
    let (w_in, w_out) = (wheel_r * 0.10, wheel_r * 0.16);
    let section = vec![
        radial * inner_r - tangent * w_in,
        radial * inner_r + tangent * w_in,
        radial * outer_r + tangent * w_out,
        radial * outer_r - tangent * w_out,
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

/// A single centred rubber tire whose tread dips to a groove in the middle, giving the T-54 dual-tire
/// read as *one* concentric piece. Built as an uncapped surface of revolution so it rings the rim
/// without a disc cap that would cover the steel face. Its side lips run inward to 0.86 r —
/// radially UNDER the steel ring's outer wall (0.895 r) on a wider plane — so no annular window
/// opens between tire and ring (an open ring flickers as the spokes sweep behind it).
fn dual_tire(r: f32, half_w: f32, segments: usize) -> GeometryMesh {
    MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![
                ProfilePoint::new(r * 0.86, -half_w),
                ProfilePoint::new(r * 0.91, -half_w),
                ProfilePoint::new(r, -half_w * 0.80),
                ProfilePoint::new(r, -half_w * 0.34),
                // Groove deep enough that the shoes' guide horns ride INSIDE it on the ground
                // run and the end wraps (the 0.02 link seat leaves the horns ~1.6 cm proud).
                ProfilePoint::new(r * 0.92, 0.0),
                ProfilePoint::new(r, half_w * 0.34),
                ProfilePoint::new(r, half_w * 0.80),
                ProfilePoint::new(r * 0.91, half_w),
                ProfilePoint::new(r * 0.86, half_w),
            ],
            axis: Axis::X,
            segments,
            material: MaterialRole::Rubber,
            smoothing: SG_WHEEL,
        })
        .build()
}

pub(crate) fn wheel_disc_at(
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

