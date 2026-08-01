# Tank anatomy — what exists, and what to build next

A design proposal was raised on 2026-08-01: the tank as a modular structure rather than "a box with
a health bar" — armour layers, internal modules, external hardware, a five-role crew, loadout and
consumables. This document records what of that **already exists in the code**, what genuinely
does not, three objections, and the order the remaining work should happen in.

## Already built — verified in source

The module system is **two-tier**, and richer than the proposal assumed.

| layer | what exists |
|---|---|
| **Gameplay slots** (`ModuleSlot`) | Engine · Suspension · Turret · Gun · AmmoRack · Radio |
| **Interior components** (`DamageComponentKind`) | Breech · RecoilMechanism · TurretDrive · AmmunitionRack · Radio · **FuelTank** · Engine · **Transmission** · **FinalDrive** · Suspension |
| **Armour zones** (`ArmorZone`, 13) | UpperGlacis · LowerPlate · HullSide · HullRear · HullDeck · Roof · Mantlet · TurretFront/Side/Rear · Skirt · LeftTrack · RightTrack |

- **Sloped armour** resolves against the plate's true 3D normal, not a nominal number.
- **Spaced armour is done well**: an ordered screen stack, the skirt resolved **outside** the track
  belt, HEAT paying ×2.0 for standoff, overmatch at 3× calibre.
- **The "jack-in-the-box" is complete**: an ammo-rack detonation flings the turret along a ballistic
  arc and settles it beside the hull, replicated (protocol v20, `detached_turrets`).
- **Tracks are three-stage** (Healthy → Damaged → Broken) over a belt HP pool.
- **A damaged gun** multiplies reload (1.0 → 1.6) and raises dispersion.
- **Field repair** exists: the crew patches a module after 15 s to 25 % of its pool.
- **AP / APCR / HEAT / HE** all exist, with HEAT flat over range and killed by spaced screens.

## Genuinely missing — three things

1. **Optics / vision devices as a module.** The one internal module with no representation at all,
   and the one that would change tactical play most, because spotting today is pure line-of-sight
   plus range with no concealment layer.
2. **Crew as anything but one number.** `CrewRole` lists five roles but is decorative: `Crew` is a
   single `proficiency: f32` scaling reload and aim time by at most 30 %. No crew member is
   individually hit or knocked out.
3. **Fuel fire never reaches the screen.** The simulation sets `fuel_fire` when a `FuelTank`
   component is struck (`sim/combat.rs:276`), and `engine/src/world.rs:205` then hardcodes `false`
   on the production path. **One line blocks an entire damage state that is otherwise finished.**

---

## Objection 1 — two sections belong to a different game

ERA, composite/ceramic armour, soft- and hard-kill APS, ATGM and APFSDS are main-battle-tank systems
from roughly 1960–2000. This game is **three eras ending at the early Cold War** — T-54, IS-3,
Centurion Mk 3. ERA on a T-54 is twenty years out of period.

And an ATGM is not "another shell type": a guided missile removes the premium on shooting from a
halt and on range estimation, which is most of the positional game. That is a second product, not
extra depth. File it under a hypothetical Era IV, deliberately.

## Objection 2 — the crew table contradicts the project's own doctrine

The proposed table is World of Tanks' crew system: the commander grants "a bonus to the rest of the
crew's stats", the gunner affects "aim time, accuracy", the loader doubles reload. That is a **stack
of hidden multipliers** — precisely what this project rejected when it committed to no ±25 % RNG and
honest numbers. A player cannot reason about a shot whose dispersion is the product of four
proficiency coefficients.

**A crew loss should remove a CAPABILITY, not scale a statistic:**

- **Loader down** → you cannot switch shell type for the rest of the battle. A *decision* removed.
- **Gunner down** → the commander takes over: a stated, visible, worse aim time — not a coefficient.
- **Driver down** → the tank runs in one gear. Legible, and playable around.
- **Radio operator down** → you stop sharing spotting. Already binary, already honest.

"I lost the ability to X" is readable. "My gunner is at 73 %" is not.

## Objection 3 — more modules means less agency, not more depth

Measured in a real battle: **16 of 24 damage events destroyed or damaged a module**, mostly the
engine — *before* adding optics, transmission, fuel tanks and five separately killable crew. Add
them all and every hit takes something away, which is exactly the least-loved experience in the
genre: permanently degraded, with no counterplay.

What makes a module satisfying is not **count**. It is three properties:

1. **I know what broke.** (The module panel does this.)
2. **I know why.** (Hit position and perforations do this.)
3. **I can do something about it.** (The 15-second field patch does this.)

The proof of where the bottleneck actually is: the report *"sometimes I can't fire even though I'm
loaded"* turned out to be a gun destroyed thirty seconds earlier with no persistent indicator — a
**legibility** failure on a module that already existed. Verify the six current modules satisfy all
three properties before adding a seventh.

## Objection 4 — the armour layer does not currently function

In a full 7v7 played on 2026-08-01: **53 hits, 53 penetrations, zero ricochets**
(`playtest-2026-08-01.md`). Angling, hull-down and aim-point selection are dead mechanics right now.

Adding ERA on top of an armour model that never deflects anything is a second roof on a house with
no walls.

---

## Recommended order

| # | work | why here |
|---|---|---|
| 1 | Diagnose the zero-ricochet result | without it the entire armour section is inert |
| 2 | Let fuel fire reach the screen | one line; the rest is already built |
| 3 | Optics / vision as a module | the only genuinely missing internal module, and the one that most changes tactical play given spotting is pure LOS |
| 4 | Crew as capability loss | after deciding it is *not* stat multipliers |
| 5 | HESH | fits the era, adds a real third behaviour |
| 6 | ERA · composite · APS · ATGM · APFSDS | Era IV backlog, deliberately deferred |

## The principle underneath

What separates a good armoured game from a simulator is not the number of parts. It is **the number
of decisions the player makes in the second before firing**.

- *"Do I expose my flank to get an angle on his?"* — needs a **working ricochet**, not ERA.
- *"Track him, or go for the hull?"* — needs **legible modules**, not five more of them.
- *"Peek now, or wait?"* — needs **terrain with somewhere to hide**, and the cell is 5 m today.

All three are on the critical path, and none of them needs a new system. **There is more tank here
than the proposal assumes; what is missing is a battlefield on which the parts matter.**
