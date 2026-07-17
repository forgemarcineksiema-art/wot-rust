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
    match kin.wheel_face {
        game_core::WheelFace::Openwork => openwork_wheel(kin),
        game_core::WheelFace::SteelDish => dished_wheel(kin, false),
        game_core::WheelFace::RubberDish => dished_wheel(kin, true),
    }
}

/// The openwork Soviet family face: spokes/ribs over a recessed web (T-54 starfish, IS ribs,
/// T-34 spoked) — the original construction, now ONE of the family reads (audit #14).
fn openwork_wheel(kin: &RunningGearKinematics) -> GeometryMesh {
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
    // Radial arms bridging hub to rim, proud of the recessed web: the T-54's six-arm starfish
    // or the IS family's denser rib casting (`wheel_spokes`). Their tips are BURIED radially in
    // the rim ring and the hub, and their faces sit just INBOARD of the ring's side annuli
    // (0.94 x body width vs the ring's full body width) — an arm face flush with the ring's
    // side plane z-fights across the whole overlap band.
    let arms = kin.wheel_spokes.max(3);
    for i in 0..arms {
        let angle = (i as f32 / arms as f32) * std::f32::consts::TAU;
        builder = builder.append(&spoke_arm(angle, r * 0.16, r * 0.80, r, body_half * 0.94));
    }
    builder.build()
}

/// A bolted DISH wheel: a shallow cone face from hub to rim with a bolt ring — the German
/// late-war steel-rimmed wheel (`rubber_tire == false`: the tire band itself is steel) and the
/// Centurion's rubber-tired dish (`rubber_tire == true`). No openwork: the dish IS the face.
fn dished_wheel(kin: &RunningGearKinematics, rubber_tire: bool) -> GeometryMesh {
    let seg = kin.segments.max(22);
    let r = kin.wheel_radius;
    let half_w = kin.wheel_half_width;
    let body_half = half_w * 0.92;

    let mut builder = MeshBuilder::new()
        // Closed conical dish: proud at the hub, falling to the rim seat on both faces.
        .append(
            &MeshBuilder::new()
                .revolve(RevolveSpec {
                    profile: vec![
                        ProfilePoint::new(r * 0.16, body_half * 0.62),
                        ProfilePoint::new(r * 0.88, body_half * 0.16),
                        ProfilePoint::new(r * 0.88, -body_half * 0.16),
                        ProfilePoint::new(r * 0.16, -body_half * 0.62),
                    ],
                    axis: Axis::X,
                    segments: seg,
                    material: MaterialRole::TrackMetal,
                    smoothing: SG_WHEEL,
                })
                .build(),
        )
        // Proud hub cap.
        .append(&wheel_disc_at(0.0, r * 0.20, half_w * 1.05, seg, MaterialRole::TrackMetal));
    // The bolt ring on both faces: the dish read is the bolts.
    let bolts = 8;
    for i in 0..bolts {
        let angle = (i as f32 / bolts as f32) * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        for side in [-1.0_f32, 1.0] {
            builder = builder.append(
                &MeshBuilder::new()
                    .extrude(
                        Vec3::new(side * body_half * 0.40, sin * r * 0.52, cos * r * 0.52),
                        ExtrudeSpec {
                            section: vec![
                                Vec2::new(-0.018, -0.018),
                                Vec2::new(0.018, -0.018),
                                Vec2::new(0.018, 0.018),
                                Vec2::new(-0.018, 0.018),
                            ],
                            axis: Axis::X,
                            half_depth: 0.014,
                            material: MaterialRole::TrackMetal,
                            smoothing: SG_HARD,
                        },
                    )
                    .build(),
            );
        }
    }
    if rubber_tire {
        builder = builder.append(&dual_tire(r, half_w, seg));
    } else {
        // Steel tire band: same silhouette as the rubber, cut in steel (no groove).
        builder = builder.append(
            &MeshBuilder::new()
                .revolve(RevolveSpec {
                    profile: vec![
                        ProfilePoint::new(r * 0.84, -half_w),
                        ProfilePoint::new(r, -half_w * 0.75),
                        ProfilePoint::new(r, half_w * 0.75),
                        ProfilePoint::new(r * 0.84, half_w),
                    ],
                    axis: Axis::X,
                    segments: seg,
                    material: MaterialRole::TrackMetal,
                    smoothing: SG_WHEEL,
                })
                .build(),
        );
    }
    builder.build()
}

/// One return roller: a small rubber-rimmed carrier wheel for the top run (IS family), centred
/// at the origin with its axle along X — a compact steel hub disc under a rubber band.
pub fn return_roller_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments.max(16);
    let r = kin.roller_radius.max(0.05);
    let half_w = kin.wheel_half_width * 0.55;
    MeshBuilder::new()
        .append(&wheel_disc_at(0.0, r * 0.72, half_w * 0.9, seg, MaterialRole::TrackMetal))
        .append(&wheel_disc_at(0.0, r * 0.30, half_w * 1.1, seg, MaterialRole::TrackMetal))
        .append(&rubber_band(r, half_w, seg))
        .build()
}

/// The roller's plain rubber band (no groove — carrier rollers run a flat tire).
fn rubber_band(r: f32, half_w: f32, segments: usize) -> GeometryMesh {
    MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![
                ProfilePoint::new(r * 0.70, -half_w),
                ProfilePoint::new(r, -half_w * 0.8),
                ProfilePoint::new(r, half_w * 0.8),
                ProfilePoint::new(r * 0.70, half_w),
            ],
            axis: Axis::X,
            segments,
            material: MaterialRole::Rubber,
            smoothing: SG_WHEEL,
        })
        .build()
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
fn spoke_arm(
    angle: f32,
    inner_r: f32,
    outer_r: f32,
    wheel_r: f32,
    half_width: f32,
) -> GeometryMesh {
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
