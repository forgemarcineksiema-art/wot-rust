//! The German engine deck and the slab's hull details as library parts (Forge 2.0 K3, step 4d):
//! the deck plate with its two radiator grilles and fan covers, the twin exhaust stacks with
//! their three-sided late-E shields on the stern, the spare-link rack on the lower bow plate,
//! the driver's visor and the bow MG ball on the driver's plate, and the hinged fender flaps
//! over both belt wraps. Every dimension is the blueprint's (the recipe's own sections, lifted
//! below the seam), so the last two recipe pieces of a welded slab vehicle become parts the
//! inventory can name — and the vehicle can leave the recipe behind.

use game_core::VehicleBlueprint;
use game_core::roundness::round_segments;
use glam::{Vec2, Vec3};
use vehicle_geometry::{
    Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec,
    SubmeshKind,
};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};
use crate::smoothing::SG_HARD;

/// The deck and details for `bp`, or `None` when its visual file declares no welded slab
/// construction (the deck belongs to the slab hull it sits on).
pub fn german_deck_parts_for_blueprint(bp: &VehicleBlueprint) -> Option<Vec<VehiclePart>> {
    bp.visual_detail()?.construction?;
    let guard_top_y = bp.visual_detail()?.fender.map(|f| f.center_y + f.half.y);
    Some(german_deck_parts(bp, guard_top_y))
}

/// Deck plate, grilles, stacks and shields, spare links, visor, MG ball, fender flaps.
pub fn german_deck_parts(bp: &VehicleBlueprint, guard_top_y: Option<f32>) -> Vec<VehiclePart> {
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

    // --- the stern: twin stacks, each in a three-sided late-E shield --------------------------
    for (i, x) in [-0.62_f32, 0.62].into_iter().enumerate() {
        let z = -hull.half_len + 0.12;
        let stack = MeshBuilder::new()
            .capped_revolve_at(
                Vec3::new(x, 0.0, z),
                RevolveSpec {
                    profile: vec![ProfilePoint::new(0.10, 1.00), ProfilePoint::new(0.10, 2.02)],
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
    let plate_z = |y: f32| match bp.armor.hull_bow_shelf {
        Some((top, setback)) if y >= top => hull.half_len - setback - (y - top) * glacis,
        _ => hull.half_len - (y - hull.belly_y - hull.nose_rise).max(0.0) * glacis,
    };
    for i in 0..4u16 {
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
    parts.push(VehiclePart {
        key: PartKey::new("course_mg_port"),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::CastArmor,
        smoothing: SG_HARD,
        shape: PartShape::Mesh(
            MeshBuilder::new()
                .capped_revolve_at(
                    Vec3::new(-0.62, 1.58, 0.0),
                    RevolveSpec {
                        profile: vec![
                            ProfilePoint::new(0.13, plate_z(1.58) - 0.04),
                            ProfilePoint::new(0.09, plate_z(1.58) + 0.09),
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

    // --- the fender flaps over both wraps -------------------------------------------------------
    let track = &bp.track;
    let band_half = ((track.outer_x - track.inner_x) * 0.5).max(0.05);
    let wrap_outer = track.end_radius + 0.02 + 0.055;
    let hinge_y =
        guard_top_y.unwrap_or_else(|| (track.end_y + wrap_outer + 0.03).max(hull.sponson_y + 0.02));
    let (droop_dz, droop_dy) = (0.45_f32, 0.22_f32);
    let mut index = 0u16;
    for end_sign in [1.0_f32, -1.0] {
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
