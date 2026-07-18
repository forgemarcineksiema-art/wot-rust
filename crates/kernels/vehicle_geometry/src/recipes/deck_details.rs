//! Per-vehicle deck fittings — the de-clone of the old `blueprint_deck_details`, which
//! stamped ONE identical hatch/headlight/tow-hook/engine-panel set onto every vehicle
//! (model-logic audit #1/#2, photo comparison F4). What is shared here is CONSTRUCTION
//! (how a hinged hatch or a guarded headlight is built); every vehicle owns its LAYOUT —
//! hatch count and positions, light count and positions, and the engine deck of its family
//! — sourced from the reference photos (docs/vehicle-photo-comparison-2026-07.md).
//!
//! Hatches carry a hinge bar and a grab handle (audit #12): a plate that shows HOW it
//! opens reads as a door, not a sticker.

use game_core::VehicleBlueprint;
use glam::{Vec2, Vec3};

use super::SG_HARD;
use crate::{ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec};

/// The engine bay's forward edge: never ahead of the turret seat (plan plus clearance) — the
/// turret's base skirt drops below deck level, so a grille frame running under it would clash.
fn engine_bay_front(bp: &VehicleBlueprint) -> f32 {
    (-1.15_f32).min(bp.turret.ring_z - bp.turret.plan_half_length - 0.15)
}

/// Z of the deck's forward edge (where the glacis plane cuts the roof).
fn deck_front_edge(bp: &VehicleBlueprint) -> f32 {
    let hull = &bp.hull;
    let run = (hull.deck_y - hull.sponson_y).max(0.0) * hull.glacis_slope_deg.to_radians().tan();
    hull.half_len - run
}

/// Whether a fitting of z-half-extent `half` centred at `z` would intrude on the turret's
/// seat (plan plus clearance) — kept from the old module: a hatch corner grazing the turret
/// seat reads as the turret sunk into the deck.
fn turret_covers(bp: &VehicleBlueprint, z: f32, half: f32) -> bool {
    (z - bp.turret.ring_z).abs() < bp.turret.plan_half_length + half + 0.25
}

// --- Shared construction, per-vehicle layout. ---

/// A rectangular roof hatch: proud plate, hinge bar along the `hinge_sign_z` edge, and a
/// small grab handle on the opposite edge.
fn rect_hatch(
    builder: MeshBuilder,
    c: Vec3,
    half_x: f32,
    half_z: f32,
    hinge_sign_z: f32,
) -> MeshBuilder {
    builder
        .plate_box(c, Vec3::new(half_x, 0.022, half_z), 0.045, MaterialRole::RolledArmor, SG_HARD)
        .plate_box(
            Vec3::new(c.x, c.y + 0.012, c.z + hinge_sign_z * half_z),
            Vec3::new(half_x * 0.8, 0.014, 0.02),
            0.008,
            MaterialRole::BarrelSteel,
            SG_HARD,
        )
        .plate_box(
            Vec3::new(c.x, c.y + 0.03, c.z - hinge_sign_z * (half_z * 0.6)),
            Vec3::new(0.05, 0.012, 0.015),
            0.006,
            MaterialRole::BarrelSteel,
            SG_HARD,
        )
}

/// A round roof hatch: proud disc, hinge lug on the `hinge_x` side, handle opposite.
fn round_hatch(builder: MeshBuilder, c: Vec3, radius: f32, hinge_sign_x: f32) -> MeshBuilder {
    builder
        .capped_revolve_at(
            c,
            RevolveSpec {
                profile: vec![ProfilePoint::new(radius, 0.0), ProfilePoint::new(radius, 0.045)],
                axis: crate::Axis::Y,
                segments: 14,
                material: MaterialRole::RolledArmor,
                smoothing: SG_HARD,
            },
        )
        .plate_box(
            Vec3::new(c.x + hinge_sign_x * radius, c.y + 0.02, c.z),
            Vec3::new(0.04, 0.016, 0.03),
            0.008,
            MaterialRole::BarrelSteel,
            SG_HARD,
        )
        .plate_box(
            Vec3::new(c.x - hinge_sign_x * (radius * 0.55), c.y + 0.055, c.z),
            Vec3::new(0.045, 0.012, 0.014),
            0.006,
            MaterialRole::BarrelSteel,
            SG_HARD,
        )
}

