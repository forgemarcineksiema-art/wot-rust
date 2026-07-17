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
        // Targets are the documented 1:1 dimensions: hull 6.04 long x 3.27 wide x 1.75 to the
        // roof, ~2.25 m casting 0.66 tall with the cupola, and the D-10T's 2.96 m bow overhang
        // (9.00 m with gun forward).
        vec![
            RatioTarget::new(
                RatioKind::HullLengthToWidth,
                1.85,
                0.15,
                "Overall hull plan (6.04 / 3.27) should read as a compact Soviet medium, not a long heavy.",
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
                0.48,
                0.05,
                "The tall ~0.7 m hemispherical casting (plus cupola) rides the LOW 1.58 hull roof.",
            ),
            RatioTarget::new(
                RatioKind::GunProtrusionToHullLength,
                0.49,
                0.08,
                "The D-10T projects 2.96 m past the bow (9.00 m overall) — decisive but not a heavy-tank gun.",
            ),
            // The PR-04 ratio family — the pilot authors all three so the fleet packs have a
            // worked example to copy in their W1/W2 dossier PRs.
            RatioTarget::new(
                RatioKind::TurretLengthToWidth,
                0.95,
                0.06,
                "The cast dome plan is nearly round, a touch wider than long (2.23 long over                  2.31 wide as baked — the mantlet cheeks carry the width).",
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
                "810 mm starfish wheels on the 6.04 m hull read heavy on the side view.",
            ),
        ],
    )
    // Absolute anchors (metres): the pilot of the dimension gate. Ratios pass at any scale;
    // these pin the model to the documented tape measure. Tolerances cover the current
    // authoritative hybrid (fender stowage widens the plan slightly past the bare hull).
    .with_dimensions(vec![
        DimensionTarget::new(
            DimensionKind::HullLength,
            6.04,
            0.15,
            ReferenceSource::new(
                "Project T-54 vehicle notes",
                "docs/vehicles/t-54.md",
                "Documented 6.04 m hull; the baked mesh adds fender line and rear stowage.",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::HullWidth,
            3.27,
            0.10,
            ReferenceSource::new(
                "Project T-54 vehicle notes",
                "docs/vehicles/t-54.md",
                "3.27 m over the combat tracks.",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::HeightToTurretRoof,
            2.40,
            0.05,
            ReferenceSource::new(
                "Project T-54 vehicle notes",
                "docs/vehicles/t-54.md",
                "2.40 m silhouette apex (cupola lid inside the hitbox top).",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::OverallLengthWithGun,
            9.00,
            0.15,
            ReferenceSource::new(
                "Project T-54 vehicle notes",
                "docs/vehicles/t-54.md",
                "9.00 m overall with the D-10T forward.",
            ),
        ),
        DimensionTarget::new(
            DimensionKind::RoadWheelDiameter,
            0.81,
            0.01,
            ReferenceSource::new(
                "Project T-54 vehicle notes",
                "docs/vehicles/t-54.md",
                "810 mm starfish road wheels.",
            ),
        ),
    ])
}
