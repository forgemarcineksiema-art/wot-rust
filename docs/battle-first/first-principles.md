# First principles — questioning the premises

Not *"how do we improve spotting"* but *"what is spotting, physically"*. Every question below is
grounded in something already measured or already built; the cost check at the end separates the
three worth doing now from the ones that are thought experiments.

---

## 1. Why is the terrain a heightmap at all?

A heightmap charges a uniform price everywhere. But the gameplay lives in **a handful of places**: a
ditch that hides a hull, a berm to peek over, a crater to wait in.

Going 5 m → 1.25 m is **16× the samples across the whole map** to buy micro-relief in perhaps five
per cent of the area that matters. You pay for a square kilometre to purchase twenty places.

**And the solution is already built, used for one thing.**

`crater_ledger.rs` writes quantised crater records, replicates them, merges near-duplicates, caps the
list at 64, and **deforms the heightmap overlay bit-identically on the server and every client**.
That is precisely "authored micro-relief on a coarse field". It was built for high-explosive bursts.

Generalise it into an **authoring primitive**: ditches, sunken roads, berms, shell-holed ground,
ruts — records in the same ledger, placed by the map author or generated. The cost then scales with
**the number of places**, not with area.

The terrain stays at 5 m. The places get their 30 cm.

## 2. Why does armour carry a zone table?

`ArmorZone` is 54 match arms across five files — and `VehicleArmorVolumes` is already baked from the
blueprint.

Physically a shell strikes **a surface with a normal and a thickness**. That is all the resolution
needs. The zone exists for the HUD readout and for module mapping — it is a **label**, not a source.

Derive it from the volume that was hit instead of maintaining it alongside. One truth, 54 arms fewer,
and the question "do the zone and the volume still agree?" stops existing.

## 3. What is spotting, physically? *(the important one)*

WoT has a "camouflage value" — an opaque number players have argued about for fifteen years.

Physically, spotting is: **does the observer's optical system resolve the target against its
background?** The inputs are all computable — **angular size** (silhouette over range), contrast with
the background, motion, atmospheric extinction, the optic's magnification.

**Every one of those inputs already exists in this project.** The mesh is baked, its bounds are known,
the silhouette from any angle is derivable, the weather and time of day are authoritative.

> **A smaller tank IS harder to see — because it subtends fewer arcminutes.**

No camouflage stat is needed. Concealment **falls out of the model geometry that is already built.**

Cost: 14 tanks is 196 pairs; at the existing 10 Hz cadence that is ~2 000 evaluations per second.
Against 35 µs per tick, it is noise.

What it buys:

- **light tanks get an identity** without inventing a statistic — they are small, so they are hard to see;
- **angling starts to cut both ways** — presenting a flank presents a larger silhouette;
- **it can be shown to the player**: *"the target subtends 4.2 arcminutes; your threshold is 5.1 — you
  cannot see it"*;
- **honesty becomes provable** in the single most disputed mechanic in the genre.

A competitor cannot copy this without deleting their own camouflage system.

## 4. Why does a tank have hit points?

Every penetration removes the same HP whether it passed through the turret or the engine bay. **This
is the one place in this game that is exactly as arbitrary as the game it means to beat.**

Physically, a tank stops fighting when the crew is down, the ammunition detonates, it burns, or the
engine dies. **Not when a number reaches zero.**

Already present: armour volumes, interior components with geometry, spall directions
(`deterministic_spall_directions`), breach space, engine fire, ammo-rack detonation with the turret
flung clear.

**The logical end of "the honest tank" is deleting HP.**

Stated honestly: this is frightening. Outcome variance rises, balance gets hard, legibility needs
dedicated work. WoT cannot do it because HP is their balance and monetisation lever — **which is
exactly what makes it a differentiator.**

**The middle path, if the full version is too sharp:** HP stays as a proxy for crew fatigue and
structural integrity, but **damage depends on what was hit**. A penetration through an empty corner
of the hull hurts less than one through the fighting compartment. That is one function
(`shell_damage_hp`) and it turns aiming into a decision immediately.

## 5. What is the 400× headroom for?

35 µs against a 16 667 µs budget. Spending **1 ms per tick** — still 6 % of a frame — is **28× more
compute** than the simulation uses today.

| what | cost | what it buys |
|---|---|---|
| **Optics instead of boolean LOS** | negligible | the highest return in the project (see §3) |
| **Damage by compartment hit** | negligible | aiming becomes a decision |
| **Shell as a body with spin** | small | a round at extreme obliquity tumbles instead of flipping a boolean |
| **Suspension in the sim, not in presentation** | medium | terrain starts to affect gunnery and handling |
| **Track ruts that deform terrain** | medium | the battlefield remembers where it was driven |
| Per-crew-member state | medium | only after deciding "capabilities, not multipliers" |

## 6. What is already owned and not used?

**A deterministic simulation with replay fixtures** — used only by tests.

One property yields, for free: **proof of shot** (replay any hit with the plate, the angle, 187 mm
effective against 175 mm of penetration — *that* is why it bounced), killcam, spectator mode,
server-side verification against cheating, and a debugging instrument.

The thesis is "the honest tank". **A competitor cannot answer "ours is too" without removing their own
RNG.** It is the one thing here that is structurally uncopyable.

---

## Cost check

First principles without arithmetic is fantasy.

| idea | verdict |
|---|---|
| **Optics as angular size** | **do it** — the data exists, the cost is nil, it closes the scouting gap |
| **Damage by compartment** | **do it** — one function, immediate depth |
| **Deform ledger as an authoring primitive** | **do it** — the mechanism exists; prototype on one map |
| Armour zone derived from the volume | later — housekeeping, not gameplay |
| Shell with spin | later — good, not critical |
| **Deleting HP** | **thought experiment** — prototype in a branch, not in the plan |

The first three are perhaps a week between them, and **each changes the game more than a fourth era
would.**
