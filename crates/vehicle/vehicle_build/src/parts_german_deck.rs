//! The German engine deck and the slab's hull details as library parts (Forge 2.0 K3, step 4d):
//! the deck plate with its two radiator grilles and fan covers, the twin exhaust stacks — in
//! their three-sided late-E shields on the Tiger I's stern, or open and dark-mouthed off the
//! Tiger II's leaned stern on a top bracket — the spare-link rack on the lower bow plate, the
//! driver's visor or periscope hood, the bow MG ball on the driver's plate, and the hinged
//! fender flaps over the belt wraps. Every dimension is the blueprint's (the recipe's own
//! sections, lifted below the seam); which furniture a vehicle wears is its visual file's
//! (`GermanDeckVisual`), so the German line shares one deck and each vehicle keeps its own read.

use game_core::roundness::round_segments;
use game_core::{GermanDeckVisual, VehicleBlueprint};
use glam::{Vec2, Vec3};
use vehicle_geometry::{
    Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec,
    SmoothingGroup, SubmeshKind,
};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};
use crate::smoothing::SG_HARD;

/// The deck and details for `bp`, or `None` when its visual file declares no welded slab
/// construction (the deck belongs to the slab hull it sits on).
pub fn german_deck_parts_for_blueprint(bp: &VehicleBlueprint) -> Option<Vec<VehiclePart>> {
    let visual = bp.visual_detail()?;
    visual.construction?;
    // A vehicle that authors no `german_deck` wears no German deck — the T-34-85's slab hull
    // wore the Tiger's stacks, shields and flaps for one bake before this line existed.
    let deck = visual.german_deck?;
    let guard_top_y = visual.fender.map(|f| f.center_y + f.half.y);
    Some(german_deck_parts(bp, guard_top_y, &deck))
}

