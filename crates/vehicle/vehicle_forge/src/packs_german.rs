//! Reference packs for the migrated German line (Tiger I/II, Jagdtiger, Panther II).
//!
//! These share the five-ratio silhouette gate from [`silhouette_ratios`]; the targets are tuned to
//! each vehicle's baked geometry and documented per family. Split from [`crate::packs`] to keep each
//! pack module small and reviewable.

use game_core::VehicleKind;

use crate::{
    DimensionKind, DimensionTarget, RatioKind, RatioTarget, ReferencePack, ReferenceSource,
};

/// Five-ratio silhouette gate builder: targets AND tolerances are per vehicle, so each pack
/// owns how tightly its proportions are held (the old shared tolerances — up to 0.25 on
/// turret height — caught almost nothing). W1 dossier PRs tighten these vehicle by vehicle.
/// Shared with the Soviet heavy pack (`packs_is3`) — one shape of gate, per-vehicle numbers.
pub(crate) fn silhouette_ratios(
    hull_len_to_width: (f32, f32),
    hull_height_to_len: (f32, f32),
    turret_width: (f32, f32),
    turret_height: (f32, f32),
    gun_protrusion: (f32, f32),
    notes: [&str; 5],
) -> Vec<RatioTarget> {
    vec![
        RatioTarget::new(
            RatioKind::HullLengthToWidth,
            hull_len_to_width.0,
            hull_len_to_width.1,
            notes[0],
        ),
        RatioTarget::new(
            RatioKind::HullHeightToLength,
            hull_height_to_len.0,
            hull_height_to_len.1,
            notes[1],
        ),
        RatioTarget::new(
            RatioKind::TurretWidthToHullWidth,
            turret_width.0,
            turret_width.1,
            notes[2],
        ),
        RatioTarget::new(
            RatioKind::TurretHeightToHullHeight,
            turret_height.0,
            turret_height.1,
            notes[3],
        ),
        RatioTarget::new(
            RatioKind::GunProtrusionToHullLength,
            gun_protrusion.0,
            gun_protrusion.1,
            notes[4],
        ),
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
        // W1 dossier (2026-07-17): tolerances tightened around the verified body — every
        // measured ratio sits within 2.3% of target, so these hold LOD headroom (~3x delta)
        // while actually catching a proportion drifting into the wrong tank.
        {
            let mut ratios = silhouette_ratios(
                (1.66, 0.08),
                (0.33, 0.02),
                (0.52, 0.05),
                (0.58, 0.05),
                (0.34, 0.03),
                [
                    "Hull plan reads as a long heavy, not a medium.",
                    "Tall, slab-sided heavy hull — the deepest body in the German line.",
                    "Horseshoe turret is broad but narrower than the full sponson beam.",
                    "The turret is a low broad band on the tall superstructure, cupola on the left.",
                    "8.8 cm KwK 36 projects well past the flat nose.",
                ],
            );
            ratios.push(RatioTarget::new(
                RatioKind::TurretLengthToWidth,
                1.25,
                0.08,
                "Horseshoe plus the Rommelkiste: the turret plan is a long band (2.50 over 2.00).",
            ));
            ratios.push(RatioTarget::new(
                RatioKind::RoadWheelDiameterToHullLength,
                0.127,
                0.008,
                "800 mm interleaved wheels on the 6.32 m hull dominate the flank.",
            ));
            ratios
        },
    )
    // Dossier anchors (W1 + the 2026-08-06 research pass). The variant is pinned to a LATE
    // Ausf. E — post-February 1944, Fgst.Nr. 250822 and up — and every anchor below is read
    // against THAT tank; the source conflicts and their resolutions live in
    // docs/vehicles/panzerkampfwagen-vi-tiger.md.
    .with_dimensions(vec![
        // -- Locked: the model already honours these documented numbers. --
        DimensionTarget::new(
            DimensionKind::HullLength,
            6.316,
            0.08,
            tiger_i_dossier("6.316 m hull (Wikipedia + Panzerworld agree)."),
        ),
        DimensionTarget::new(
            DimensionKind::HullWidth,
            3.705,
            0.08,
            tiger_i_dossier(
                "3.705 m over the 725 mm combat tracks (German records; 3.56 m = sponsons; \
                 Tank Museum's 3.72 recorded as a conflict, inside build tolerance).",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::HeightToTurretRoof,
            3.00,
            0.05,
            tiger_i_dossier("3.00 m silhouette apex at the drum cupola."),
        ),
        DimensionTarget::new(
            DimensionKind::HeightToTurretRoofBare,
            2.885,
            0.05,
            tiger_i_dossier(
                "2.885 m to the bare turret roof (German records; tiger1.info's 2625 mm \
                 'total height' is an unresolved conflict, triangulated against but not closed).",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::OverallLengthWithGun,
            8.45,
            0.10,
            tiger_i_dossier("8.450 m gun forward."),
        ),
        DimensionTarget::new(
            DimensionKind::RoadWheelDiameter,
            0.80,
            0.01,
            tiger_i_dossier(
                "800 mm road wheels — the Tank Museum records the diameter as UNCHANGED across \
                 the rubber-tyred to steel-rimmed swap; only count and material moved.",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::TurretRingDiameter,
            1.836,
            0.015,
            tiger_i_dossier(
                "1836 mm ring in the clear (tiger1.info, factory-drawing derived). NOT the \
                 1500 mm some pages quote — that is Krupp's 1937 design spec, not the built tank.",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::TrackWidth,
            0.725,
            0.005,
            tiger_i_dossier(
                "725 mm Kgs 63/725/130 combat track (three sources); the 520 mm Kgs 63/520/130 \
                 transport belt is the OTHER configuration and is not what the game models.",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::GroundClearance,
            0.47,
            0.01,
            tiger_i_dossier("470 mm documented clearance."),
        ),
        // -- Target: documented values the model has NOT reached. Data first, geometry second;
        //    each flips to Locked in the PR that closes it. --
        DimensionTarget::target_pending(
            DimensionKind::FireLineHeight,
            2.195,
            0.02,
            tiger_i_dossier(
                "2195 mm bore/trunnion axis at gun level (Panzerworld AND Alan Hamby — a second \
                 independent source, so the model's 2.17 is a real 25 mm debt, not a soft number).",
            ),
        ),
        DimensionTarget::target_pending(
            DimensionKind::RoadWheelCount,
            16.0,
            0.4,
            tiger_i_dossier(
                "16 road wheels per side on 8 torsion-bar stations (2 per arm) after the \
                 February 1944 steel-rimmed change at Fgst.Nr. 250822 — the early tank carried \
                 24 on 3 rows. The model draws ONE wheel per authored axle, so the fleet's \
                 signature Schachtellaufwerk runs at half its wheel count.",
            ),
        ),
        DimensionTarget::target_pending(
            DimensionKind::TrackLinkCountPerSide,
            96.0,
            0.5,
            tiger_i_dossier(
                "96 links per side at the 130 mm pitch the Kgs 63/725/130 designation itself \
                 states (two independent sources agree on the count).",
            ),
        ),
    ])
}

/// Every Tiger I anchor cites the in-repo dossier, whose Reference anatomy table carries the
/// number, its external sources, its confidence grade, and — where two records disagree — the
/// resolution and the reasoning behind it.
fn tiger_i_dossier(note: &str) -> ReferenceSource {
    ReferenceSource::new(
        "Tiger I dossier (Reference anatomy)",
        "docs/vehicles/panzerkampfwagen-vi-tiger.md",
        note,
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
        // W1 dossier PR-T2.1 (2026-07-17): tolerances tightened around the measured body —
        // same ~3x-delta headroom rule as the Tiger I pack, so a proportion drifting into the
        // wrong tank actually fails. PR-T2.2 re-measured with the Schuerzen fitted: the hull
        // beam ratios now describe the SKIRTED envelope (1.906 / 0.506 via Studio).
        {
            let mut ratios = silhouette_ratios(
                (1.91, 0.06),
                (0.28, 0.02),
                (0.51, 0.05),
                (0.62, 0.05),
                (0.39, 0.03),
                [
                    "Very long sloped heavy hull.",
                    "Long sloped hull — flatter than the upright Tiger I.",
                    "Sloped Henschel turret is narrower than the broad hull.",
                    "Low faceted turret with a rear bustle rides the wide deck.",
                    "Long 8.8 cm KwK 43 reaches decisively past the sloped glacis.",
                ],
            );
            ratios.push(RatioTarget::new(
                RatioKind::TurretLengthToWidth,
                1.58,
                0.10,
                "Serienturm is LONG: 3.10 m plan over the 1.96 m beam, bustle included.",
            ));
            ratios.push(RatioTarget::new(
                RatioKind::RoadWheelDiameterToHullLength,
                0.108,
                0.008,
                "800 mm overlapped wheels on the 7.38 m hull.",
            ));
            ratios
        },
    )
    // W1 dossier anchors (PR-T2.1) — sources and the Serienturm front-plate resolution live in
    // docs/vehicles/panzerkampfwagen-vi-b-tiger-ii.md.
    .with_dimensions(vec![
        DimensionTarget::new(
            DimensionKind::HullLength,
            7.38,
            0.08,
            ReferenceSource::new(
                "Tiger II dossier",
                "docs/vehicles/panzerkampfwagen-vi-b-tiger-ii.md",
                "7.38 m hull (Panzerworld/OnWar agree).",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::HullWidth,
            3.88,
            0.08,
            ReferenceSource::new(
                "Tiger II dossier",
                "docs/vehicles/panzerkampfwagen-vi-b-tiger-ii.md",
                "3.88 m over the fitted Schuerzen (3.755 m bare tracks; 3.27 m = transport tracks).",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::HeightToTurretRoof,
            3.09,
            0.05,
            ReferenceSource::new(
                "Tiger II dossier",
                "docs/vehicles/panzerkampfwagen-vi-b-tiger-ii.md",
                "3.09 m silhouette apex at the cupola.",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::OverallLengthWithGun,
            10.286,
            0.10,
            ReferenceSource::new(
                "Tiger II dossier",
                "docs/vehicles/panzerkampfwagen-vi-b-tiger-ii.md",
                "10.286 m gun forward (KwK 43 L/71).",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::RoadWheelDiameter,
            0.80,
            0.01,
            ReferenceSource::new(
                "Tiger II dossier",
                "docs/vehicles/panzerkampfwagen-vi-b-tiger-ii.md",
                "800 mm overlapped (not interleaved) steel-rimmed wheels.",
            ),
        ),
    ])
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
        // W1 dossier PR-JT.1 (2026-07-18): tolerances tightened around the measured body
        // (2.167 / 0.264 / 0.753 / 0.530 / 0.365 via Studio) — same ~3x-delta headroom rule
        // as the Tiger I/II packs.
        {
            let mut ratios = silhouette_ratios(
                (2.17, 0.06),
                (0.26, 0.02),
                (0.75, 0.05),
                (0.53, 0.04),
                (0.365, 0.03),
                [
                    "The longest hull in the German line, stretched from the Tiger II.",
                    "The unbroken flank makes the hull band deep; the casemate is welded INTO it.",
                    "Wide fixed casemate fills most of the hull width.",
                    "A low wide fighting compartment crowns the deep hull, no cupola.",
                    "12.8 cm Pak reaches far past the nose.",
                ],
            );
            ratios.push(RatioTarget::new(
                RatioKind::TurretLengthToWidth,
                1.18,
                0.08,
                "The casemate plan is longer than wide (3.20 over 2.71, rear overhang included).",
            ));
            ratios.push(RatioTarget::new(
                RatioKind::RoadWheelDiameterToHullLength,
                0.103,
                0.008,
                "800 mm overlapped wheels on the German line's longest 7.80 m hull.",
            ));
            ratios
        },
    )
    // W1 dossier anchors (PR-JT.1) — sources and the PaK 44 muzzle decision live in
    // docs/vehicles/jagdtiger.md.
    .with_dimensions(vec![
        DimensionTarget::new(
            DimensionKind::HullLength,
            7.80,
            0.08,
            ReferenceSource::new(
                "Jagdtiger dossier",
                "docs/vehicles/jagdtiger.md",
                "7.80 m hull — the Tiger II chassis lengthened for the casemate.",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::HullWidth,
            3.625,
            0.08,
            ReferenceSource::new(
                "Jagdtiger dossier",
                "docs/vehicles/jagdtiger.md",
                "3.625 m over the 800 mm combat tracks.",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::HeightToTurretRoof,
            2.945,
            0.05,
            ReferenceSource::new(
                "Jagdtiger dossier",
                "docs/vehicles/jagdtiger.md",
                "2.945 m to the casemate roof.",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::OverallLengthWithGun,
            10.654,
            0.10,
            ReferenceSource::new(
                "Jagdtiger dossier",
                "docs/vehicles/jagdtiger.md",
                "10.654 m gun forward (12.8 cm PaK 44 L/55).",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::RoadWheelDiameter,
            0.80,
            0.01,
            ReferenceSource::new(
                "Jagdtiger dossier",
                "docs/vehicles/jagdtiger.md",
                "800 mm overlapped steel-rimmed wheels (production Henschel suspension).",
            ),
        ),
    ])
}

pub fn panther_ii_reference_pack() -> ReferencePack {
    ReferencePack::new(
        "panther_ii",
        "Panther II",
        vec![VehicleKind::PantherII],
        "Armored Vehicle Forge reference for the Panther II as the Fort Benning museum specimen:          the up-armoured Panther II hull on Tiger II-commonality steel wheels, carrying the          Panther Ausf. G turret with the 7.5 cm KwK 42 L/70.",
        7,
        heavy_sources(
            "https://commons.wikimedia.org/wiki/Category:Panther_II",
            "https://tank-afv.com/ww2/germany/Panther-II.php",
            "docs/vehicles/panzerkampfwagen-v-panther-ii.md",
        ),
        // W1 dossier PR-PII.1 (2026-07-18): HULL rows tightened around the measured 1:1 body.
        // PR-PII.2 landed the G turret ON the goal targets (0.600 / 1.127 / 0.290 measured)
        // and the tolerances closed to their final values — a wedge drifting back fails here.
        {
            let mut ratios = silhouette_ratios(
                (2.02, 0.06),
                (0.30, 0.02),
                (0.60, 0.05),
                (0.58, 0.05),
                (0.29, 0.03),
                [
                    "Long sloped Panther hull, longest-legged medium of the German line.",
                    "The Panther silhouette is low for its length.",
                    "G turret target: wider rounded plan than the wedge (temp tolerance).",
                    "G turret carries Panther proportions on the deck.",
                    "KwK 42 L/70 reach at the documented 8.86 m overall (temp tolerance).",
                ],
            );
            ratios.push(RatioTarget::new(
                RatioKind::TurretLengthToWidth,
                1.15,
                0.08,
                "G turret plan: 2.30 over the 2.04 beam, bustle included.",
            ));
            ratios.push(RatioTarget::new(
                RatioKind::RoadWheelDiameterToHullLength,
                0.116,
                0.008,
                "800 mm Tiger II-commonality steel wheels on the 6.87 m hull.",
            ));
            ratios
        },
    )
    // W1 dossier anchors (PR-PII.1) — sources and the museum-specimen configuration live in
    // docs/vehicles/panzerkampfwagen-v-panther-ii.md.
    .with_dimensions(vec![
        DimensionTarget::new(
            DimensionKind::HullLength,
            6.87,
            0.08,
            ReferenceSource::new(
                "Panther II dossier",
                "docs/vehicles/panzerkampfwagen-v-panther-ii.md",
                "6.87 m hull (Panther-dimensioned prototype hull, thicker plates).",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::HullWidth,
            3.42,
            0.10,
            ReferenceSource::new(
                "Panther II dossier",
                "docs/vehicles/panzerkampfwagen-v-panther-ii.md",
                "3.42 m over the 660 mm Tiger II-commonality tracks (Spielberger).",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::HeightToTurretRoof,
            2.99,
            0.06,
            ReferenceSource::new(
                "Panther II dossier",
                "docs/vehicles/panzerkampfwagen-v-panther-ii.md",
                "2.99 m to the G-turret cupola on the specimen.",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::OverallLengthWithGun,
            8.86,
            0.10,
            ReferenceSource::new(
                "Panther II dossier",
                "docs/vehicles/panzerkampfwagen-v-panther-ii.md",
                "8.86 m gun forward (7.5 cm KwK 42 L/70, G-turret specimen).",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::RoadWheelDiameter,
            0.80,
            0.01,
            ReferenceSource::new(
                "Panther II dossier",
                "docs/vehicles/panzerkampfwagen-v-panther-ii.md",
                "800 mm steel-rimmed wheels (Tiger II commonality programme).",
            ),
        ),
    ])
}
