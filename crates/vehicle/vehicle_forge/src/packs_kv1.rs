//! Reference pack for the KV-1 mod. 1942. Targets computed from the blueprint's documented
//! anatomy (6.75 m hull, 3.32 m beam, 1.75 m deck, cast loaf to 2.71 m, ZiS-5 muzzle at z = 3.58).
//! Two of the five ratios are this vehicle's anti-clone locks: the turret is NARROW for a heavy
//! and the gun barely overhangs at all.

use game_core::VehicleKind;

use crate::packs_german::silhouette_ratios;
use crate::{DimensionKind, DimensionTarget, ReferencePack, ReferenceSource};

fn dossier(note: &str) -> ReferenceSource {
    ReferenceSource::new("KV-1 dossier", "docs/vehicles/kv-1.md", note)
}

pub fn kv1_reference_pack() -> ReferencePack {
    ReferencePack::new(
        "kv1_1942",
        "KV-1 obr. 1942",
        vec![VehicleKind::KV1_1942],
        "Armored Vehicle Forge reference for the KV-1 model 1942: mass without slope — a shallow \
         stepped bow over dead-vertical 75 mm sides and a flat deck, six small 600 mm wheels at an \
         even pitch on torsion bars with their top run carried taut on three return rollers, and a \
         100 mm cast turret that is a long slab-sided loaf with a flat roof and the rear DT ball, \
         not a dome. The short 76 mm ZiS-5 barely clears the bow.",
        // Six 600 mm discs per side — the blueprint's wheel_count, cross-checked by the part graph.
        6,
        vec![
            ReferenceSource::new(
                "Wikipedia — Kliment Voroshilov tank",
                "https://en.wikipedia.org/wiki/Kliment_Voroshilov_tank",
                "Dimensions (6.75 x 3.32 x 2.71 m), crew, 90/75/70 mm hull armour, V-2K 600 hp, \
                 torsion bar suspension, and the model-by-model variant breakdown.",
            ),
            ReferenceSource::new(
                "o5m6.de — KV-1 Model 1942 Heavy Tank",
                "https://www.o5m6.de/redarmy/kv1_1942.php",
                "The mod-1942 turret's identity: the reinforced casting with selective 110-120 mm \
                 armour and the armoured collar around the rear machine-gun mount. Not a \
                 dimensions source — it carries no specification table.",
            ),
            ReferenceSource::new(
                "GlobalSecurity — KV-1 Heavy Tank, Design",
                "https://www.globalsecurity.org/military/world/russia/kv-1-design.htm",
                "The running gear: six road wheels and three return rollers per side, rear drive \
                 sprocket, front idler, torsion bars with individual shock absorption, and 88 \
                 track links of 700 mm width per side.",
            ),
        ],
        // Tolerances tightened onto the measured bake (`tools forge-report --vehicle kv1-1942`)
        // now that the dressing, fenders and cast mask are in and the shape has stopped moving.
        // The turret-height target was the one real miss — 0.55 estimated against 0.681 measured,
        // because the ratio takes the baked hull bounds and not `deck_y`.
        silhouette_ratios(
            (2.033, 0.040),
            (0.249, 0.030),
            (0.590, 0.050),
            (0.681, 0.060),
            (0.066, 0.020),
            [
                "A heavy's plan: 6.75 m hull over a 3.32 m beam.",
                "A LONG hull for its height — the KV is mass spread out, not stacked up.",
                "The loaf is NARROW for a heavy: 1.96 m of turret on a 3.32 m beam. Anti-clone \
                 lock — every other heavy in the fleet fills far more of its beam.",
                "The casting carries two thirds of the hull's own height above it.",
                "The ZiS-5 barely overhangs. Anti-clone lock, and the tightest band in the pack: \
                 the T-34-85 sits at 0.33 and the IS-3 at 0.45, five to seven times this.",
            ],
        ),
    )
    // Only the dossier's HIGH-confidence rows anchor here. The road wheel diameter and the
    // turret plan are photo-derived estimates and are deliberately left out until a drawing or a
    // measured specimen settles them; `OverallLengthWithGun` is out because the modelled muzzle
    // deliberately deviates (see the dossier's deviation 2), and anchoring a deviation would
    // dress it up as a reference.
    .with_dimensions(vec![
        DimensionTarget::new(
            DimensionKind::HullLength,
            6.75,
            0.08,
            dossier("6.75 m hull over the tracks (Wikipedia specification table)."),
        ),
        DimensionTarget::new(
            DimensionKind::HullWidth,
            3.32,
            0.08,
            dossier("3.32 m over the 700 mm tracks (Wikipedia specification table)."),
        ),
        DimensionTarget::new(
            DimensionKind::HeightToTurretRoof,
            2.71,
            0.08,
            dossier("2.71 m OVERALL height: the 2.62 m roof plus the periscope standing on it."),
        ),
    ])
}
