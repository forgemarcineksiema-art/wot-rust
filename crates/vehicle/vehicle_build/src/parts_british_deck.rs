//! The British deck as library parts (Forge 2.0 K3, the Centurion Mk 3, 2026-09-06): the flat
//! raised engine panel with its broad mesh-grille frame, the low armoured exhaust cowls on the
//! rear deck flanks with their dark outlet slots, the driver's rectangular split hatch on the
//! roof, and the fender stowage boxes with the port lamp on them. Every dimension is the
//! blueprint's (the recipe's own sections, lifted below the seam); which furniture a vehicle wears
//! is its visual file's (`BritishDeckVisual`).

use game_core::{BritishDeckVisual, VehicleBlueprint};
use glam::Vec3;
use vehicle_geometry::{GeometryMesh, MaterialRole, MeshBuilder, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};
use crate::smoothing::SG_HARD;

/// The British deck for `bp`, or `None` when its visual file authors none.
pub fn british_deck_parts_for_blueprint(bp: &VehicleBlueprint) -> Option<Vec<VehiclePart>> {
    let visual = bp.visual_detail()?;
    visual.construction?;
    let deck = visual.british_deck?;
    Some(british_deck_parts(bp, &deck))
}

/// The engine panel and grille, the cowls, the roof hatch and the fender boxes `deck` selects.
pub fn british_deck_parts(bp: &VehicleBlueprint, deck: &BritishDeckVisual) -> Vec<VehiclePart> {
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

    // --- the engine deck: one flat raised panel with a broad mesh-grille frame -----------------
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
                Vec3::new(hull.lower_half_width * 0.8, 0.02, (front - deck_back).abs() * 0.5),
                0.04,
                MaterialRole::RolledArmor,
                SG_HARD,
            )
            .build(),
    ));
    parts.push(part(
        PartKey::indexed("deck_grille", 0),
        MaterialRole::TrackMetal,
        PartLod::Detail,
        MeshBuilder::new()
            .plate_box(
                Vec3::new(0.0, hull.deck_y + 0.045, center_z + 0.15),
                Vec3::new(hull.lower_half_width * 0.5, 0.012, 0.35),
                0.02,
                MaterialRole::TrackMetal,
                SG_HARD,
            )
            .build(),
    ));

    // --- the stern: low armoured exhaust cowls on the rear deck flanks (the Meteor's read) ----
    if deck.exhaust_cowls {
        let z = -hull.half_len + 0.55;
        for (i, sign) in [-1.0_f32, 1.0].into_iter().enumerate() {
            let x = sign * hull.lower_half_width * 0.62;
            parts.push(part(
                PartKey::indexed("exhaust_cowl", i as u16),
                MaterialRole::RolledArmor,
                PartLod::Detail,
                MeshBuilder::new()
                    .plate_box(
                        Vec3::new(x, hull.deck_y + 0.055, z),
                        Vec3::new(0.14, 0.05, 0.30),
                        0.03,
                        MaterialRole::RolledArmor,
                        SG_HARD,
                    )
                    // The dark outlet slot facing rearward under the cowl lip.
                    .plate_box(
                        Vec3::new(x, hull.deck_y + 0.035, z - 0.30),
                        Vec3::new(0.10, 0.028, 0.012),
                        0.008,
                        MaterialRole::TrackMetal,
                        SG_HARD,
                    )
                    .build(),
            ));
        }
    }

    // --- the driver's rectangular split hatch on the roof, its hinge bar and grab handle -------
    if let Some((x, z_behind_edge, half_x, half_z)) = deck.driver_roof_hatch {
        let edge = hull.half_len
            - (hull.deck_y - hull.sponson_y) * hull.glacis_slope_deg.to_radians().tan();
        let c = Vec3::new(x, hull.deck_y + 0.002, edge - z_behind_edge);
        parts.push(part(
            PartKey::new("driver_roof_hatch"),
            MaterialRole::RolledArmor,
            PartLod::Detail,
            MeshBuilder::new()
                .plate_box(
                    c,
                    Vec3::new(half_x, 0.022, half_z),
                    0.045,
                    MaterialRole::RolledArmor,
                    SG_HARD,
                )
                .plate_box(
                    Vec3::new(c.x, c.y + 0.012, c.z + half_z),
                    Vec3::new(half_x * 0.8, 0.014, 0.02),
                    0.008,
                    MaterialRole::BarrelSteel,
                    SG_HARD,
                )
                .plate_box(
                    Vec3::new(c.x, c.y + 0.03, c.z - half_z * 0.6),
                    Vec3::new(0.05, 0.012, 0.015),
                    0.006,
                    MaterialRole::BarrelSteel,
                    SG_HARD,
                )
                .build(),
        ));
    }

    // --- the fender stowage boxes over the belts, the port lamp standing on the front one ------
    if let Some((box_half_y, box_half_z)) = deck.fender_boxes {
        let track = &bp.track;
        let center_x = (track.inner_x + track.outer_x) * 0.5;
        let half_x = (track.outer_x - track.inner_x) * 0.5 - 0.01;
        let y = hull.sponson_y + 0.03 + box_half_y;
        let stations = [hull.half_len - 0.95, -hull.half_len + 0.95];
        let mut index = 0u16;
        for sign in [-1.0_f32, 1.0] {
            for z in stations {
                parts.push(part(
                    PartKey::indexed("stowage_bin", index),
                    MaterialRole::RolledArmor,
                    PartLod::Detail,
                    MeshBuilder::new()
                        .plate_box(
                            Vec3::new(sign * center_x, y, z),
                            Vec3::new(half_x, box_half_y, box_half_z),
                            0.02,
                            MaterialRole::RolledArmor,
                            SG_HARD,
                        )
                        .build(),
                ));
                index += 1;
            }
        }
    }
    parts
}
