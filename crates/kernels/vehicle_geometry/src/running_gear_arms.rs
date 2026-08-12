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

/// Arm plate half-thickness along the axle.
const ARM_HALF_X: f32 = 0.045;

/// One trailing swing arm, authored with the HULL PIVOT at the origin and the axle tip at
/// `(0, -ARM_RISE_M, -ARM_REACH_M)`: a tapered forged arm, a pivot boss, and an axle stub that
/// reaches outboard into the road wheel's hub. This is the RIGHT-hand arm; the left is
/// [`swing_arm_unit_mesh_left`].
pub fn swing_arm_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    match kin.suspension {
        game_core::SuspensionKind::TorsionBar => torsion_arm_unit_mesh(kin),
        game_core::SuspensionKind::Christie => christie_crank_unit_mesh(kin),
        game_core::SuspensionKind::Horstmann => horstmann_bogie_unit_mesh(kin),
    }
}

/// The LEFT-hand arm: the right one mirrored across the wheel plane.
///
/// An arm cannot be turned to face the other side of the tank. It is not a solid of revolution
/// (a half-turn about Y fixed the wheels) and it is not X-symmetric (its torsion boss stands
/// proud INBOARD, toward the hull the bar crosses); a half-turn would also swap trailing for
/// leading. So the left side gets mirrored GEOMETRY — x negated on positions and normals, each
/// triangle's winding re-reversed so the mesh stays outward-facing under back-face culling —
/// which handles every suspension family this dispatch will ever grow, not just the one whose
/// asymmetry was noticed.
pub fn swing_arm_unit_mesh_left(kin: &RunningGearKinematics) -> GeometryMesh {
    mirror_x(&swing_arm_unit_mesh(kin))
}

/// Mirror a mesh across the YZ plane, keeping it a valid outward-facing mesh: a mirror has a
/// negative determinant, so the triangle winding is reversed to compensate.
fn mirror_x(mesh: &GeometryMesh) -> GeometryMesh {
    let vertices = mesh
        .vertices()
        .iter()
        .map(|v| {
            let mut m = *v;
            m.position.x = -m.position.x;
            m.normal.x = -m.normal.x;
            m
        })
        .collect();
    let indices = mesh.indices().chunks_exact(3).flat_map(|t| [t[0], t[2], t[1]]).collect();
    GeometryMesh::new(vertices, indices)
}

/// One trailing torsion arm.
///
/// It used to be a flat plate of constant thickness, sized by two constants that every
/// torsion-bar tank in the fleet shared — 0.26 m of reach and 0.13 m of rise, from a 36-tonne
/// T-54 to a 70-tonne Tiger II. A suspension is one of the few things a tank is judged on, and
/// its geometry now comes from the blueprint like everything else the vehicle is.
///
/// The shape is a forging, not a slab: an I-SECTION, thick along the load path and waisted
/// between its flanges, with the torsion-bar hub at the pivot end — the splined boss the bar
/// actually twists inside, which is the part that makes the mechanism legible at all.
fn torsion_arm_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments_for(12);
    let tip = Vec2::new(-kin.arm_rise, -kin.arm_reach);
    let along = tip.normalize_or_zero();
    let across = Vec2::new(-along.y, along.x);
    // The web: the thin middle of the I, carrying the arm from boss to axle.
    let web = vec![-across * 0.030, across * 0.030, tip + across * 0.024, tip - across * 0.024];
    // The flanges: the thick edges the section's stiffness actually lives in. They are what
    // makes it a forging rather than a plate — and they are visible, because a swing arm is
    // seen edge-on from every angle a tank is looked at from the side.
    let flange = |offset: f32| {
        vec![
            -across * 0.055 + along * offset,
            across * 0.055 + along * offset,
            across * 0.048 + along * (offset + 0.045),
            -across * 0.048 + along * (offset + 0.045),
        ]
    };
    let extrude = |section: Vec<Vec2>, half_depth: f32| {
        MeshBuilder::new()
            .extrude(
                Vec3::ZERO,
                ExtrudeSpec {
                    section,
                    axis: Axis::X,
                    half_depth,
                    material: MaterialRole::TrackMetal,
                    smoothing: SG_HARD,
                },
            )
            .build()
    };
    let mut builder = MeshBuilder::new().append(&extrude(web, ARM_HALF_X * 0.55));
    // The flanges are what makes the section an I rather than a plate — and at the switch range
    // a 20 mm step on an arm in the hull's shadow is not something anyone resolves.
    if kin.detail == crate::GearDetail::Near {
        builder = builder
            .append(&extrude(flange(0.0), ARM_HALF_X))
            .append(&extrude(flange(kin.arm_reach * 0.62), ARM_HALF_X));
    }
    builder
        // The torsion-bar hub: the boss at the pivot the bar is splined into. It runs the whole
        // width of the pivot and stands proud INBOARD toward the hull, where the bar actually
        // crosses the floor — one boss, not a bar hub overlapping a second plain one.
        .append(&stub(Vec3::new(-ARM_HALF_X * 0.9, 0.0, 0.0), 0.086, ARM_HALF_X * 2.4, seg))
        // The axle stub, reaching outboard into the wheel hub.
        .append(&stub(Vec3::new(0.0, tip.x, tip.y), 0.055, ARM_HALF_X * 2.4, seg))
        .build()
}