/// A headlight drum, optionally with the Soviet brush-guard hoop over it.
fn headlight(builder: MeshBuilder, c: Vec3, radius: f32, guarded: bool) -> MeshBuilder {
    let mut b = builder.capped_revolve_at(
        c,
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(radius, -radius * 0.9),
                ProfilePoint::new(radius, radius * 0.9),
            ],
            axis: crate::Axis::Z,
            segments: 12,
            material: MaterialRole::BarrelSteel,
            smoothing: SG_HARD,
        },
    );
    if guarded {
        for dx in [-radius * 1.15, radius * 1.15] {
            b = b.plate_box(
                Vec3::new(c.x + dx, c.y, c.z),
                Vec3::new(0.012, radius * 1.2, 0.012),
                0.006,
                MaterialRole::BarrelSteel,
                SG_HARD,
            );
        }
        b = b.plate_box(
            Vec3::new(c.x, c.y + radius * 1.25, c.z),
            Vec3::new(radius * 1.25, 0.012, 0.012),
            0.006,
            MaterialRole::BarrelSteel,
            SG_HARD,
        );
    }
    b
}

fn tow_hooks(mut builder: MeshBuilder, bp: &VehicleBlueprint, stations: &[f32]) -> MeshBuilder {
    let hull = &bp.hull;
    for &z in stations {
        for sign in [-1.0_f32, 1.0] {
            builder = builder.plate_box(
                Vec3::new(sign * hull.lower_half_width * 0.62, hull.sponson_y + 0.10, z),
                Vec3::new(0.10, 0.08, 0.12),
                0.04,
                MaterialRole::RolledArmor,
                SG_HARD,
            );
        }
    }
    builder
}

/// Hinged bow fender flaps over the drive sprockets (photo comparison F3): a slanted plate
/// dropping forward from the sponson lip over each track's front wrap — the German line's
/// bow signature. Built as an extruded parallelogram so the droop is real geometry.
fn german_bow_flaps(mut builder: MeshBuilder, bp: &VehicleBlueprint) -> MeshBuilder {
    let track = &bp.track;
    let band_half = ((track.outer_x - track.inner_x) * 0.5).max(0.05);
    let hinge_y = bp.hull.sponson_y + 0.02;
    let hinge_z = track.end_z - 0.10;
    let droop_dz = 0.55;
    let droop_dy = 0.22;
    for sign in [-1.0_f32, 1.0] {
        builder = builder
            .extrude(
                Vec3::new(sign * track.center_x, hinge_y, hinge_z),
                ExtrudeSpec {
                    section: vec![
                        Vec2::new(0.0, -0.018),
                        Vec2::new(droop_dz, -droop_dy - 0.018),
                        Vec2::new(droop_dz, -droop_dy),
                        Vec2::new(0.0, 0.0),
                    ],
                    axis: crate::Axis::X,
                    half_depth: band_half * 0.96,
                    material: MaterialRole::RolledArmor,
                    smoothing: SG_HARD,
                },
            )
            // The hinge bar along the flap's root.
            .plate_box(
                Vec3::new(sign * track.center_x, hinge_y + 0.014, hinge_z),
                Vec3::new(band_half * 0.80, 0.014, 0.024),
                0.008,
                MaterialRole::BarrelSteel,
                SG_HARD,
            );
    }
    builder
}

/// Twin vertical exhaust stacks on the rear plate — the German family's tail signature
/// (audit #5: smoke needs somewhere to leave). The Tiger I keeps its bespoke shielded pair;
/// this shared construction serves the rest of the four.
fn german_exhaust_stacks(mut builder: MeshBuilder, bp: &VehicleBlueprint) -> MeshBuilder {
    let hull = &bp.hull;
    for x in [-hull.lower_half_width * 0.55, hull.lower_half_width * 0.55] {
        builder = builder
            .capped_revolve_at(
                Vec3::new(x, 0.0, -hull.half_len + 0.10),
                RevolveSpec {
                    profile: vec![
                        ProfilePoint::new(0.09, hull.deck_y - 0.85),
                        ProfilePoint::new(0.09, hull.deck_y + 0.12),
                    ],
                    axis: crate::Axis::Y,
                    segments: 10,
                    material: MaterialRole::RolledArmor,
                    smoothing: SG_HARD,
                },
            )
            // Dark open mouth — the pipe is a pipe, not a capped rod.
            .capped_revolve_at(
                Vec3::new(x, 0.0, -hull.half_len + 0.10),
                RevolveSpec {
                    profile: vec![
                        ProfilePoint::new(0.055, hull.deck_y + 0.121),
                        ProfilePoint::new(0.055, hull.deck_y + 0.125),
                    ],
                    axis: crate::Axis::Y,
                    segments: 10,
                    material: MaterialRole::TrackMetal,
                    smoothing: SG_HARD,
                },
            );
    }
    builder
}

