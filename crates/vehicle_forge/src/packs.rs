//! Concrete `ReferencePack` data, one constructor per benchmarked vehicle family.
//!
//! The generic reference types live in [`crate::reference`]; this module is the photo-backed data
//! that proves where a family's proportions come from. The T-54/T-55 family is the first Forge
//! quality benchmark (see `docs/vehicle-forge-policy.md`).

use game_core::VehicleKind;

use crate::{RatioKind, RatioTarget, ReferencePack, ReferenceSource};

pub fn t54_t55_reference_pack() -> ReferencePack {
    ReferencePack::new(
        "t54_t55",
        "T-54/T-55",
        vec![VehicleKind::T54_1951, VehicleKind::T55A],
        "Armored Vehicle Forge benchmark for the T-54/T-55 family: low Soviet medium hull, \
         five-road-wheel running gear, rounded cast turret, D-10 gun family, and photo-backed \
         silhouette ratios.",
        5,
        vec![
            ReferenceSource::new(
                "Wikimedia Commons T-54/T-55 gallery",
                "https://commons.wikimedia.org/wiki/T-54/T-55",
                "Photo reference for family silhouette, turret mass, running gear, and stowage.",
            ),
            ReferenceSource::new(
                "Tank AFV T-55 article",
                "https://tank-afv.com/coldwar/ussr/T-55.php",
                "Technical and visual reference for T-55 dimensions, engine, armament, and family cues.",
            ),
            ReferenceSource::new(
                "Project T-54/T-55 vehicle notes",
                "docs/vehicles/t-54-t-55.md",
                "In-repo gameplay translation and public source summary.",
            ),
        ],
        vec![
            RatioTarget::new(
                RatioKind::HullLengthToWidth,
                1.72,
                0.18,
                "Overall hull plan should read as a compact Soviet medium, not a long heavy.",
            ),
            RatioTarget::new(
                RatioKind::HullHeightToLength,
                0.245,
                0.06,
                "Low Soviet medium silhouette: the hull must read as long and flat, not tall.",
            ),
            RatioTarget::new(
                RatioKind::TurretWidthToHullWidth,
                0.57,
                0.14,
                "Rounded cast turret should be broad but clearly narrower than the track span.",
            ),
            RatioTarget::new(
                RatioKind::TurretHeightToHullHeight,
                0.65,
                0.14,
                "Dome turret stands roughly two-thirds of hull height — present but not a tall casemate.",
            ),
            RatioTarget::new(
                RatioKind::GunProtrusionToHullLength,
                0.37,
                0.14,
                "D-10 family barrel should project decisively past the glacis without reading as a heavy-tank gun.",
            ),
        ],
    )
}

/// Standard five-ratio silhouette gate shared by the migrated families. Targets are tuned to each
/// vehicle's baked geometry and documented per family; tolerances are wide enough to survive LOD
/// reduction but tight enough to catch a proportion regressing into the wrong class of tank.
fn silhouette_ratios(
    hull_len_to_width: f32,
    hull_height_to_len: f32,
    turret_width: f32,
    turret_height: f32,
    gun_protrusion: f32,
    notes: [&str; 5],
) -> Vec<RatioTarget> {
    vec![
        RatioTarget::new(RatioKind::HullLengthToWidth, hull_len_to_width, 0.18, notes[0]),
        RatioTarget::new(RatioKind::HullHeightToLength, hull_height_to_len, 0.06, notes[1]),
        RatioTarget::new(RatioKind::TurretWidthToHullWidth, turret_width, 0.14, notes[2]),
        RatioTarget::new(RatioKind::TurretHeightToHullHeight, turret_height, 0.25, notes[3]),
        RatioTarget::new(RatioKind::GunProtrusionToHullLength, gun_protrusion, 0.16, notes[4]),
    ]
}

fn heavy_sources(gallery: &str, article: &str, notes_doc: &str) -> Vec<ReferenceSource> {
    vec![
        ReferenceSource::new(
            "Wikimedia Commons gallery",
            gallery,
            "Photo reference for silhouette, superstructure mass, running gear, and stowage.",
        ),
        ReferenceSource::new(
            "Tank AFV reference article",
            article,
            "Technical and visual reference for dimensions, armament, and family cues.",
        ),
        ReferenceSource::new("In-repo vehicle notes", notes_doc, "Gameplay translation and source summary."),
    ]
}

