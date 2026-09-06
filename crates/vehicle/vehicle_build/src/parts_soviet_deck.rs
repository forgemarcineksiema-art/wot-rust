//! The Soviet deck and hull furniture as library parts (Forge 2.0 K3, the IS-3 and the T-34-85,
//! 2026-09-06): the transverse louvre strips over the engine bay, the V-2's twin round exhaust
//! ports low on the sloped stern with their dark open mouths, the glacis furniture the T-34
//! carries in its plate — the driver's hatch with its twin periscope hoods and the hull MG ball
//! — and the IS family's fender line with the external fuel drums. Every dimension is the
//! blueprint's (the recipes' own sections, lifted below the seam); which furniture a vehicle
//! wears is its visual file's (`SovietDeckVisual`).

use game_core::roundness::round_segments;
use game_core::{SovietDeckVisual, VehicleBlueprint};
use glam::{Vec2, Vec3};
use vehicle_geometry::{
    Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec,
    SubmeshKind,
};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};
use crate::smoothing::{SG_CAST, SG_HARD};

/// The Soviet deck for `bp`, or `None` when its visual file authors none.
pub fn soviet_deck_parts_for_blueprint(bp: &VehicleBlueprint) -> Option<Vec<VehiclePart>> {
    let visual = bp.visual_detail()?;
    visual.construction?;
    let deck = visual.soviet_deck?;
    Some(soviet_deck_parts(bp, &deck))
}