/// The V-2 diesel's twin round exhaust ports low on the rear plate (T-34's read): short
/// horizontal pipes with dark open mouths.
fn soviet_exhaust_ports(mut builder: MeshBuilder, bp: &VehicleBlueprint, y: f32) -> MeshBuilder {
    let hull = &bp.hull;
    // Seat the ports ON the sloped rear plate (fold at the sponson step, like the hull ring),
    // so they poke from the armour instead of hanging in the air behind it.
    let rear = hull.rear_slope_deg.to_radians().tan();
    let plate_z = -hull.half_len + (y - hull.sponson_y).abs() * rear;
    for x in [-0.36_f32, 0.36] {
        let c = Vec3::new(x, y, plate_z + 0.02);
        builder = builder
            .capped_revolve_at(
                c,
                RevolveSpec {
                    profile: vec![ProfilePoint::new(0.085, -0.05), ProfilePoint::new(0.085, 0.06)],
                    axis: crate::Axis::Z,
                    segments: 10,
                    material: MaterialRole::RolledArmor,
                    smoothing: SG_HARD,
                },
            )
            .capped_revolve_at(
                c,
                RevolveSpec {
                    profile: vec![
                        ProfilePoint::new(0.055, -0.055),
                        ProfilePoint::new(0.055, -0.051),
                    ],
                    axis: crate::Axis::Z,
                    segments: 10,
                    material: MaterialRole::TrackMetal,
                    smoothing: SG_HARD,
                },
            );
    }
    builder
}

/// Low armoured exhaust cowls on the rear deck flanks — the Meteor-engined Centurion's read.
fn british_exhaust_cowls(mut builder: MeshBuilder, bp: &VehicleBlueprint) -> MeshBuilder {
    let hull = &bp.hull;
    let z = -hull.half_len + 0.55;
    for sign in [-1.0_f32, 1.0] {
        let x = sign * hull.lower_half_width * 0.62;
        builder = builder
            .plate_box(
                Vec3::new(x, hull.deck_y + 0.055, z),
                Vec3::new(0.14, 0.05, 0.30),
                0.03,
                MaterialRole::RolledArmor,
                SG_HARD,
            )
            // Dark outlet slot facing rearward under the cowl lip.
            .plate_box(
                Vec3::new(x, hull.deck_y + 0.035, z - 0.30),
                Vec3::new(0.10, 0.028, 0.012),
                0.008,
                MaterialRole::TrackMetal,
                SG_HARD,
            );
    }
    builder
}

/// German engine deck: central grille frame flanked by the family's TWO round cooling-fan
/// armours — the Tiger/Panther rear-deck signature (photo comparison), replacing the cloned
/// pair of raised rectangles every nation used to wear.
fn engine_deck_german(mut builder: MeshBuilder, bp: &VehicleBlueprint) -> GeometryMesh {
    let hull = &bp.hull;
    let deck_back = -hull.half_len + 0.35;
    let front = engine_bay_front(bp);
    let center_z = (front + deck_back) * 0.5;
    builder = builder.plate_box(
        Vec3::new(0.0, hull.deck_y + 0.02, center_z),
        Vec3::new(hull.lower_half_width * 0.5, 0.02, (front - deck_back).abs() * 0.5),
        0.03,
        MaterialRole::RolledArmor,
        SG_HARD,
    );
    for x in [-hull.lower_half_width * 0.52, hull.lower_half_width * 0.52] {
        // Dark intake-mesh bed under the fan armour so the grille reads against the deck.
        builder = builder.plate_box(
            Vec3::new(x, hull.deck_y + 0.042, center_z),
            Vec3::new(hull.lower_half_width * 0.36, 0.008, hull.lower_half_width * 0.36),
            0.015,
            MaterialRole::TrackMetal,
            SG_HARD,
        );
        builder = builder.capped_revolve_at(
            Vec3::new(x, hull.deck_y + 0.05, center_z),
            RevolveSpec {
                profile: vec![
                    ProfilePoint::new(hull.lower_half_width * 0.30, 0.0),
                    ProfilePoint::new(hull.lower_half_width * 0.30, 0.035),
                ],
                axis: crate::Axis::Y,
                segments: 16,
                material: MaterialRole::RolledArmor,
                smoothing: SG_HARD,
            },
        );
        // Open fan slats: the armour ring guards a dark circular grating, not solid plate.
        builder = builder.capped_revolve_at(
            Vec3::new(x, hull.deck_y + 0.078, center_z),
            RevolveSpec {
                profile: vec![
                    ProfilePoint::new(hull.lower_half_width * 0.22, 0.0),
                    ProfilePoint::new(hull.lower_half_width * 0.22, 0.012),
                ],
                axis: crate::Axis::Y,
                segments: 16,
                material: MaterialRole::TrackMetal,
                smoothing: SG_HARD,
            },
        );
    }
    builder.build()
}

