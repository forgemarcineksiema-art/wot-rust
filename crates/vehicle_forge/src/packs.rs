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
                RatioKind::TurretWidthToHullWidth,
                0.57,
                0.14,
                "Rounded cast turret should be broad but clearly narrower than the track span.",
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
