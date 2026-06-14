//! Shared turret fittings: cupolas, turret rings, mantlet sockets, and the cast turret shell.

mod socket;

use glam::{Vec2, Vec3};

use super::{SG_CAST, SG_CUPOLA, SG_MANTLET, SG_RING};
use crate::{Axis, LoftSection, LoftSpec, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec};
use socket::oval_socket_mesh;

/// Append a small cupola (drum or domed) onto a turret roof.
pub(crate) fn add_cupola(
    builder: MeshBuilder,
    x: f32,
    z: f32,
    base_y: f32,
    radius: f32,
    height: f32,
    domed: bool,
) -> MeshBuilder {
    let origin = Vec3::new(x, 0.0, z);
    let profile = if domed {
        vec![
            ProfilePoint::new(radius, base_y),
            ProfilePoint::new(radius, base_y + height * 0.55),
            ProfilePoint::new(radius * 0.5, base_y + height),
        ]
    } else {
        vec![ProfilePoint::new(radius, base_y), ProfilePoint::new(radius, base_y + height)]
    };
    builder.capped_revolve_at(
        origin,
        RevolveSpec {
            profile,
            axis: Axis::Y,
            segments: 12,
            material: MaterialRole::CastArmor,
            smoothing: SG_CUPOLA,
        },
    )
}

/// Append a low visible collar around the turret ring so the rotating submesh reads as seated in
/// the hull deck rather than merely touching it at a mathematical plane.
pub(crate) fn add_turret_ring(
    builder: MeshBuilder,
    center_z: f32,
    base_y: f32,
    radius: f32,
    height: f32,
    segments: usize,
) -> MeshBuilder {
    builder.capped_revolve_at(
        Vec3::new(0.0, 0.0, center_z),
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(radius, base_y - height * 0.35),
                ProfilePoint::new(radius * 1.04, base_y + height * 0.65),
            ],
            axis: Axis::Y,
            segments,
            material: MaterialRole::CastArmor,
            smoothing: SG_RING,
        },
    )
}

/// Append a fixed socket on the turret/casemate face behind the elevating mantlet. The moving
/// mantlet still lives in the gun submesh, but this collar keeps the joint visually anchored.
pub(crate) fn add_mantlet_socket(
    builder: MeshBuilder,
    axis_y: f32,
    mantlet: Option<(f32, f32, f32)>,
    segments: usize,
) -> MeshBuilder {
    add_mantlet_socket_with_profile(builder, axis_y, mantlet, segments, 0.96, 1.08)
}

/// Append a wider fixed socket for broad cast Soviet turret fronts where a small collar reads like
/// a detached ball rather than an integrated mantlet seat.
pub(crate) fn add_broad_mantlet_socket(
    builder: MeshBuilder,
    axis_y: f32,
    mantlet: Option<(f32, f32, f32)>,
    segments: usize,
) -> MeshBuilder {
    add_mantlet_socket_with_profile(builder, axis_y, mantlet, segments, 1.40, 1.18)
}

pub(crate) fn add_t54_mantlet_socket(
    builder: MeshBuilder,
    axis_y: f32,
    mantlet: Option<(f32, f32, f32)>,
    segments: usize,
) -> MeshBuilder {
    let Some((radius, back_z, front_z)) = mantlet else {
        return builder;
    };
    let span = (front_z - back_z).max(0.12);
    let socket_back = back_z - span * 0.25;
    let socket_front = back_z + span * 0.40;
    builder.append(&oval_socket_mesh(
        Vec3::new(0.0, axis_y, 0.0),
        radius,
        socket_back,
        socket_front,
        2.15,
        0.82,
        segments,
    ))
}

fn add_mantlet_socket_with_profile(
    builder: MeshBuilder,
    axis_y: f32,
    mantlet: Option<(f32, f32, f32)>,
    segments: usize,
    back_radius_scale: f32,
    front_radius_scale: f32,
) -> MeshBuilder {
    let Some((radius, back_z, front_z)) = mantlet else {
        return builder;
    };
    let span = (front_z - back_z).max(0.12);
    let socket_back = back_z - span * 0.25;
    let socket_front = back_z + span * 0.55;
    builder.capped_revolve_at(
        Vec3::new(0.0, axis_y, 0.0),
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(radius * back_radius_scale, socket_back),
                ProfilePoint::new(radius * front_radius_scale, socket_front),
            ],
            axis: Axis::Z,
            segments,
            material: MaterialRole::CastArmor,
            smoothing: SG_MANTLET,
        },
    )
}

/// A low, rounded Soviet cast turret shell centred on the turret ring. Unlike a plain revolve dome
/// (circular in plan), this lofts egg-shaped plan rings — fuller and longer toward the front than
/// the rear — up through an inset base, a swelling shoulder, a tapering upper, and a small roof, so
/// the turret reads as an asymmetric casting with real front-cheek mass rather than a turned bowl.
///
/// `half_length` sets the fore/aft reach (the front overhang is a touch longer than the rear
/// bustle); `half_width` the beam at the shoulder; `roof_radius` the small roof ring the cupola and
/// hatches sit on.
pub(crate) fn cast_turret_shell(
    center_z: f32,
    half_width: f32,
    half_length: f32,
    roof_radius: f32,
    base_y: f32,
    roof_y: f32,
    segments: usize,
) -> MeshBuilder {
    let span = roof_y - base_y;
    let front = half_length * 0.96;
    let back = half_length * 0.82;
    let _ = roof_radius;
    // (height, plan scale): a low, wide cast dome with a real flat roof — the casting keeps its beam
    // through broad, near-vertical sides and only rounds over into a flat roof plane near the top,
    // so it reads as a T-54 turret casting rather than a smooth bun. The roof ring stays wide enough
    // that the commander's cupola and loader's hatch seat on a real roof.
    let stations = [
        (base_y, 0.92),
        (base_y + span * 0.22, 1.00),
        (base_y + span * 0.55, 0.97),
        (base_y + span * 0.80, 0.84),
        (roof_y, 0.66),
    ];
    let sections = stations
        .iter()
        .map(|&(y, scale)| {
            LoftSection::new(
                y,
                cast_ring(center_z, half_width * scale, front * scale, back * scale, segments),
            )
        })
        .collect();
    MeshBuilder::new().loft(
        Vec3::ZERO,
        LoftSpec {
            sections,
            axis: Axis::Y,
            material: MaterialRole::CastArmor,
            smoothing: SG_CAST,
            cap_ends: true,
        },
    )
}

/// One egg-shaped plan ring in the `(x, z)` plane: an ellipse of beam `half_width` whose forward
/// half reaches `front` and rear half `back`, so `front > back` biases the casting's mass ahead of
/// the ring. Convex by construction (two half-ellipses sharing the beam line).
fn cast_ring(center_z: f32, half_width: f32, front: f32, back: f32, segments: usize) -> Vec<Vec2> {
    (0..segments)
        .map(|k| {
            let theta = (k as f32 / segments as f32) * std::f32::consts::TAU;
            let (sin, cos) = theta.sin_cos();
            let reach = if cos >= 0.0 { front } else { back };
            Vec2::new(half_width * sin, center_z + reach * cos)
        })
        .collect()
}
