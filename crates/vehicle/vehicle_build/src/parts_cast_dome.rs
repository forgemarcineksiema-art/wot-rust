//! The cast dome turret as library parts (Forge 2.0 K3, the Soviet family and the Centurion's
//! Mk 3 casting, 2026-09-06): the low wide dome lofted from the blueprint's ring and roof
//! (`cast_turret_shell` — the recipe's own casting, lifted below the seam), the roof furniture
//! each vehicle wears (the IS-3's two flush hatches and TPK periscope, the T-34-85's slit cupola
//! and loader's hatch, the Centurion's British cupola and loader's hatch), the ring collar, the
//! broad mantlet socket, and the bustle stowage bin a vehicle authors — each its own part, so
//! the inventory sees a shell, a cupola, hatches, a ring, a socket and a bin. The benchmark's
//! cast turret is the `turret_loft` tree; this class is the rest of the cast fleet's.

use game_core::{BustleBinVisual, CastDomeVisual, CastRoofKind, TurretShape, VehicleBlueprint};
use glam::Vec3;
use vehicle_geometry::{GeometryMesh, MaterialRole, MeshBuilder, SmoothingGroup, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};
use crate::smoothing::{SG_CAST, SG_CUPOLA, SG_HARD, SG_MANTLET, SG_RING};
use crate::turret_fittings::{
    add_british_cupola, add_broad_mantlet_socket, add_commander_periscope, add_flush_ring_hatch,
    add_soviet_slit_cupola_tall, add_turret_ring, cast_turret_shell,
};

/// The cast dome for `bp`, or `None` when its visual file authors no cast dome.
pub fn cast_dome_parts_for_blueprint(bp: &VehicleBlueprint) -> Option<Vec<VehiclePart>> {
    let visual = bp.visual_detail()?.cast_dome?;
    Some(cast_dome_parts(bp, &visual))
}

/// Shell, roof furniture, ring collar, mantlet socket, bustle bin.
pub fn cast_dome_parts(bp: &VehicleBlueprint, v: &CastDomeVisual) -> Vec<VehiclePart> {
    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    let part = |key: PartKey,
                material: MaterialRole,
                smoothing: SmoothingGroup,
                lod: PartLod,
                mesh: GeometryMesh| VehiclePart {
        key,
        submesh: SubmeshKind::Turret,
        material,
        smoothing,
        shape: PartShape::Mesh(mesh),
        lod,
        generator: GeneratorKind::Sweep,
    };
    let mut parts = vec![part(
        PartKey::new("turret_shell"),
        MaterialRole::CastArmor,
        SG_CAST,
        PartLod::Silhouette,
        cast_turret_shell(
            t.ring_z,
            t.base_radius,
            t.plan_half_length,
            t.roof_radius,
            t.ring_y,
            t.roof_y,
            usize::from(v.segments).max(12),
        )
        .build(),
    )];
    parts.extend(roof_furniture(t, v.roof, t.cupola_proud_m(&bp.hull)));
    parts.push(part(
        PartKey::new("turret_ring_collar"),
        MaterialRole::RolledArmor,
        SG_RING,
        PartLod::Silhouette,
        add_turret_ring(
            MeshBuilder::new(),
            t.ring_z,
            t.ring_y,
            t.ring_radius,
            v.ring_height,
            usize::from(v.ring_segments),
        )
        .build(),
    ));
    parts.push(part(
        PartKey::new("mantlet_socket"),
        MaterialRole::CastArmor,
        SG_MANTLET,
        PartLod::Silhouette,
        add_broad_mantlet_socket(
            MeshBuilder::new(),
            bp.gun.trunnion_y,
            mantlet,
            usize::from(v.socket_segments),
        )
        .build(),
    ));
    if let Some(bin) = v.bustle_bin {
        parts.push(part(
            PartKey::new("turret_bin"),
            MaterialRole::RolledArmor,
            SG_HARD,
            PartLod::Silhouette,
            bustle_bin(t, &bin),
        ));
    }
    parts
}