/// Soviet engine deck: transverse louvre strips over the engine bay — the T-34/IS read.
fn engine_deck_soviet(mut builder: MeshBuilder, bp: &VehicleBlueprint) -> GeometryMesh {
    let hull = &bp.hull;
    let deck_back = -hull.half_len + 0.35;
    let front = engine_bay_front(bp);
    let strips = 4;
    for i in 0..strips {
        let t = (i as f32 + 0.5) / strips as f32;
        let z = front + (deck_back - front) * t;
        builder = builder.plate_box(
            Vec3::new(0.0, hull.deck_y + 0.038, z),
            Vec3::new(hull.lower_half_width * 0.72, 0.024, 0.09),
            0.02,
            MaterialRole::RolledArmor,
            SG_HARD,
        );
    }
    builder.build()
}

/// British engine deck: one flat raised panel with a broad mesh-grille frame.
fn engine_deck_british(builder: MeshBuilder, bp: &VehicleBlueprint) -> GeometryMesh {
    let hull = &bp.hull;
    let deck_back = -hull.half_len + 0.35;
    let front = engine_bay_front(bp);
    let center_z = (front + deck_back) * 0.5;
    builder
        .plate_box(
            Vec3::new(0.0, hull.deck_y + 0.02, center_z),
            Vec3::new(hull.lower_half_width * 0.8, 0.02, (front - deck_back).abs() * 0.5),
            0.04,
            MaterialRole::RolledArmor,
            SG_HARD,
        )
        .plate_box(
            Vec3::new(0.0, hull.deck_y + 0.045, center_z + 0.15),
            Vec3::new(hull.lower_half_width * 0.5, 0.012, 0.35),
            0.02,
            MaterialRole::TrackMetal,
            SG_HARD,
        )
        .build()
}

// --- Per-vehicle layouts (positions from the reference photos). ---

/// German bow roof: driver (left) and radio operator (right) hatches side by side, one
/// central Bosch headlight at the glacis top edge.
fn german_bow(mut builder: MeshBuilder, bp: &VehicleBlueprint, hatch_r: f32) -> MeshBuilder {
    let edge = deck_front_edge(bp);
    let hatch_z = edge - 0.42;
    if !turret_covers(bp, hatch_z, hatch_r) {
        for sign in [-1.0_f32, 1.0] {
            builder = round_hatch(
                builder,
                Vec3::new(sign * bp.hull.lower_half_width * 0.5, bp.hull.deck_y + 0.002, hatch_z),
                hatch_r,
                sign,
            );
        }
    }
    headlight(builder, Vec3::new(0.0, bp.hull.deck_y + 0.10, edge - 0.10), 0.085, false)
}

pub(crate) fn tiger_i_deck(bp: &VehicleBlueprint) -> GeometryMesh {
    let b = german_bow(MeshBuilder::new(), bp, 0.24);
    let b = tow_hooks(b, bp, &[bp.hull.half_len - 0.14, -bp.hull.half_len + 0.14]);
    engine_deck_german(b, bp)
}

pub(crate) fn tiger_ii_deck(bp: &VehicleBlueprint) -> GeometryMesh {
    let b = german_bow(MeshBuilder::new(), bp, 0.24);
    let b = tow_hooks(b, bp, &[bp.hull.half_len - 0.14, -bp.hull.half_len + 0.14]);
    let b = german_exhaust_stacks(b, bp);
    let b = german_bow_flaps(b, bp);
    engine_deck_german(b, bp)
}

