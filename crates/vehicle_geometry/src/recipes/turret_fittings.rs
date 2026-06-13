//! Shared turret fittings: cupolas, turret rings, mantlet sockets, and cast domes.

use glam::Vec3;

use super::{SG_CAST, SG_CUPOLA, SG_MANTLET, SG_RING};
use crate::{Axis, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec};

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

/// A low, rounded Soviet cast turret dome centred on the turret ring. Circular in plan; the
/// four-point profile bulges at the shoulder and tapers to the roof, reading as a cast casting
/// rather than a box.
pub(crate) fn cast_dome_turret(
    center_z: f32,
    base_radius: f32,
    roof_radius: f32,
    base_y: f32,
    roof_y: f32,
    segments: usize,
) -> MeshBuilder {
    let span = roof_y - base_y;
    MeshBuilder::new().capped_revolve_at(
        Vec3::new(0.0, 0.0, center_z),
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(base_radius * 0.86, base_y),
                ProfilePoint::new(base_radius, base_y + span * 0.20),
                ProfilePoint::new(base_radius * 0.80, base_y + span * 0.55),
                ProfilePoint::new(roof_radius, roof_y),
            ],
            axis: Axis::Y,
            segments,
            material: MaterialRole::CastArmor,
            smoothing: SG_CAST,
        },
    )
}
