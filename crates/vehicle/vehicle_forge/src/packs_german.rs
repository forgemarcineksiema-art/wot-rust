//! Reference packs for the migrated German line (Tiger I/II, Jagdtiger, Panther II).
//!
//! These share the five-ratio silhouette gate from [`silhouette_ratios`]; the targets are tuned to
//! each vehicle's baked geometry and documented per family. Split from [`crate::packs`] to keep each
//! pack module small and reviewable.

use game_core::VehicleKind;

use crate::{RatioKind, RatioTarget, ReferencePack, ReferenceSource};

/// Standard five-ratio silhouette gate shared by the migrated families. Targets are tuned to each
/// vehicle's baked geometry and documented per family; tolerances are wide enough to survive LOD
/// reduction but tight enough to catch a proportion regressing into the wrong class of tank.
/// Shared with the Soviet heavy pack (`packs_is3`) — one gate, per-vehicle targets.
pub(crate) fn silhouette_ratios(
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
        ReferenceSource::new(
            "In-repo vehicle notes",
            notes_doc,
            "Gameplay translation and source summary.",
        ),
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
            "docs/vehicles/panzerkampfwagen-vi-tiger.md",
        ),
        // Re-tuned to the blueprint-born 1:1 body (6.32 m hull, 3.705 m beam, 3.0 m tall): the
        // documented Tiger is a genuinely DEEP slab (hull-height/length ~0.33), and its broad
        // horseshoe turret is a low band on that tall superstructure — the old targets described
        // the squat legacy stretch.
        silhouette_ratios(
            1.66,
            0.33,
            0.52,
            0.58,
            0.34,
            [
                "Hull plan reads as a long heavy, not a medium.",
                "Tall, slab-sided heavy hull — the deepest body in the German line.",
                "Horseshoe turret is broad but narrower than the full sponson beam.",
                "The turret is a low broad band on the tall superstructure, cupola on the left.",
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
            "docs/vehicles/panzerkampfwagen-vi-b-tiger-ii.md",
        ),
        // Re-tuned to the blueprint-born 1:1 body (7.38 m hull, 3.755 m beam, 3.09 m tall):
        // still visibly flatter than the upright Tiger I (0.28 vs 0.33), and the long Henschel
        // turret is a low sloped band on the deck, not the near-hull-height tower the old
        // legacy-tuned target described.
        silhouette_ratios(
            1.91,
            0.28,
            0.51,
            0.62,
            0.39,
            [
                "Very long sloped heavy hull.",
                "Long sloped hull — flatter than the upright Tiger I.",
                "Sloped Henschel turret is narrower than the broad hull.",
                "Low faceted turret with a rear bustle rides the wide deck.",
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
        // Re-tuned to the blueprint-born 1:1 body (7.80 m hull, 3.64 m beam, 2.95 m roof): the
        // casemate flank now CONTINUES the hull's 25° plane, so the "hull" band carries most of
        // the height and the superstructure reads as a low wide crown on it — the old targets
        // described the legacy tall-box-on-flat-hull construction.
        silhouette_ratios(
            2.10,
            0.26,
            0.73,
            0.53,
            0.37,
            [
                "The longest hull in the German line, stretched from the Tiger II.",
                "The unbroken flank makes the hull band deep; the casemate is welded INTO it.",
                "Wide fixed casemate fills most of the hull width.",
                "A low wide fighting compartment crowns the deep hull, no cupola.",
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
         sloped Panther hull with seven overlapped steel wheels and the narrow Schmalturm.",
        7,
        heavy_sources(
            "https://commons.wikimedia.org/wiki/Panther_tank",
            "https://tank-afv.com/ww2/germany/Panther.php",
            "docs/vehicles/panzerkampfwagen-v-panther-ii.md",
        ),
        // Re-tuned to the blueprint-born 1:1 body (6.87 m hull, 3.42 m beam, 2.99 m tall): the
        // wedge's ramp makes the hull band deeper than the legacy-tuned target, and the narrow
        // Schmalturm is a low crest, not a near-hull-height tower.
        silhouette_ratios(
            1.97,
            0.30,
            0.49,
            0.58,
            0.32,
            [
                "Long sloped medium/heavy hull.",
                "The 55° ramp carries the hull band deep in the Panther proportion.",
                "The Schmalturm is the narrowest turret in the German line.",
                "A low converging turret crest on the wedge, cupola near the centreline.",
                "Long 7.5 cm reaches well past the sloped glacis.",
            ],
        ),
    )
}
