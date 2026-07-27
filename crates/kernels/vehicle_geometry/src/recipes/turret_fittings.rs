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

/// The German cast commander's cupola (late Tiger I / Tiger II / Panther G): a drum a crewman
/// actually fits through (real outer ⌀ ~0.78 m), SEVEN periscope hoods around the crown, and a
/// swing-aside lid with hinge lug and grab handle — replaces the cloned bare drum that was far
/// too small for a human (model-logic audit #3/#12).
pub(crate) fn add_german_cast_cupola(
    builder: MeshBuilder,
    x: f32,
    z: f32,
    base_y: f32,
    radius: f32,
    height: f32,
) -> MeshBuilder {
    let drum_h = height * 0.78;
    let origin = Vec3::new(x, 0.0, z);
    let mut b = builder.capped_revolve_at(
        origin,
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(radius, base_y),
                ProfilePoint::new(radius, base_y + drum_h * 0.72),
                ProfilePoint::new(radius * 0.90, base_y + drum_h),
            ],
            axis: Axis::Y,
            segments: 12,
            material: MaterialRole::CastArmor,
            smoothing: SG_CUPOLA,
        },
    );
    // Seven periscope hoods spaced around the crown — the cast cupola's signature.
    for k in 0..7 {
        let theta = (k as f32 / 7.0) * std::f32::consts::TAU;
        let (sin, cos) = theta.sin_cos();
        b = b.plate_box(
            Vec3::new(x + sin * radius * 0.80, base_y + drum_h * 0.86, z + cos * radius * 0.80),
            Vec3::new(0.05, 0.035, 0.05),
            0.015,
            MaterialRole::CastArmor,
            SG_CUPOLA,
        );
    }
    // Swing-aside lid: disc, hinge lug at the rim, grab handle.
    let lid_y = base_y + drum_h;
    b = b
        .capped_revolve_at(
            origin,
            RevolveSpec {
                profile: vec![
                    ProfilePoint::new(radius * 0.58, lid_y),
                    ProfilePoint::new(radius * 0.58, lid_y + height * 0.12),
                ],
                axis: Axis::Y,
                segments: 12,
                material: MaterialRole::CastArmor,
                smoothing: SG_CUPOLA,
            },
        )
        .plate_box(
            Vec3::new(x + radius * 0.58, lid_y + height * 0.06, z),
            Vec3::new(0.05, 0.02, 0.035),
            0.008,
            MaterialRole::BarrelSteel,
            SG_CUPOLA,
        )
        .plate_box(
            Vec3::new(x - radius * 0.30, lid_y + height * 0.16, z),
            Vec3::new(0.05, 0.014, 0.016),
            0.006,
            MaterialRole::BarrelSteel,
            SG_CUPOLA,
        );
    b
}

/// The T-34-85 commander's cupola: a ⌀0.6 m drum with a ring of VISION SLITS around the upper
/// band and a SPLIT two-piece lid (centre seam, hinges both sides) — the MK-4-era Soviet read,
/// distinct from the German periscope crown.
pub(crate) fn add_soviet_slit_cupola(
    builder: MeshBuilder,
    x: f32,
    z: f32,
    base_y: f32,
    radius: f32,
) -> MeshBuilder {
    let drum_h = 0.17;
    let origin = Vec3::new(x, 0.0, z);
    // Drum and split-lid cap in ONE revolve — the seam bar and hinge lugs carry the "split" read.
    let mut b = builder.capped_revolve_at(
        origin,
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(radius * 0.94, base_y),
                ProfilePoint::new(radius, base_y + drum_h * 0.55),
                ProfilePoint::new(radius * 0.92, base_y + drum_h),
                ProfilePoint::new(radius * 0.60, base_y + drum_h + 0.035),
            ],
            axis: Axis::Y,
            segments: 12,
            material: MaterialRole::CastArmor,
            smoothing: SG_CUPOLA,
        },
    );
    // Five dark vision slits around the forward arc of the upper band — single proud quads on
    // the drum surface (a slit is a marking, not a solid; 2 tris each keeps the budget honest).
    let slit_y = base_y + drum_h * 0.62;
    for k in 0..5 {
        let theta = (k as f32 - 2.0) * 0.9;
        let (sin, cos) = theta.sin_cos();
        let c = Vec3::new(x + sin * radius * 1.01, slit_y, z + cos * radius * 1.01);
        let t = Vec3::new(cos, 0.0, -sin) * 0.045;
        let up = Vec3::new(0.0, 0.014, 0.0);
        b.push_quad(
            [c - t - up, c + t - up, c + t + up, c - t + up],
            MaterialRole::TrackMetal,
            SG_CUPOLA,
        );
    }
    // Centre seam bar and a hinge lug either side sell the two-piece lid.
    let lid_y = base_y + drum_h;
    b = b.plate_box(
        Vec3::new(x, lid_y + 0.030, z),
        Vec3::new(0.014, 0.010, radius * 0.80),
        0.006,
        MaterialRole::BarrelSteel,
        SG_CUPOLA,
    );
    for sign in [-1.0_f32, 1.0] {
        b = b.plate_box(
            Vec3::new(x + sign * radius * 0.88, lid_y + 0.012, z),
            Vec3::new(0.035, 0.016, 0.03),
            0.008,
            MaterialRole::BarrelSteel,
            SG_CUPOLA,
        );
    }
    b
}