/// Louvres, exhaust ports, the glacis furniture, the fender line and the drums `deck` selects.
pub fn soviet_deck_parts(bp: &VehicleBlueprint, deck: &SovietDeckVisual) -> Vec<VehiclePart> {
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

    // --- the engine deck: transverse louvre strips over the bay (the T-34/IS read) ------------
    let deck_back = -hull.half_len + 0.35;
    let front = (-1.15_f32).min(bp.turret.ring_z - bp.turret.plan_half_length - 0.15);
    let strips = usize::from(deck.louvres).max(1);
    for i in 0..strips {
        let t = (i as f32 + 0.5) / strips as f32;
        let z = front + (deck_back - front) * t;
        parts.push(part(
            PartKey::indexed("engine_deck_louvre", i as u16),
            MaterialRole::RolledArmor,
            PartLod::Detail,
            MeshBuilder::new()
                .plate_box(
                    Vec3::new(0.0, hull.deck_y + 0.038, z),
                    Vec3::new(hull.lower_half_width * 0.72, 0.024, 0.09),
                    0.02,
                    MaterialRole::RolledArmor,
                    SG_HARD,
                )
                .build(),
        ));
    }

    // --- the stern: the V-2's twin round exhaust ports ON the sloped rear plate ---------------
    let rear = hull.rear_slope_deg.to_radians().tan();
    let port_y = deck.exhaust_port_y;
    let plate_z = -hull.half_len + (port_y - hull.sponson_y).abs() * rear;
    for (i, x) in [-0.36_f32, 0.36].into_iter().enumerate() {
        let c = Vec3::new(x, port_y, plate_z + 0.02);
        parts.push(part(
            PartKey::indexed("exhaust_port", i as u16),
            MaterialRole::RolledArmor,
            PartLod::Detail,
            MeshBuilder::new()
                .capped_revolve_at(
                    c,
                    RevolveSpec {
                        profile: vec![
                            ProfilePoint::new(0.085, -0.05),
                            ProfilePoint::new(0.085, 0.06),
                        ],
                        axis: Axis::Z,
                        segments: round_segments(0.085),
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
                        axis: Axis::Z,
                        segments: round_segments(0.055),
                        material: MaterialRole::TrackMetal,
                        smoothing: SG_HARD,
                    },
                )
                .build(),
        ));
    }

    // --- the glacis furniture (the T-34's plate): the driver's hatch with its twin periscope
    // hoods, and the hull MG ball right of it --------------------------------------------------
    let glacis = hull.glacis_slope_deg.to_radians();
    // A point on the glacis plane at height `y` (the prism folds at the sponson step), pushed
    // `standoff` along the plate's TRUE normal `(0, sin g, cos g)` for a plate leaning `g` from
    // vertical — the recipe pushed along `(0, cos g, sin g)`, 30° off the normal on this glacis.
    let on_glacis = |x: f32, y: f32, standoff: f32| -> Vec3 {
        let run = (y - hull.sponson_y).max(0.0) * glacis.tan();
        Vec3::new(x, y, hull.half_len - run) + Vec3::new(0.0, glacis.sin(), glacis.cos()) * standoff
    };
    if let Some((x, y)) = deck.glacis_hatch {
        let hatch_c = on_glacis(x, y, 0.02);
        parts.push(part(
            PartKey::new("glacis_hatch"),
            MaterialRole::RolledArmor,
            PartLod::Detail,
            MeshBuilder::new()
                .plate_box(
                    hatch_c,
                    Vec3::new(0.28, 0.30, 0.035),
                    0.03,
                    MaterialRole::RolledArmor,
                    SG_HARD,
                )
                .build(),
        ));
        for (i, dx) in [-0.14_f32, 0.14].into_iter().enumerate() {
            let hood_c = hatch_c + Vec3::new(dx, 0.26, 0.05);
            let hood_half = Vec3::new(0.055, 0.05, 0.05);
            for (k, (material, solid)) in [
                (MaterialRole::RolledArmor, crate::periscope(hood_c, hood_half)),
                (MaterialRole::Glass, crate::periscope_prism(hood_c, hood_half)),
            ]
            .into_iter()
            .enumerate()
            {
                parts.push(VehiclePart {
                    key: PartKey::indexed("periscope_hood", (i * 2 + k) as u16),
                    submesh: SubmeshKind::Hull,
                    material,
                    smoothing: SG_HARD,
                    shape: PartShape::Plates(solid),
                    lod: PartLod::Detail,
                    generator: GeneratorKind::Solid,
                });
            }
        }
    }
    if let Some((x, y)) = deck.glacis_mg_ball {
        parts.push(VehiclePart {
            key: PartKey::new("course_mg_port"),
            submesh: SubmeshKind::Hull,
            material: MaterialRole::CastArmor,
            smoothing: SG_HARD,
            shape: PartShape::Mesh(
                MeshBuilder::new()
                    .capped_revolve_at(
                        on_glacis(x, y, 0.0),
                        RevolveSpec {
                            profile: vec![
                                ProfilePoint::new(0.12, -0.03),
                                ProfilePoint::new(0.09, 0.11),
                            ],
                            axis: Axis::Z,
                            segments: round_segments(0.12),
                            material: MaterialRole::CastArmor,
                            smoothing: SG_HARD,
                        },
                    )
                    .build(),
            ),
            lod: PartLod::Detail,
            generator: GeneratorKind::Revolve,
        });
    }

    // --- the IS family's fender line and the external fuel drums -------------------------------
    let track = &bp.track;
    if deck.is_fenders {
        // A thin full-length shelf over the belt, the front mudguard sloping down over the idler,
        // and a short rear flap over the sprocket — each panel one part.
        let center_x = (track.inner_x + track.outer_x) * 0.5;
        let half_x = (track.outer_x - track.inner_x) * 0.5 - 0.005;
        let shelf_y = hull.sponson_y + 0.015;
        let belt_top = track.end_y + track.end_radius + 0.04;
        let shelf_front_z = hull.half_len - 1.35;
        let guard_tip_z = track.end_z + track.end_radius + 0.20;
        let rear_tip_z = -(track.end_z + track.end_radius + 0.20);
        let panels: [(&'static str, Vec<Vec2>); 3] = [
            (
                "fender_shelf",
                vec![
                    Vec2::new(-hull.half_len + 0.15, shelf_y),
                    Vec2::new(shelf_front_z, shelf_y),
                    Vec2::new(shelf_front_z, shelf_y + 0.035),
                    Vec2::new(-hull.half_len + 0.15, shelf_y + 0.035),
                ],
            ),
            (
                "fender_front",
                vec![
                    Vec2::new(shelf_front_z, shelf_y),
                    Vec2::new(guard_tip_z, belt_top),
                    Vec2::new(guard_tip_z, belt_top + 0.035),
                    Vec2::new(shelf_front_z, shelf_y + 0.035),
                ],
            ),
            (
                "fender_rear",
                vec![
                    Vec2::new(rear_tip_z, belt_top),
                    Vec2::new(-hull.half_len + 0.15, shelf_y),
                    Vec2::new(-hull.half_len + 0.15, shelf_y + 0.035),
                    Vec2::new(rear_tip_z, belt_top + 0.035),
                ],
            ),
        ];
        for (side, sign) in [1.0_f32, -1.0].into_iter().enumerate() {
            for (key, section) in &panels {
                // The section is symmetric across the belt, so the port panel is the same
                // extrusion centred on the port belt.
                let mesh = MeshBuilder::new()
                    .extrude(
                        Vec3::new(sign * center_x, 0.0, 0.0),
                        ExtrudeSpec {
                            section: section.clone(),
                            axis: Axis::X,
                            half_depth: half_x,
                            material: MaterialRole::RolledArmor,
                            smoothing: SG_HARD,
                        },
                    )
                    .build();
                parts.push(part(
                    PartKey::indexed(key, side as u16),
                    MaterialRole::RolledArmor,
                    PartLod::Silhouette,
                    mesh,
                ));
            }
        }
    }
    if deck.fuel_drums {
        let center_x = (track.inner_x + track.outer_x) * 0.5;
        let shelf_y = hull.sponson_y + 0.05;
        let radius = 0.145;
        let (back_z, front_z) = (-hull.half_len + 0.35, -hull.half_len + 1.15);
        for (i, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
            parts.push(VehiclePart {
                key: PartKey::indexed("fuel_tank", i as u16),
                submesh: SubmeshKind::Hull,
                material: MaterialRole::RolledArmor,
                smoothing: SG_CAST,
                shape: PartShape::Mesh(
                    MeshBuilder::new()
                        .capped_revolve_at(
                            Vec3::new(side * center_x, shelf_y + radius, 0.0),
                            RevolveSpec {
                                profile: vec![
                                    ProfilePoint::new(radius, back_z),
                                    ProfilePoint::new(radius, front_z),
                                ],
                                axis: Axis::Z,
                                segments: round_segments(radius),
                                material: MaterialRole::RolledArmor,
                                smoothing: SG_CAST,
                            },
                        )
                        .build(),
                ),
                lod: PartLod::Silhouette,
                generator: GeneratorKind::Revolve,
            });
        }
    }
    parts
}
