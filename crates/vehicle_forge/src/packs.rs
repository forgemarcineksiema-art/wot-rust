//! Concrete `ReferencePack` data, one constructor per benchmarked vehicle family.
//!
//! The generic reference types live in [`crate::reference`]; this module is the photo-backed data
//! that proves where a vehicle's proportions come from. The T-54-3 obr. 1951 is the first Forge
//! quality benchmark (see `docs/vehicle-forge-policy.md`).

use game_core::VehicleKind;

use crate::{RatioKind, RatioTarget, ReferencePack, ReferenceSource};

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
                0.50,
                0.04,
                "T-54-3 carries a flattened pancake casting, not a high dome or a casemate.",
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