/// Deck plate, grilles, stacks (shielded or open), spare links, visor or periscope hood, MG
/// ball, fender flaps — the furniture `deck` selects.
pub fn german_deck_parts(
    bp: &VehicleBlueprint,
    guard_top_y: Option<f32>,
    deck: &GermanDeckVisual,
) -> Vec<VehiclePart> {
    let hull = &bp.hull;
    let mut parts = Vec::new();
    let part =
        |key: PartKey, material: MaterialRole, lod: PartLod, mesh: GeometryMesh| VehiclePart {
            key,
            submesh: SubmeshKind::Hull,
            material,
            smoothing: SG_HARD,
            shape: PartShape::Mesh(mesh),
            lod,
            generator: GeneratorKind::Solid,
        };

    // --- the engine deck: a raised plate over the bay, two grilles with fan covers on it ------
    let deck_back = -hull.half_len + 0.35;
    let front = (-1.15_f32).min(bp.turret.ring_z - bp.turret.plan_half_length - 0.15);
    let center_z = (front + deck_back) * 0.5;
    parts.push(part(
        PartKey::new("engine_deck_panel"),
        MaterialRole::RolledArmor,
        PartLod::Silhouette,
        MeshBuilder::new()
            .plate_box(
                Vec3::new(0.0, hull.deck_y + 0.02, center_z),
                Vec3::new(hull.lower_half_width * 0.5, 0.02, (front - deck_back).abs() * 0.5),
                0.03,
                MaterialRole::RolledArmor,
                SG_HARD,
            )
            .build(),
    ));
    for (i, x) in
        [-hull.lower_half_width * 0.52, hull.lower_half_width * 0.52].into_iter().enumerate()
    {
        let grille = MeshBuilder::new()
            .plate_box(
                Vec3::new(x, hull.deck_y + 0.042, center_z),
                Vec3::new(hull.lower_half_width * 0.36, 0.008, hull.lower_half_width * 0.36),
                0.015,
                MaterialRole::TrackMetal,
                SG_HARD,
            )
            .capped_revolve_at(
                Vec3::new(x, hull.deck_y + 0.05, center_z),
                RevolveSpec {
                    profile: vec![
                        ProfilePoint::new(hull.lower_half_width * 0.30, 0.0),
                        ProfilePoint::new(hull.lower_half_width * 0.30, 0.035),
                    ],
                    axis: Axis::Y,
                    segments: round_segments(hull.lower_half_width * 0.30),
                    material: MaterialRole::RolledArmor,
                    smoothing: SG_HARD,
                },
            )
            .capped_revolve_at(
                Vec3::new(x, hull.deck_y + 0.078, center_z),
                RevolveSpec {
                    profile: vec![
                        ProfilePoint::new(hull.lower_half_width * 0.22, 0.0),
                        ProfilePoint::new(hull.lower_half_width * 0.22, 0.012),
                    ],
                    axis: Axis::Y,
                    segments: round_segments(hull.lower_half_width * 0.22),
                    material: MaterialRole::TrackMetal,
                    smoothing: SG_HARD,
                },
            )
            .build();
        parts.push(part(
            PartKey::indexed("deck_grille", i as u16),
            MaterialRole::TrackMetal,
            PartLod::Detail,
            grille,
        ));
    }

    // --- the stern: twin stacks, each in a three-sided late-E shield on the near-vertical
    // stern (the Tiger I), or open pipes standing off a leaned stern — vertical, footed on the
    // plate low down, held at the top by a bracket to the plate that has sloped away beneath
    // them, with the dark open mouth a pipe has (the Tiger II) ----------------------------------
    let rear = hull.rear_slope_deg.to_radians().tan();
    let stern_z = |y: f32| -hull.half_len + (y - hull.sponson_y).abs() * rear;
    let (stack_x, stack_z, stack_top) = if deck.exhaust_shields {
        (0.62_f32, -hull.half_len + 0.12, 2.02_f32)
    } else {
        (hull.lower_half_width * 0.55, -hull.half_len + 0.10, hull.deck_y + 0.16)
    };
    for (i, x) in [-stack_x, stack_x].into_iter().enumerate() {
        let z = stack_z;
        let stack = MeshBuilder::new()
            .capped_revolve_at(
                Vec3::new(x, 0.0, z),
                RevolveSpec {
                    profile: vec![
                        ProfilePoint::new(0.10, 1.00),
                        ProfilePoint::new(0.10, stack_top),
                    ],
                    axis: Axis::Y,
                    segments: round_segments(0.10),
                    material: MaterialRole::RolledArmor,
                    smoothing: SG_HARD,
                },
            )
            .build();
        parts.push(part(
            PartKey::indexed("exhaust_stack", i as u16),
            MaterialRole::RolledArmor,
            PartLod::Silhouette,
            stack,
        ));
        if !deck.exhaust_shields {
            // The dark open mouth — the pipe is a pipe, not a capped rod.
            parts.push(part(
                PartKey::indexed("exhaust_mouth", i as u16),
                MaterialRole::TrackMetal,
                PartLod::Detail,
                MeshBuilder::new()
                    .capped_revolve_at(
                        Vec3::new(x, 0.0, z),
                        RevolveSpec {
                            profile: vec![
                                ProfilePoint::new(0.06, stack_top + 0.001),
                                ProfilePoint::new(0.06, stack_top + 0.005),
                            ],
                            axis: Axis::Y,
                            segments: round_segments(0.06),
                            material: MaterialRole::TrackMetal,
                            smoothing: SG_HARD,
                        },
                    )
                    .build(),
            ));
            // The top bracket from the pipe back to the plate that leaned away under it.
            let bracket_y = hull.deck_y - 0.06;
            let plate = stern_z(bracket_y);
            if plate - z > 0.06 {
                parts.push(part(
                    PartKey::indexed("exhaust_bracket", i as u16),
                    MaterialRole::RolledArmor,
                    PartLod::Detail,
                    MeshBuilder::new()
                        .plate_box(
                            Vec3::new(x, bracket_y, (plate + z) * 0.5),
                            Vec3::new(0.05, 0.02, (plate - z) * 0.5),
                            0.008,
                            MaterialRole::RolledArmor,
                            SG_HARD,
                        )
                        .build(),
                ));
            }
            continue;
        }
        let mut shield = MeshBuilder::new().plate_box(
            Vec3::new(x, 1.55, z - 0.16),
            Vec3::new(0.17, 0.55, 0.012),
            0.012,
            MaterialRole::RolledArmor,
            SG_HARD,
        );
        for side in [-1.0_f32, 1.0] {
            shield = shield.plate_box(
                Vec3::new(x + side * 0.17, 1.55, z - 0.02),
                Vec3::new(0.012, 0.55, 0.15),
                0.012,
                MaterialRole::RolledArmor,
                SG_HARD,
            );
        }
        parts.push(part(
            PartKey::indexed("exhaust_shield", i as u16),
            MaterialRole::RolledArmor,
            PartLod::Detail,
            shield.build(),
        ));
    }

    // --- the bow: spare links on the lower plate, the visor and the MG ball on the driver's --
    let glacis = hull.glacis_slope_deg.to_radians().tan();
    // Where the driver's plate stands at height `y`: above an authored bow shelf, leaning back
    // from the shelf's top edge; below it, the lower bow plate from the nose line; without a
    // shelf, the prism folds at the SPONSON step (the slab library's own rule) — seated from
    // the nose line the Tiger II's MG ball stood 0.48 m inside the hull.
    let plate_z = |y: f32| match bp.armor.hull_bow_shelf {
        Some((top, setback)) if y >= top => hull.half_len - setback - (y - top) * glacis,
        Some(_) => hull.half_len - (y - hull.belly_y - hull.nose_rise).max(0.0) * glacis,
        None => hull.half_len - (y - hull.sponson_y).max(0.0) * glacis,
    };
    // The Tiger I's rack: four links across the plate, 0.48 m apart from x -0.72.
    for i in 0..u16::from(deck.spare_links) {
        let x = -0.72 + f32::from(i) * 0.48;
        parts.push(part(
            PartKey::indexed("spare_track", i),
            MaterialRole::TrackMetal,
            PartLod::Detail,
            MeshBuilder::new()
                .plate_box(
                    Vec3::new(x, 0.88, plate_z(0.88) - 0.065),
                    Vec3::new(0.20, 0.115, 0.045),
                    0.04,
                    MaterialRole::TrackMetal,
                    SG_HARD,
                )
                .build(),
        ));
    }
    if deck.driver_visor {
        parts.push(part(
            PartKey::new("driver_visor"),
            MaterialRole::RolledArmor,
            PartLod::Detail,
            MeshBuilder::new()
                .plate_box(
                    Vec3::new(0.55, 1.62, plate_z(1.62) + 0.02),
                    Vec3::new(0.20, 0.07, 0.05),
                    0.03,
                    MaterialRole::RolledArmor,
                    SG_HARD,
                )
                .build(),
        ));
    }
    if deck.periscope_hood {
        // The driver's periscope hood riding the glacis line at the deck's front edge, just
        // left of the centre (the Tiger II recipe's own station).
        let run = (hull.deck_y - hull.sponson_y) * glacis;
        parts.push(part(
            PartKey::new("periscope_hood"),
            MaterialRole::RolledArmor,
            PartLod::Detail,
            MeshBuilder::new()
                .plate_box(
                    Vec3::new(0.60, hull.deck_y + 0.03, hull.half_len - run - 0.30),
                    Vec3::new(0.16, 0.06, 0.12),
                    0.03,
                    MaterialRole::RolledArmor,
                    SG_HARD,
                )
                .build(),
        ));
    }
    // The bow MG ball at its station on the driver's plate (the Tiger's by default).
    let (ball_x, ball_y) = deck.mg_ball.unwrap_or((-0.62, 1.58));
    parts.push(VehiclePart {
        key: PartKey::new("course_mg_port"),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::CastArmor,
        smoothing: SG_HARD,
        shape: PartShape::Mesh(
            MeshBuilder::new()
                .capped_revolve_at(
                    Vec3::new(ball_x, ball_y, 0.0),
                    RevolveSpec {
                        profile: vec![
                            ProfilePoint::new(0.13, plate_z(ball_y) - 0.04),
                            ProfilePoint::new(0.09, plate_z(ball_y) + 0.09),
                        ],
                        axis: Axis::Z,
                        segments: round_segments(0.13),
                        material: MaterialRole::CastArmor,
                        smoothing: SG_HARD,
                    },
                )
                .build(),
        ),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    });

    if deck.twin_periscopes {
        // The driver's twin periscope hoods at the roof's front edge, left — each a raked head
        // with its GLASS lying in the raked face (the Panther's read), as plate solids.
        let edge = hull.half_len - (hull.deck_y - hull.sponson_y) * glacis;
        for (i, x) in [0.72_f32, 0.46].into_iter().enumerate() {
            let center = Vec3::new(x, hull.deck_y + 0.035, edge - 0.10);
            let half = Vec3::new(0.055, 0.035, 0.055);
            for (k, (material, solid)) in [
                (MaterialRole::RolledArmor, crate::periscope(center, half)),
                (MaterialRole::Glass, crate::periscope_prism(center, half)),
            ]
            .into_iter()
            .enumerate()
            {
                parts.push(VehiclePart {
                    key: PartKey::indexed("periscope_hood", (i * 2 + k) as u16),
                    submesh: SubmeshKind::Hull,
                    material,
                    smoothing: SmoothingGroup::hard_edges(),
                    shape: PartShape::Plates(solid),
                    lod: PartLod::Detail,
                    generator: GeneratorKind::Solid,
                });
            }
        }
    }

    if deck.flank_stowage {
        // Hull-flank stowage on the leaned sponson plates: the tow cable, the jack and its timber
        // block, the tool box — every piece lies ON the armour plane the sponson's upper side
        // leans on, never floating off it (the Jagdtiger recipe's rule, lifted below the seam).
        let lean = bp.armor.hull_side.0.to_radians().tan();
        let wall_x = |y: f32| hull.half_width - (y - hull.sponson_y) * lean;
        // (key, centre Z, half length along Z, centre Y, half height, proud of the plate)
        let items = [
            ("tow_cable", 2.05_f32, 0.42_f32, 1.62_f32, 0.085_f32, 0.070_f32),
            ("tool_jack", 1.05, 0.30, 1.60, 0.100, 0.085),
            ("tool_block", 0.20, 0.50, 1.63, 0.060, 0.050),
            ("tool_box", -1.55, 0.46, 1.61, 0.090, 0.075),
        ];
        for (side, sign) in [-1.0_f32, 1.0].into_iter().enumerate() {
            for (key, cz, half_z, cy, half_y, proud) in items {
                parts.push(part(
                    PartKey::indexed(key, side as u16),
                    MaterialRole::TrackMetal,
                    PartLod::Detail,
                    MeshBuilder::new()
                        .extrude(
                            Vec3::new(0.0, 0.0, cz),
                            ExtrudeSpec {
                                section: crate::parts_casemate::plate_pad(
                                    wall_x,
                                    sign,
                                    cy - half_y,
                                    cy + half_y,
                                    proud,
                                ),
                                axis: Axis::Z,
                                half_depth: half_z,
                                material: MaterialRole::TrackMetal,
                                smoothing: SG_HARD,
                            },
                        )
                        .build(),
                ));
            }
        }
    }

    // --- the fender flaps over the wraps, the Panther's curved sweep, or the Jagdtiger's flat
    // guards over the bow wrap ------------------------------------------------------------------
    let track = &bp.track;
    let band_half = ((track.outer_x - track.inner_x) * 0.5).max(0.05);
    let wrap_outer = track.end_radius + 0.02 + 0.055;
    if deck.flat_bow_guards {
        // Large FLAT guards over the front sprockets with a downturned lip closing the leading
        // edge — the Jagdtiger's read, unlike the Tiger II's drooping hinged flaps.
        for (i, sign) in [-1.0_f32, 1.0].into_iter().enumerate() {
            parts.push(part(
                PartKey::indexed("fender_guard", i as u16),
                MaterialRole::RolledArmor,
                PartLod::Detail,
                MeshBuilder::new()
                    .plate_box(
                        Vec3::new(sign * track.center_x, hull.sponson_y + 0.03, track.end_z + 0.18),
                        Vec3::new(band_half * 0.96, 0.015, 0.42),
                        0.02,
                        MaterialRole::RolledArmor,
                        SG_HARD,
                    )
                    .plate_box(
                        Vec3::new(sign * track.center_x, hull.sponson_y - 0.03, track.end_z + 0.59),
                        Vec3::new(band_half * 0.92, 0.05, 0.015),
                        0.012,
                        MaterialRole::RolledArmor,
                        SG_HARD,
                    )
                    .build(),
            ));
        }
        return parts;
    }
    if deck.curved_sweep {
        // Three chained slanted segments per side approximating the Panther's quarter-round
        // mudguard: the chain rides ABOVE the wrap circle's top and only drops once past the
        // circle's front edge (the recipe's clearance math, lifted below the seam).
        let crown_y = (track.end_y + wrap_outer + 0.03).max(hull.sponson_y + 0.04);
        let front = track.end_z + wrap_outer;
        let sweep = [
            (track.end_z - 0.10, crown_y, front + 0.02, crown_y - 0.02),
            (front + 0.02, crown_y - 0.02, front + 0.26, crown_y - 0.20),
            (front + 0.26, crown_y - 0.20, front + 0.38, crown_y - 0.46),
        ];
        let mut index = 0u16;
        for sign in [-1.0_f32, 1.0] {
            for &(z0, y0, z1, y1) in &sweep {
                let mid_z = (z0 + z1) * 0.5;
                parts.push(part(
                    PartKey::indexed("fender_sweep", index),
                    MaterialRole::RolledArmor,
                    PartLod::Detail,
                    MeshBuilder::new()
                        .extrude(
                            Vec3::new(sign * track.center_x, 0.0, mid_z),
                            ExtrudeSpec {
                                section: vec![
                                    Vec2::new(z0 - mid_z, y0 - 0.020),
                                    Vec2::new(z1 - mid_z, y1 - 0.020),
                                    Vec2::new(z1 - mid_z, y1),
                                    Vec2::new(z0 - mid_z, y0),
                                ],
                                axis: Axis::X,
                                half_depth: band_half * 0.96,
                                material: MaterialRole::RolledArmor,
                                smoothing: SG_HARD,
                            },
                        )
                        .build(),
                ));
                index += 1;
            }
        }
        return parts;
    }
    let hinge_y =
        guard_top_y.unwrap_or_else(|| (track.end_y + wrap_outer + 0.03).max(hull.sponson_y + 0.02));
    let (droop_dz, droop_dy) = (0.45_f32, 0.22_f32);
    let mut index = 0u16;
    let ends: &[f32] = if deck.rear_flaps { &[1.0, -1.0] } else { &[1.0] };
    for &end_sign in ends {
        let hinge_z = end_sign * (track.end_z + 0.12);
        let mut section = vec![
            Vec2::new(0.0, -0.018),
            Vec2::new(droop_dz, -droop_dy - 0.018),
            Vec2::new(droop_dz, -droop_dy),
            Vec2::new(0.0, 0.0),
        ];
        if end_sign < 0.0 {
            for point in &mut section {
                point.x = -point.x;
            }
            section.reverse();
        }
        for sign in [-1.0_f32, 1.0] {
            let flap = MeshBuilder::new()
                .extrude(
                    Vec3::new(sign * track.center_x, hinge_y, hinge_z),
                    ExtrudeSpec {
                        section: section.clone(),
                        axis: Axis::X,
                        half_depth: band_half * 0.96,
                        material: MaterialRole::RolledArmor,
                        smoothing: SG_HARD,
                    },
                )
                .plate_box(
                    Vec3::new(sign * track.center_x, hinge_y + 0.014, hinge_z),
                    Vec3::new(band_half * 0.80, 0.014, 0.024),
                    0.008,
                    MaterialRole::BarrelSteel,
                    SG_HARD,
                )
                .build();
            parts.push(part(
                PartKey::indexed("fender_flap", index),
                MaterialRole::RolledArmor,
                PartLod::Detail,
                flap,
            ));
            index += 1;
        }
    }
    parts
}
