//! Reference pack for the T-34-85 — the first vehicle authored through the Forge Studio loop.
//! Targets computed from the blueprint's documented anatomy (6.10 m hull, 3.00 m beam, 1.56 m
//! hull roof, cast dome to 2.46 m, ZiS-S-53 muzzle at z = 5.05).

use game_core::VehicleKind;

use crate::packs_german::silhouette_ratios;
use crate::{ReferencePack, ReferenceSource};

pub fn t34_85_reference_pack() -> ReferencePack {
    ReferencePack::new(
        "t34_85",
        "T-34-85",
        vec![VehicleKind::T34_85],
        "Armored Vehicle Forge reference for the T-34-85: slope as armour — a 60-degree glacis \
         and raked sides overhanging the tracks, five large bare Christie wheels with an open \
         gap behind the first station and no return rollers, and the wide low three-man cast \
         dome of the 85 refit seated forward.",
        // Five big 830 mm discs per side — the blueprint's wheel_count.
        5,
        vec![
            ReferenceSource::new(
                "Wikimedia Commons gallery",
                "https://commons.wikimedia.org/wiki/Category:T-34-85",
                "Photo reference for the sloped hull, the Christie gear gap, and the cast dome.",
            ),
            ReferenceSource::new(
                "Tank AFV reference article",
                "https://tank-afv.com/ww2/soviet/soviet_T34-85.php",
                "Technical and visual reference for dimensions, armament, and family cues.",
            ),
        ],
        // Tolerances tightened 2026-08-23 around the measured bake (Studio report: deltas
        // 0.007 / 0.032 / 0.030 / 0.024 / 0.002) — ~3x the measured delta with a 0.03 floor,
        // so they hold LOD headroom while catching a proportion drifting into the wrong tank.
        // The one exception: hull-height/length measures 0.302 against the 0.270 target (the
        // dossier is still the self-labelled unresearched stub), so its band is a ratchet
        // around today's bake, not an endorsement — reconciling target vs body belongs to the
        // dossier pass.
        silhouette_ratios(
            (2.05, 0.03),
            (0.27, 0.05),
            (0.74, 0.09),
            (0.55, 0.08),
            (0.33, 0.03),
            [
                "A medium's plan: 6.10 m hull over a full 3.00 m beam.",
                "Low hull for its length — the 1.56 m roof under the tall dome.",
                "The widened 85 turret spans most of the beam.",
                "The dome carries a third of the silhouette above the low hull.",
                "The 85 overhangs a modest 2 m — a medium's gun, not a heavy's.",
            ],
        ),
    )
}