/// Panther II (dossier PII.1/PII.3, Fort Benning): two round roof hatches like the family,
/// but the BOW furniture is the Panther's own — the MG Kugelblende in the glacis right, the
/// driver's twin periscope hoods at the roof front edge left, ONE Bosch headlight standing
/// ON the glacis left (F4 — not the family's central deck light), and the CURVED front
/// fender sweeps over the sprockets (F3 — three chained slanted segments, not the Tiger II's
/// single drooping flap).
pub(crate) fn panther_ii_deck(bp: &VehicleBlueprint) -> GeometryMesh {
    let hull = &bp.hull;
    let edge = deck_front_edge(bp);
    let glacis = hull.glacis_slope_deg.to_radians();
    // A point on the glacis plane at height `y` (the prism folds at the sponson step),
    // pushed `standoff` along the plate normal.
    let on_glacis = |x: f32, y: f32, standoff: f32| -> Vec3 {
        let run = (y - hull.sponson_y).max(0.0) * glacis.tan();
        Vec3::new(x, y, hull.half_len - run) + Vec3::new(0.0, glacis.cos(), glacis.sin()) * standoff
    };
    let mut b = MeshBuilder::new();
    // Two round roof hatches (driver left, radio operator right) with hinge and handle.
    let hatch_z = edge - 0.42;
    if !turret_covers(bp, hatch_z, 0.23) {
        for sign in [-1.0_f32, 1.0] {
            b = round_hatch(
                b,
                Vec3::new(sign * hull.lower_half_width * 0.5, hull.deck_y + 0.002, hatch_z),
                0.23,
                sign,
            );
        }
    }
    // Driver's twin periscope hoods at the roof front edge, left side.
    for x in [-0.72_f32, -0.46] {
        b = b.plate_box(
            Vec3::new(x, hull.deck_y + 0.035, edge - 0.10),
            Vec3::new(0.055, 0.035, 0.055),
            0.015,
            MaterialRole::RolledArmor,
            SG_HARD,
        );
    }
    // The MG Kugelblende in the glacis right — a cast ball seated in the 55-degree plate.
    let ball = on_glacis(0.60, 1.42, 0.0);
    b = b.capped_revolve_at(
        Vec3::new(ball.x, ball.y, 0.0),
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(0.13, ball.z - 0.05),
                ProfilePoint::new(0.09, ball.z + 0.08),
            ],
            axis: crate::Axis::Z,
            segments: 10,
            material: MaterialRole::CastArmor,
            smoothing: SG_HARD,
        },
    );
    // ONE Bosch headlight standing on the glacis LEFT on a short bracket (F4).
    let light = on_glacis(-0.95, 1.70, 0.10);
    b = b.plate_box(
        on_glacis(-0.95, 1.70, 0.05),
        Vec3::new(0.03, 0.03, 0.05),
        0.01,
        MaterialRole::BarrelSteel,
        SG_HARD,
    );
    b = headlight(b, light, 0.075, false);
    // Curved front fender sweeps over the sprockets: three chained slanted segments per side
    // approximating the Panther's quarter-round mudguard.
    let band_half = ((bp.track.outer_x - bp.track.inner_x) * 0.5).max(0.05);
    let sweep = [
        // (z0, y0, z1, y1) segment chain, hull-local, front positive.
        (
            bp.track.end_z - 0.25,
            hull.sponson_y + 0.06,
            bp.track.end_z + 0.30,
            hull.sponson_y + 0.04,
        ),
        (
            bp.track.end_z + 0.30,
            hull.sponson_y + 0.04,
            bp.track.end_z + 0.58,
            hull.sponson_y - 0.14,
        ),
        (
            bp.track.end_z + 0.58,
            hull.sponson_y - 0.14,
            bp.track.end_z + 0.70,
            hull.sponson_y - 0.40,
        ),
    ];
    for sign in [-1.0_f32, 1.0] {
        for &(z0, y0, z1, y1) in &sweep {
            let mid_z = (z0 + z1) * 0.5;
            b = b.extrude(
                Vec3::new(sign * bp.track.center_x, 0.0, mid_z),
                ExtrudeSpec {
                    section: vec![
                        Vec2::new(z0 - mid_z, y0 - 0.020),
                        Vec2::new(z1 - mid_z, y1 - 0.020),
                        Vec2::new(z1 - mid_z, y1),
                        Vec2::new(z0 - mid_z, y0),
                    ],
                    axis: crate::Axis::X,
                    half_depth: band_half * 0.96,
                    material: MaterialRole::RolledArmor,
                    smoothing: SG_HARD,
                },
            );
        }
    }
    let b = tow_hooks(b, bp, &[bp.hull.half_len - 0.14, -bp.hull.half_len + 0.14]);
    let b = german_exhaust_stacks(b, bp);
    engine_deck_german(b, bp)
}

