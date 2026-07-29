//! Concrete `ReferencePack` data, one constructor per benchmarked vehicle family.
//!
//! The generic reference types live in [`crate::reference`]; this module is the photo-backed data
//! that proves where a vehicle's proportions come from. The T-54-3 obr. 1951 is the first Forge
//! quality benchmark (see `docs/vehicle-forge-policy.md`).

use game_core::VehicleKind;

use crate::{
    DimensionKind, DimensionTarget, RatioKind, RatioTarget, ReferencePack, ReferenceSource,
};

pub fn t54_reference_pack() -> ReferencePack {
    ReferencePack::new(
        "t54",
        "T-54",
        vec![VehicleKind::T54_1951],
        "Armored Vehicle Forge benchmark for the canonical T-54-3 obr. 1951: low Soviet medium \
         hull, five-road-wheel running gear without return rollers, rounded cast turret, D-10T \
         gun without bore evacuator, and photo-backed silhouette ratios.",
        5,
        vec![
            ReferenceSource::new(
                "Wikimedia Commons T-54/T-55 gallery",
                "https://commons.wikimedia.org/wiki/T-54/T-55",
                "Photo reference for T-54 silhouette, turret mass, running gear, and stowage.",
            ),
            ReferenceSource::new(
                "Tanks Encyclopedia T-54-1 article",
                "https://tanks-encyclopedia.com/coldwar/soviet/t-54-1-1947/",
                "Technical and visual baseline for early T-54 hull, turret, and D-10T cues.",
            ),
            ReferenceSource::new(
                "Project T-54 vehicle notes",
                "docs/vehicles/t-54.md",
                "In-repo gameplay translation and public source summary.",
            ),
        ],
        // Proportion gates (scale-free). The absolute tape-measure truth lives in the
        // dimension anchors below, sourced from the docs/vehicles/t-54.md dossier table.
        vec![
            RatioTarget::new(
                RatioKind::HullLengthToWidth,
                1.85,
                0.15,
                "Overall hull plan (6.235 / 3.27) should read as a compact Soviet medium, not a long heavy.",
            ),
            RatioTarget::new(
                RatioKind::HullHeightToLength,
                0.29,
                0.05,
                "Low Soviet medium silhouette (roof 1.58 + fittings / length 6.04): long and flat.",
            ),
            RatioTarget::new(
                RatioKind::TurretWidthToHullWidth,
                0.68,
                0.10,
                "The ~2.25 m cast turret is broad on the 3.27 m hull but stays inside the track span.",
            ),
            RatioTarget::new(
                RatioKind::TurretHeightToHullHeight,
                // Re-blessed 2026-07-29 (PR-15) onto the documented casting: 0.82 m of turret
                // from the 1.58 ring seat to the 2.40 roof, over the hull it rides. The 0.48 it
                // replaces was the shallow 2.27 dome expressed as a ratio — a target fitted to
                // the model rather than taken from the tank.
                0.55,
                0.06,
                "The 0.82 m casting (ring 1.58 to roof 2.40) rides the LOW 1.58 hull roof.",
            ),
            RatioTarget::new(
                RatioKind::GunProtrusionToHullLength,
                0.49,
                0.08,
                "The D-10T projects 2.73 m past the bow (9.00 m overall) — decisive but not a heavy-tank gun.",
            ),
            // The PR-04 ratio family — the pilot authors all three so the fleet packs have a
            // worked example to copy in their W1/W2 dossier PRs.
            RatioTarget::new(
                RatioKind::TurretLengthToWidth,
                // Blender session S1 measured 1.05 on the real casting and flagged the bake at
                // 0.97 — "the cheeks are too wide for the length". The 0.95 target here was that
                // same wrong bake written down as a goal.
                1.05,
                0.06,
                "The cast dome plan is a touch LONGER than wide: 2.363 m over 2.25 (S1).",
            ),
            RatioTarget::new(
                RatioKind::TurretRingPositionOnHull,
                0.51,
                0.04,
                "The ring sits amidships — the dome rides the hull centre, not a bow turret.",
            ),
            RatioTarget::new(
                RatioKind::RoadWheelDiameterToHullLength,
                0.132,
                0.008,
                "810 mm spider-web wheels on the 6.235 m hull read heavy on the side view.",
            ),
        ],
    )
    // Absolute anchors: the documented tape measure of the obr. 1951, from the dossier table
    // in docs/vehicles/t-54.md (2026-07-28 research, graded sources). `Locked` rows gate;
    // `Target` rows are the Model Idealny program's declared debt — the documented value the
    // model has NOT reached yet, flipped to Locked by the PR that closes the register entry
    // (M-register, docs/model-idealny-t54.md). Anchoring the WRONG (fit-to-model) number to
    // keep a gate green is exactly the failure this file used to have — never again.
    .with_dimensions(vec![
        // -- Locked: the model already honours these documented numbers. --
        DimensionTarget::new(
            DimensionKind::HullWidth,
            3.27,
            0.05,
            t54_dossier("3.27 m over the fenders (high confidence)."),
        ),
        DimensionTarget::new(
            DimensionKind::OverallLengthWithGun,
            9.00,
            0.15,
            t54_dossier("9.00 m overall with the D-10T forward (high confidence)."),
        ),
        DimensionTarget::new(
            DimensionKind::RoadWheelDiameter,
            0.81,
            0.01,
            t54_dossier("810 mm starfish road wheels; measured off the wheel unit mesh."),
        ),
        DimensionTarget::new(
            DimensionKind::TurretRingDiameter,
            1.825,
            0.015,
            t54_dossier("1825 mm ring in the clear (Tankograd); pins the blueprint ring."),
        ),
        DimensionTarget::new(
            DimensionKind::FireLineHeight,
            1.79,
            0.02,
            t54_dossier("1790 mm fire line; measured off the gun-trunnion mount frame."),
        ),
        DimensionTarget::new(
            DimensionKind::TrackLinkCountPerSide,
            90.0,
            0.4,
            t54_dossier("90 OMSh links per side, counted from the rendered placements."),
        ),
        DimensionTarget::new(
            DimensionKind::RoadWheelCount,
            5.0,
            0.4,
            t54_dossier("5 road wheels per side, counted from the rendered placements."),
        ),
        // -- Target: documented values the model has not reached yet (program debt). --
        // The clearance debt was FOUND BY this instrument on its first run: the dossier says
        // 425 mm, the belly bakes at 440 (`belly_y 0.43` + plate). Retunes with the hull PR.
        DimensionTarget::target_pending(
            DimensionKind::GroundClearance,
            0.425,
            0.01,
            t54_dossier("425 mm documented clearance, four sources agreeing."),
        ),
        DimensionTarget::new(
            DimensionKind::HullLength,
            6.235,
            0.05,
            t54_dossier("6.20-6.27 m documented band; the hull is built at the working 6.235."),
        ),
        DimensionTarget::new(
            DimensionKind::HeightToTurretRoofBare,
            2.40,
            0.05,
            t54_dossier("2.40 m to the bare turret roof (book sources); the dome is built there."),
        ),
        DimensionTarget::new(
            DimensionKind::HeightToTurretRoof,
            2.53,
            0.06,
            t54_dossier("~2.53 m apex: roof 2.40 plus the authored 131 mm of exposed cupola."),
        ),
        DimensionTarget::new(
            DimensionKind::CupolaDiameter,
            0.624,
            0.01,
            t54_dossier("624 mm external cupola (Tankograd); the drum is built there."),
        ),
        DimensionTarget::new(
            DimensionKind::TrackWidth,
            0.58,
            0.005,
            t54_dossier("580 mm OMSh belt; the links span it (PR-18)."),
        ),
        DimensionTarget::new(
            DimensionKind::TrackGauge,
            2.64,
            0.02,
            t54_dossier("2640 mm gauge; the belts sit on it (PR-18)."),
        ),
    ])
}

/// Every T-54 anchor cites the in-repo dossier — whose Reference anatomy table carries the
/// number, its external sources, and its confidence grade. The provenance test enforces that
/// the cited number actually appears there.
fn t54_dossier(note: &str) -> ReferenceSource {
    ReferenceSource::new("T-54 dossier (Reference anatomy)", "docs/vehicles/t-54.md", note)
}
