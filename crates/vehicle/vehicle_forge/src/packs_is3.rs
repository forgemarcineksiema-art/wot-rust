//! Reference pack for the IS-3 — the Soviet heavy line's first entry. Shares the five-ratio
//! silhouette gate with the migrated German line; the targets are computed from the IS-3
//! blueprint's documented anatomy (6.77 m hull, 3.15 m over the tracks, 1.62 m hull roof,
//! flattened dome to 2.39 m, D-25T muzzle at z = 6.46).

use game_core::VehicleKind;

use crate::packs_german::silhouette_ratios;
use crate::{ReferencePack, ReferenceSource};

pub fn is3_reference_pack() -> ReferencePack {
    ReferencePack::new(
        "is3",
        "IS-3",
        vec![VehicleKind::IS3],
        "Armored Vehicle Forge reference for the IS-3: the pike-nose heavy — two swept bow \
         plates meeting at a central ridge, a wide flattened cast dome overhanging its ring, six \
         small road wheels per side, and the braked 122 mm D-25T.",
        8,
        vec![
            ReferenceSource::new(
                "Wikimedia Commons gallery",
                "https://commons.wikimedia.org/wiki/Category:IS-3",
                "Photo reference for the pike bow, the flattened dome, and the running gear.",
            ),
            ReferenceSource::new(
                "Tank AFV reference article",
                "https://tank-afv.com/coldwar/USSR/IS-3M.php",
                "Technical and visual reference for dimensions, armament, and family cues.",
            ),
            ReferenceSource::new(
                "In-repo vehicle notes",
                "docs/vehicles/is-3.md",
                "Gameplay translation and source summary.",
            ),
        ],
        silhouette_ratios(
            2.15,
            0.239,
            0.73,
            0.48,
            0.45,
            [
                "Hull plan reads long and wide — a heavy, but lower-slung than the Germans.",
                "The LOW heavy: hull roof at 1.62 m on a 6.77 m hull.",
                "The wide dome overhangs its ring, most of the hull's width.",
                "A flattened frying-pan dome, not a tall casting.",
                "The 122 mm D-25T overhangs almost half the hull again.",
            ],
        ),
    )
}