/// The Jagdtiger's bow hatches sit AHEAD of the casemate; the engine deck behind it.
pub(crate) fn jagdtiger_deck(bp: &VehicleBlueprint) -> GeometryMesh {
    let edge = deck_front_edge(bp);
    let hatch_z = edge - 0.38;
    let mut b = MeshBuilder::new();
    for sign in [-1.0_f32, 1.0] {
        b = round_hatch(
            b,
            Vec3::new(sign * bp.hull.lower_half_width * 0.48, bp.hull.deck_y + 0.002, hatch_z),
            0.23,
            sign,
        );
    }
    b = headlight(b, Vec3::new(0.0, bp.hull.deck_y + 0.10, edge - 0.10), 0.085, false);
    b = tow_hooks(b, bp, &[bp.hull.half_len - 0.14, -bp.hull.half_len + 0.14]);
    b = german_exhaust_stacks(b, bp);
    // Bow MG Kugelblende in the glacis right (dossier JT.2) — the ball's seat sits just
    // ahead of the 50-degree plate, like the Tiger I's, scaled to the Jagdtiger's glacis.
    let hull = &bp.hull;
    let ball_y = 1.45;
    let run = (ball_y - hull.belly_y - hull.nose_rise).max(0.0)
        * hull.glacis_slope_deg.to_radians().tan();
    let plate_z = hull.half_len - run;
    b = b.capped_revolve_at(
        Vec3::new(0.58, ball_y, 0.0),
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(0.13, plate_z - 0.05),
                ProfilePoint::new(0.09, plate_z + 0.08),
            ],
            axis: crate::Axis::Z,
            segments: 10,
            material: MaterialRole::CastArmor,
            smoothing: SG_HARD,
        },
    );
    // Large FLAT bow guards over the front sprockets (photo comparison F3) — the Jagdtiger's
    // read, unlike the Tiger II's drooping hinged flaps.
    let band_half = ((bp.track.outer_x - bp.track.inner_x) * 0.5).max(0.05);
    for sign in [-1.0_f32, 1.0] {
        b = b
            .plate_box(
                Vec3::new(sign * bp.track.center_x, hull.sponson_y + 0.03, bp.track.end_z + 0.18),
                Vec3::new(band_half * 0.96, 0.015, 0.42),
                0.02,
                MaterialRole::RolledArmor,
                SG_HARD,
            )
            // Downturned front lip closing the guard's leading edge.
            .plate_box(
                Vec3::new(sign * bp.track.center_x, hull.sponson_y - 0.03, bp.track.end_z + 0.59),
                Vec3::new(band_half * 0.92, 0.05, 0.015),
                0.012,
                MaterialRole::RolledArmor,
                SG_HARD,
            );
    }
    engine_deck_german(b, bp)
}

/// T-34-85: the driver's hatch is IN THE GLACIS FACE (big plate with twin periscope hoods),
/// the hull MG ball sits right of it — the bow's identity (photo). Roof carries no driver
/// hatch at all.
pub(crate) fn t34_85_deck(bp: &VehicleBlueprint) -> GeometryMesh {
    let hull = &bp.hull;
    let glacis = hull.glacis_slope_deg.to_radians();
    // A point on the glacis plane at height `y`, pushed `standoff` along the plate normal.
    let on_glacis = |x: f32, y: f32, standoff: f32| -> Vec3 {
        let run = (y - hull.sponson_y).max(0.0) * glacis.tan();
        Vec3::new(x, y, hull.half_len - run) + Vec3::new(0.0, glacis.cos(), glacis.sin()) * standoff
    };
    let mut b = MeshBuilder::new();
    // Driver's glacis hatch: proud plate on the slope with two periscope hoods on its top edge.
    let hatch_c = on_glacis(-0.45, hull.sponson_y + 0.55, 0.02);
    b = b.plate_box(
        hatch_c,
        Vec3::new(0.28, 0.30, 0.035),
        0.03,
        MaterialRole::RolledArmor,
        SG_HARD,
    );
    for dx in [-0.14_f32, 0.14] {
        b = b.plate_box(
            hatch_c + Vec3::new(dx, 0.26, 0.05),
            Vec3::new(0.055, 0.05, 0.05),
            0.015,
            MaterialRole::RolledArmor,
            SG_HARD,
        );
    }
    // Hull MG ball right of the hatch.
    b = b.capped_revolve_at(
        on_glacis(0.55, hull.sponson_y + 0.50, 0.0),
        RevolveSpec {
            profile: vec![ProfilePoint::new(0.12, -0.03), ProfilePoint::new(0.09, 0.11)],
            axis: crate::Axis::Z,
            segments: 12,
            material: MaterialRole::CastArmor,
            smoothing: SG_HARD,
        },
    );
    b = headlight(b, on_glacis(-0.75, hull.deck_y - 0.10, 0.10), 0.075, false);
    b = tow_hooks(b, bp, &[bp.hull.half_len - 0.14, -bp.hull.half_len + 0.14]);
    b = soviet_exhaust_ports(b, bp, hull.sponson_y + 0.42);
    engine_deck_soviet(b, bp)
}

