//! Reference pack for the KV-1 mod. 1942. Targets computed from the blueprint's documented
//! anatomy (6.75 m hull, 3.32 m beam, 1.75 m deck, cast loaf to 2.71 m, ZiS-5 muzzle at z = 3.58).
//! Two of the five ratios are this vehicle's anti-clone locks: the turret is NARROW for a heavy
//! and the gun barely overhangs at all.

use game_core::VehicleKind;

use crate::packs_german::silhouette_ratios;
use crate::{ReferencePack, ReferenceSource};

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
        // TODO(kv1): zaciśnij tolerancje po PR 3 (fendery i deck zmienią mierzone bryły).
        silhouette_ratios(
            (2.03, 0.20),
            (0.26, 0.07),
            (0.59, 0.14),
            (0.55, 0.25),
            (0.07, 0.06),
            [
                "A heavy's plan: 6.75 m hull over a 3.32 m beam.",
                "A TALL hull for its length — the 1.75 m deck is over half the silhouette.",
                "The loaf is NARROW for a heavy: 1.96 m of turret on a 3.32 m beam.",
                "The casting carries a third of the silhouette above an already tall hull.",
                "The ZiS-5 barely overhangs: a fifth of the next-shortest gun in the fleet.",
            ],
        ),
    )
}