/// T-34 Christie cue: a compact bell-crank rising steeply into the hull side. The long coil spring
/// itself is internal, so drawing the fleet's generic trailing torsion arm here would advertise the
/// wrong mechanism.
fn christie_crank_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments_for(12);
    let tip = Vec2::new(-kin.arm_rise, -kin.arm_reach);
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
        .append(&stub(Vec3::new(0.0, 0.055, 0.0), 0.075, ARM_HALF_X * 2.0, kin.segments_for(12)))
        .build()
}

/// The lever shock absorber at a damped station: the hydraulic body on its shaft at the origin,
/// the forged lever falling to the link pin near the axle line. Authored like the arm — hull
/// anchor at the origin — and CHIRAL like it (the body stands proud inboard toward its hull
/// bracket), so the left flank instances [`damper_unit_mesh_left`].
///
/// This is the part `t54_details::suspension_furniture` explicitly declined to fake statically:
/// a damper spans hull to MOVING axle, so it belongs here, riding the same live travel the arm
/// swings on.
pub fn damper_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments_for(12);
    // The lever toe: down to the axle line, trailing over the wheel's rim.
    let tip = Vec2::new(-(kin.arm_rise + 0.11), -0.16);
    let along = tip.normalize_or_zero();
    let across = Vec2::new(-along.y, along.x);
    let lever = vec![-across * 0.026, across * 0.026, tip + across * 0.018, tip - across * 0.018];
    let mut builder = MeshBuilder::new().append(
        &MeshBuilder::new()
            .extrude(
                Vec3::ZERO,
                ExtrudeSpec {
                    section: lever,
                    axis: Axis::X,
                    half_depth: ARM_HALF_X * 0.5,
                    material: MaterialRole::TrackMetal,
                    smoothing: SG_HARD,
                },
            )
            .build(),
    );
    // The hydraulic body: a squat drum on the shaft, proud INBOARD toward the hull bracket.
    builder =
        builder.append(&stub(Vec3::new(-ARM_HALF_X * 0.8, 0.0, 0.0), 0.074, ARM_HALF_X * 1.7, seg));
    if kin.detail == crate::GearDetail::Near {
        // The link pin at the lever's toe — the joint that makes "this connects" legible.
        builder = builder.append(&stub(Vec3::new(0.0, tip.x, tip.y), 0.032, ARM_HALF_X * 1.3, seg));
    }
    builder.build()
}

/// The left-hand damper: mirrored geometry, winding re-reversed (see [`swing_arm_unit_mesh_left`]).
pub fn damper_unit_mesh_left(kin: &RunningGearKinematics) -> GeometryMesh {
    mirror_x(&damper_unit_mesh(kin))
}

/// Damper placements for one side: anchored above-and-ahead of each damped station's axle, the
/// lever pivoting with that wheel's live travel at half amplitude (a damper lever's stroke is
/// shorter than the arm's swing).
pub(crate) fn damper_transforms(
    kin: &RunningGearKinematics,
    side_sign: f32,
    travel: &[f32],
) -> Vec<Mat4> {
    let travel_at = |index: usize| travel.get(index).copied().unwrap_or(0.0).clamp(-0.08, 0.20);
    kin.damper_stations
        .iter()
        .filter_map(|&station| kin.wheel_zs.get(station).map(|&z| (station, z)))
        .map(|(station, z)| {
            let x = side_sign * (kin.wheel_x - kin.wheel_half_width - ARM_HALF_X);
            let anchor = Vec3::new(x, kin.cy + kin.arm_rise + 0.13, z + kin.arm_reach + 0.15);
            let swing = (travel_at(station) * 0.5 / kin.arm_reach).clamp(-1.0, 1.0).asin();
            Mat4::from_translation(anchor) * Mat4::from_rotation_x(swing)
        })
        .collect()
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
    let pivot = Vec3::new(arm_x, kin.cy + kin.arm_rise, wheel_z + kin.arm_reach);
    let swing = (travel / kin.arm_reach).clamp(-1.0, 1.0).asin();
    Mat4::from_translation(pivot) * Mat4::from_rotation_x(swing)
}

fn christie_crank_transform(
    kin: &RunningGearKinematics,
    side_sign: f32,
    wheel_z: f32,
    travel: f32,
) -> Mat4 {
    let arm_x = side_sign * (kin.wheel_x - kin.wheel_half_width - ARM_HALF_X);
    let pivot = Vec3::new(arm_x, kin.cy + kin.arm_rise, wheel_z + kin.arm_reach);
    let swing = (travel / kin.arm_reach).clamp(-1.0, 1.0).asin();
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
