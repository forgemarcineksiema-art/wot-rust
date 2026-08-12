//! The interior stays inside the armour it lives in — measured against the BLUEPRINT, because the
//! 2026-07-29 audit found three ways it had drifted out: side liners at a literal ±1.12 (a relic
//! of a hull the vehicle no longer has — painted walls standing 90 mm outside the armour), ammo
//! and fuel authored against the old 1.05 tub (rounds buried 40 mm inside the 80 mm side plates,
//! tank faces a centimetre PAST the hull), and final drives 18 cm behind the sprocket axle they
//! turn.
//!
//! One penetration is legitimate and named: the final drives cross the hull side, because the
//! drive must reach the sprocket. Everything else lives in the clear space the tub leaves.

use game_core::{VehicleBlueprint, VehicleKind};
use vehicle_build::t54_description;
use vehicle_geometry::MaterialRole;

/// The hull side armour the interior must respect (armour catalog: 80 mm vertical plates).
const SIDE_ARMOR_M: f32 = 0.080;

#[test]
fn the_interior_lives_inside_the_hull_walls_the_blueprint_authors() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let wall_inner = bp.hull.half_width - SIDE_ARMOR_M;
    let description = t54_description();

    // The named, mechanically-required penetrations, each with ITS OWN reach limit — an axle may
    // cross the wall to the part it turns, and no further:
    //   * final drives cross to the sprocket hub;
    //   * torsion bars cross to the swing-arm hub just past the plate.
    let penetrations: [(&str, f32); 2] =
        [("final_drive", bp.track.inner_x + 0.30), ("torsion_bar", bp.hull.half_width + 0.05)];

    let mut worst: Option<(String, f32)> = None;
    for part in &description.parts {
        if let Some((_, limit)) = penetrations.iter().find(|(name, _)| part.key.name.contains(name))
        {
            if let Some(b) = part.mesh().bounds() {
                let reach = b.max.x.max(-b.min.x);
                assert!(
                    reach <= *limit,
                    "{} crosses the wall to its hub and STOPS there: {reach:.3} vs {limit:.3}",
                    part.key.name
                );
            }
            continue;
        }
        // The turret's own interior answers to the casting — in `t54_turret_containment`, which
        // measures every turret-frame vertex against the finished dome. This claim used to name
        // "its skin and basket tests", which check the skin's bounds and normals and never look
        // at what lives inside it; on the strength of that sentence four assemblies walked out
        // through the armour (2026-08-10 audit). Name the lock, not a hope.
        if part.key.name == "turret_inner_skin" {
            continue;
        }
        let mesh = part.mesh();
        let interior = mesh.vertices().iter().any(|v| {
            matches!(
                v.material,
                MaterialRole::InteriorPrimer
                    | MaterialRole::InteriorMachinery
                    | MaterialRole::Ammunition
            )
        });
        if !interior {
            continue;
        }
        if let Some(b) = mesh.bounds() {
            // Judge what lives between the hull walls; turret-carried interior sits above 1.58
            // and is judged against the CASTING by `t54_turret_containment`. Measuring it here
            // would ask the wrong question anyway: `wall_inner` is the hull's side plate, and a
            // radio panel 0.91 out passes that while standing clear outside the dome.
            if b.min.y > 1.58 {
                continue;
            }
            let reach = b.max.x.max(-b.min.x);
            if worst.as_ref().map(|(_, w)| reach > *w).unwrap_or(true) {
                worst = Some((part.key.name.to_string(), reach));
            }
            assert!(
                reach <= wall_inner + 1.0e-3,
                "{} reaches {reach:.3} — inside (or through) the {SIDE_ARMOR_M} m side plate on \
                 a {:.3} tub. The interior is authored against the blueprint, not a remembered \
                 hull.",
                part.key.name,
                bp.hull.half_width
            );
        }
    }
    let (name, reach) = worst.expect("the hull carries interior parts");
    // And the interior actually USES the space: the widest legitimate part sits near the wall.
    assert!(
        reach > wall_inner - 0.10,
        "the widest interior part ({name}) sits at {reach:.3} — a 10 cm air gap to the wall \
         means the interior is still sized for some other hull"
    );
}

#[test]
fn the_final_drives_cross_the_hull_to_their_sprockets() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let layout = game_core::DamageLayout::t54_1951();
    let drives: Vec<_> = layout
        .components()
        .iter()
        .filter(|c| c.kind == game_core::DamageComponentKind::FinalDrive)
        .collect();
    assert_eq!(drives.len(), 2, "one final drive per sprocket");
    for drive in drives {
        let (center, half_length) = match &drive.shape {
            game_core::DamageShape::Cylinder { center, half_length, .. } => (center, half_length),
            other => panic!("a final drive is an axle housing, got {other:?}"),
        };
        assert!(
            center.x.abs() + half_length > bp.hull.half_width,
            "the drive reaches THROUGH the hull side toward the sprocket"
        );
        // And it reaches the sprocket IT HAS: the blueprint's end wheel, not a remembered one.
        assert!(
            (center.z - -(bp.track.end_z)).abs() < 0.05,
            "the housing sits on the sprocket axle: z {:.3} vs blueprint {:.3}",
            center.z,
            -bp.track.end_z
        );
    }
}