/// IS-3: central round driver's hatch just behind the pike point, guarded headlight beside
/// it on the centreline (photo); front tow hooks skipped — the pike carries none.
pub(crate) fn is3_deck(bp: &VehicleBlueprint) -> GeometryMesh {
    // The pike planes slice the deck's forward corners, and the turret plan reaches far
    // forward — the free roof band is NARROW. A small oval-ish hatch (the real IS-3 driver
    // hatch is compact) sits centred just behind the pike apex, with a tight clearance to
    // the ring instead of the generic buffer; the pike honesty lock guards the front edge.
    let hatch_r = 0.19;
    let hatch_z = bp.turret.ring_z + bp.turret.plan_half_length + hatch_r + 0.06;
    let mut b = MeshBuilder::new();
    b = round_hatch(b, Vec3::new(0.0, bp.hull.deck_y + 0.002, hatch_z), hatch_r, 1.0);
    // Headlight deferred: on the real vehicle it stands on the pike near the apex; placing
    // it needs a pike-plane standoff (like the T-34 glacis helper) — absent beats a drum
    // poking through the armor plane. Dossier item with the F3 fender work.
    let _ = headlight;
    b = tow_hooks(b, bp, &[-bp.hull.half_len + 0.14]);
    b = soviet_exhaust_ports(b, bp, bp.hull.deck_y - 0.35);
    engine_deck_soviet(b, bp)
}

/// The legacy soviet-family recipe deck (the hybrid T-54 carries its own bespoke fittings).
pub(crate) fn t54_family_deck(bp: &VehicleBlueprint) -> GeometryMesh {
    let edge = deck_front_edge(bp);
    // The T-54's driver sits on the LEFT (the old shared layout put the hatch on the right).
    let hatch_z = edge - 0.45;
    let mut b = MeshBuilder::new();
    if !turret_covers(bp, hatch_z, 0.30) {
        b = rect_hatch(
            b,
            Vec3::new(-bp.hull.lower_half_width * 0.45, bp.hull.deck_y + 0.002, hatch_z),
            0.26,
            0.30,
            1.0,
        );
    }
    b = headlight(b, Vec3::new(0.55, bp.hull.deck_y + 0.09, edge - 0.12), 0.075, true);
    b = tow_hooks(b, bp, &[bp.hull.half_len - 0.14, -bp.hull.half_len + 0.14]);
    engine_deck_soviet(b, bp)
}

/// Centurion Mk 3: driver's hatch on the roof RIGHT; headlights deliberately ABSENT until
/// the fender boxes that carry them exist (dossier F3) — absent beats cloned-wrong.
pub(crate) fn centurion_deck(bp: &VehicleBlueprint) -> GeometryMesh {
    let edge = deck_front_edge(bp);
    let hatch_z = edge - 0.40;
    let mut b = MeshBuilder::new();
    if !turret_covers(bp, hatch_z, 0.30) {
        b = rect_hatch(
            b,
            Vec3::new(bp.hull.lower_half_width * 0.45, bp.hull.deck_y + 0.002, hatch_z),
            0.26,
            0.28,
            1.0,
        );
    }
    b = tow_hooks(b, bp, &[bp.hull.half_len - 0.14, -bp.hull.half_len + 0.14]);
    b = british_exhaust_cowls(b, bp);
    engine_deck_british(b, bp)
}