/// A flush round roof hatch: a low seating ring with a slightly domed lid, hinge lug, and grab
/// handle. The IS-3's dome roof carries TWO of these instead of a raised cupola (the real IS-3
/// has none) — also the family loader's hatch.
pub(crate) fn add_flush_ring_hatch(
    builder: MeshBuilder,
    x: f32,
    z: f32,
    y: f32,
    radius: f32,
    hinge_sign_x: f32,
) -> MeshBuilder {
    let origin = Vec3::new(x, 0.0, z);
    builder
        .capped_revolve_at(
            origin,
            RevolveSpec {
                profile: vec![
                    ProfilePoint::new(radius, y),
                    ProfilePoint::new(radius, y + 0.030),
                    ProfilePoint::new(radius * 0.55, y + 0.055),
                ],
                axis: Axis::Y,
                segments: 12,
                material: MaterialRole::CastArmor,
                smoothing: SG_CUPOLA,
            },
        )
        .plate_box(
            Vec3::new(x + hinge_sign_x * radius * 0.95, y + 0.022, z),
            Vec3::new(0.04, 0.016, 0.03),
            0.008,
            MaterialRole::BarrelSteel,
            SG_CUPOLA,
        )
        .plate_box(
            Vec3::new(x - hinge_sign_x * radius * 0.45, y + 0.055, z),
            Vec3::new(0.04, 0.012, 0.015),
            0.006,
            MaterialRole::BarrelSteel,
            SG_CUPOLA,
        )
}

/// A commander's periscope head (IS-3 TPK style): a small pedestal with an overhanging visor.
pub(crate) fn add_commander_periscope(builder: MeshBuilder, x: f32, z: f32, y: f32) -> MeshBuilder {
    builder
        .plate_box(
            Vec3::new(x, y + 0.045, z),
            Vec3::new(0.055, 0.045, 0.055),
            0.015,
            MaterialRole::CastArmor,
            SG_CUPOLA,
        )
        .plate_box(
            Vec3::new(x, y + 0.10, z + 0.02),
            Vec3::new(0.07, 0.022, 0.075),
            0.010,
            MaterialRole::BarrelSteel,
            SG_CUPOLA,
        )
}

/// The British commander's cupola (Centurion Mk 3): a wide drum (real outer ⌀ ~0.75 m) with a
/// pair of forward sight hoods and a TWO-PIECE lid split fore/aft — its own nation's read.
pub(crate) fn add_british_cupola(
    builder: MeshBuilder,
    x: f32,
    z: f32,
    base_y: f32,
    radius: f32,
) -> MeshBuilder {
    let drum_h = 0.19;
    let origin = Vec3::new(x, 0.0, z);
    let mut b = builder.capped_revolve_at(
        origin,
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(radius * 0.90, base_y),
                ProfilePoint::new(radius, base_y + drum_h * 0.65),
                ProfilePoint::new(radius * 0.96, base_y + drum_h),
            ],
            axis: Axis::Y,
            segments: 12,
            material: MaterialRole::CastArmor,
            smoothing: SG_CUPOLA,
        },
    );
    // Two forward sight hoods flanking the bore line of the commander's view.
    for sign in [-1.0_f32, 1.0] {
        b = b.plate_box(
            Vec3::new(x + sign * radius * 0.45, base_y + drum_h + 0.015, z + radius * 0.62),
            Vec3::new(0.06, 0.028, 0.06),
            0.012,
            MaterialRole::CastArmor,
            SG_CUPOLA,
        );
    }
    // Fore/aft split lid: shallow cap with a transverse seam bar and rear hinge lugs.
    let lid_y = base_y + drum_h;
    b = b
        .capped_revolve_at(
            origin,
            RevolveSpec {
                profile: vec![
                    ProfilePoint::new(radius * 0.84, lid_y),
                    ProfilePoint::new(radius * 0.52, lid_y + 0.030),
                ],
                axis: Axis::Y,
                segments: 12,
                material: MaterialRole::CastArmor,
                smoothing: SG_CUPOLA,
            },
        )
        .plate_box(
            Vec3::new(x, lid_y + 0.026, z),
            Vec3::new(radius * 0.76, 0.010, 0.014),
            0.006,
            MaterialRole::BarrelSteel,
            SG_CUPOLA,
        )
        .plate_box(
            Vec3::new(x, lid_y + 0.012, z - radius * 0.86),
            Vec3::new(0.05, 0.016, 0.03),
            0.008,
            MaterialRole::BarrelSteel,
            SG_CUPOLA,
        );
    b
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
    add_oval_mantlet_socket(builder, axis_y, mantlet, 2.15, 0.82, segments)
}

/// The KV-1's ZiS-5 mask: the OPPOSITE proportion to the T-54's wide flat oval — a mask taller
/// than it is wide, covering the vertical aperture the gun elevates through. Authored so the KV
/// stops wearing the generic collar every other gun in the fleet wears (audit #6).
pub(crate) fn add_kv1_mantlet_socket(
    builder: MeshBuilder,
    axis_y: f32,
    mantlet: Option<(f32, f32, f32)>,
    segments: usize,
) -> MeshBuilder {
    add_oval_mantlet_socket(builder, axis_y, mantlet, 1.10, 1.40, segments)
}

/// A broad OVAL socket band on the turret face — shared construction for the wide mantlet
/// masks (the T-54's cast mask, the Tiger II's Turmblende band); each vehicle passes its own
/// width/height scales, so the family rhymes in build but not in shape (audit #6).
pub(crate) fn add_oval_mantlet_socket(
    builder: MeshBuilder,
    axis_y: f32,
    mantlet: Option<(f32, f32, f32)>,
    x_scale: f32,
    y_scale: f32,
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
        x_scale,
        y_scale,
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