/// What sits on the dome's roof — per VEHICLE, from the photos: one cloned cupola drum used to
/// top every cast turret across three nations (audit #3), so the roof kind is data.
fn roof_furniture(t: &TurretShape, roof: CastRoofKind, proud: f32) -> Vec<VehiclePart> {
    let detail = |key: PartKey, material: MaterialRole, mesh: GeometryMesh| VehiclePart {
        key,
        submesh: SubmeshKind::Turret,
        material,
        smoothing: SG_CUPOLA,
        shape: PartShape::Mesh(mesh),
        lod: PartLod::Detail,
        generator: GeneratorKind::Sweep,
    };
    match roof {
        // NO cupola — the real dome roof carries two flush hatches and the commander's TPK
        // periscope, seated just proud of the flat cap so the rims read at garage distance.
        CastRoofKind::Is3 => vec![
            detail(
                PartKey::indexed("roof_hatch", 0),
                MaterialRole::CastArmor,
                add_flush_ring_hatch(
                    MeshBuilder::new(),
                    t.cupola_x,
                    t.cupola_z,
                    t.roof_y - 0.02,
                    t.cupola_radius,
                    -1.0,
                )
                .build(),
            ),
            detail(
                PartKey::indexed("roof_hatch", 1),
                MaterialRole::CastArmor,
                add_flush_ring_hatch(
                    MeshBuilder::new(),
                    -t.cupola_x,
                    t.cupola_z,
                    t.roof_y - 0.02,
                    t.cupola_radius,
                    1.0,
                )
                .build(),
            ),
            detail(
                PartKey::new("turret_periscope"),
                MaterialRole::CastArmor,
                add_commander_periscope(
                    MeshBuilder::new(),
                    t.cupola_x,
                    t.cupola_z + t.cupola_radius + 0.16,
                    t.roof_y - 0.02,
                )
                .build(),
            ),
        ],
        // The slit-ring cupola with a split lid — its drum built up from 6 cm inside the cap to
        // the authored crown (`cupola_height`; the lid's cap adds 3.5 cm) — and the loader's
        // flush hatch beside it.
        CastRoofKind::T3485 => vec![
            detail(
                PartKey::new("cupola_drum"),
                MaterialRole::CastArmor,
                add_soviet_slit_cupola_tall(
                    MeshBuilder::new(),
                    t.cupola_x,
                    t.cupola_z,
                    t.roof_y - 0.06,
                    t.cupola_radius,
                    (proud + 0.06 - 0.035).max(0.17),
                )
                .build(),
            ),
            detail(
                PartKey::indexed("roof_hatch", 0),
                MaterialRole::CastArmor,
                add_flush_ring_hatch(
                    MeshBuilder::new(),
                    -t.cupola_x * 0.85,
                    t.cupola_z + 0.42,
                    t.roof_y - 0.04,
                    t.cupola_radius * 0.72,
                    -1.0,
                )
                .build(),
            ),
        ],
        // The wide British cupola with its sight hoods, and the loader's flush hatch.
        CastRoofKind::Centurion => vec![
            detail(
                PartKey::new("cupola_drum"),
                MaterialRole::CastArmor,
                add_british_cupola(
                    MeshBuilder::new(),
                    t.cupola_x,
                    t.cupola_z,
                    t.roof_y - 0.06,
                    t.cupola_radius,
                )
                .build(),
            ),
            detail(
                PartKey::indexed("roof_hatch", 0),
                MaterialRole::CastArmor,
                add_flush_ring_hatch(
                    MeshBuilder::new(),
                    -t.cupola_x * 0.85,
                    t.cupola_z + 0.30,
                    t.roof_y - 0.04,
                    t.cupola_radius * 0.60,
                    -1.0,
                )
                .build(),
            ),
        ],
    }
}

/// The stowage bin closing the rear of the turret plan (the Centurion's bustle bin): a
/// chamfered box standing `rise` over the ring seat, its back face `back_inset` ahead of the
/// plan's rear.
fn bustle_bin(t: &TurretShape, bin: &BustleBinVisual) -> GeometryMesh {
    MeshBuilder::new()
        .chamfered_prism(
            Vec3::new(0.0, t.ring_y + bin.rise, t.ring_z - t.plan_half_length + bin.back_inset),
            Vec3::new(bin.half.0, bin.half.1, bin.half.2),
            0.04,
            MaterialRole::RolledArmor,
            SG_HARD,
        )
        .build()
}