pub fn tiger_i_reference_pack() -> ReferencePack {
    ReferencePack::new(
        "tiger_i",
        "Tiger I",
        vec![VehicleKind::TigerI],
        "Armored Vehicle Forge reference for the Tiger I Ausf. E: tall slab-sided heavy hull, \
         interleaved eight-wheel running gear, broad welded box turret, and the long 8.8 cm KwK 36.",
        8,
        heavy_sources(
            "https://commons.wikimedia.org/wiki/Tiger_I",
            "https://tank-afv.com/ww2/germany/Tiger.php",
            "docs/vehicles/tiger-i.md",
        ),
        silhouette_ratios(
            1.77,
            0.233,
            0.51,
            0.86,
            0.35,
            [
                "Hull plan reads as a long heavy, not a medium.",
                "Tall, slab-sided heavy hull — clearly deeper than a Soviet medium.",
                "Welded box turret is broad but narrower than the wide track span.",
                "Boxy turret stands tall on the hull roof.",
                "8.8 cm KwK 36 projects well past the flat nose.",
            ],
        ),
    )
}

pub fn tiger_ii_reference_pack() -> ReferencePack {
    ReferencePack::new(
        "tiger_ii",
        "Tiger II",
        vec![VehicleKind::TigerII],
        "Armored Vehicle Forge reference for the Tiger II Ausf. B: long sloped heavy hull, \
         nine-wheel running gear, narrow sloped turret with a rear bustle, and the 8.8 cm KwK 43.",
        9,
        heavy_sources(
            "https://commons.wikimedia.org/wiki/Tiger_II",
            "https://tank-afv.com/ww2/germany/Tiger_II.php",
            "docs/vehicles/tiger-ii.md",
        ),
        silhouette_ratios(
            1.97,
            0.21,
            0.49,
            0.98,
            0.37,
            [
                "Very long sloped heavy hull.",
                "Low, long sloped hull — flatter than the upright Tiger I.",
                "Sloped turret is narrower than the broad hull.",
                "Tall sloped turret with a rear bustle stands nearly hull-height again.",
                "Long 8.8 cm KwK 43 reaches decisively past the sloped glacis.",
            ],
        ),
    )
}

pub fn jagdtiger_reference_pack() -> ReferencePack {
    ReferencePack::new(
        "jagdtiger",
        "Jagdtiger",
        vec![VehicleKind::Jagdtiger],
        "Armored Vehicle Forge reference for the Jagdtiger: Tiger II chassis carrying a tall fixed \
         casemate superstructure and the 12.8 cm Pak — a non-traversing tank destroyer.",
        9,
        heavy_sources(
            "https://commons.wikimedia.org/wiki/Jagdtiger",
            "https://tank-afv.com/ww2/germany/Jagdtiger.php",
            "docs/vehicles/jagdtiger.md",
        ),
        silhouette_ratios(
            1.97,
            0.146,
            0.74,
            1.56,
            0.49,
            [
                "Long heavy hull shared with the Tiger II.",
                "Low hull — the mass is in the casemate above it, not the hull.",
                "Wide fixed casemate fills most of the hull width.",
                "Casemate superstructure stands taller than the hull itself.",
                "12.8 cm Pak reaches far past the nose.",
            ],
        ),
    )
}

pub fn panther_ii_reference_pack() -> ReferencePack {
    ReferencePack::new(
        "panther_ii",
        "Panther II",
        vec![VehicleKind::PantherII],
        "Armored Vehicle Forge reference for the Panther II (prototype/planned variant): the long \
         sloped Panther hull with eight-wheel running gear and a narrow sloped turret.",
        8,
        heavy_sources(
            "https://commons.wikimedia.org/wiki/Panther_tank",
            "https://tank-afv.com/ww2/germany/Panther.php",
            "docs/vehicles/panther-ii.md",
        ),
        silhouette_ratios(
            1.92,
            0.221,
            0.465,
            0.85,
            0.374,
            [
                "Long sloped medium/heavy hull.",
                "Low sloped hull in the Panther family proportion.",
                "Narrow sloped turret, clearly inset from the wide tracks.",
                "Turret stands a little under hull height.",
                "Long 7.5 cm reaches well past the sloped glacis.",
            ],
        ),
    )
}
