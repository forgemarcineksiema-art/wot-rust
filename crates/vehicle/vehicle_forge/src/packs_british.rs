//! Reference pack for the British line (Centurion Mk 3). Shares the five-ratio silhouette
//! gate with the German/Soviet packs; targets are tuned to the blueprint-born 1:1 body.

use game_core::VehicleKind;

use crate::packs_german::silhouette_ratios;
use crate::{ReferencePack, ReferenceSource};

pub fn centurion_reference_pack() -> ReferencePack {
    ReferencePack::new(
        "centurion_mk3",
        "Centurion Mk 3",
        vec![VehicleKind::Centurion],
        "Armored Vehicle Forge reference for the Centurion Mk 3: full-length skirted hull over \
         Horstmann bogie pairs, the fleet's steepest glacis, a cast turret with the rear \
         stowage bin, and the clean unbraked 20-pounder.",
        6,
        vec![
            ReferenceSource::new(
                "Wikimedia Commons gallery",
                "https://commons.wikimedia.org/wiki/Centurion_tank",
                "Photo reference for the skirt line, bogie pairs, turret casting, and bin.",
            ),
            ReferenceSource::new(
                "Tank AFV reference article",
                "https://tank-afv.com/coldwar/UK/centurion.php",
                "Technical and visual reference for dimensions, armament, and family cues.",
            ),
            ReferenceSource::new(
                "In-repo vehicle notes",
                "docs/vehicles/centurion.md",
                "Gameplay translation and source summary.",
            ),
        ],
        // Tuned to the blueprint-born 1:1 body (7.60 m hull, 3.34 m over the skirts, 2.99 m
        // tall): a long medium-heavy whose flank is one skirted band, with a broad low cast
        // turret and a clean medium gun.
        // TODO(W2-centurion): zaciśnij tolerancje przy dossier (start = stare wspólne).
        silhouette_ratios(
            (2.22, 0.18),
            (0.26, 0.06),
            (0.62, 0.14),
            (0.58, 0.25),
            (0.29, 0.16),
            [
                "A long hull for a medium — longer in plan than the Soviet park.",
                "The skirted flank reads as one deep band from fender to wheel line.",
                "The cast Mk 3 turret is broad on the narrow skirted hull.",
                "A low dome with the bustle bin, not a German tower.",
                "The clean 20-pounder reaches modestly past the steep glacis.",
            ],
        ),
    )
}
