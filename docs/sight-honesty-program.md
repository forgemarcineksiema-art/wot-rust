# Sight Honesty — the program

**The promise.** *If the sight shows you a whole tank, either you can hit it, or the sight tells
you in metres where the shot dies.*

Nothing here is about making the reticle nicer. It is about closing a seam that made the reticle
tell the truth in a way no player could act on.

## Where this came from

Reported from the game on 2026-08-07, with three screenshots: a T-54 at **321–327 m** on Bystra,
plainly visible, no terrain across it, the crosshair on its hull — and the central marker was the
**gray broken BLOCKED form**, not the green penetration verdict. The shell went into the ground.
A metre forward, or a hair of extra elevation, and it turned green.

Every part of that is the sight working exactly as designed:

- gray broken = `ReticleStatus::Blocked` = *this shot does not arrive where you point*
  (`hud/reticle_overlay.rs`, `RETICLE_BLOCKED` — "desaturated gray on purpose"),
- the trace that says so is the authoritative one the server flies,
- the shell really did die in the dirt.

The defect is not in the reticle. It is in the seam **between the eye and the muzzle**, which no
test in the repo looked at, because every reticle test hands the reticle a synthetic heightmap and
asks whether it answers correctly. It always did.

## What the measurement found

Instrument: `crates/apps/client/src/hud/reticle/seam_tests.rs` — 30 000 placements per map, pairs
of settled T-54 hulls 260–400 m apart, both on dry land, drawn from a fixed LCG; the whole live
path (sniper eye → sight sweep → firing solution → authoritative trace) in the order the game runs
it. Population kept: the shots a player is entitled to believe in, i.e. the sight ray reaches the
enemy hull and the firing solution is inside the gun's arc.

**Before any of this work**, per mille of believable shots the gun could not actually take:

| | Bystra | Prokhorovka |
|---|---|---|
| refused | 119 / 10 102 = **11.8 ‰** | 133 / 6 682 = **19.9 ‰** |
| ...of which the target is **not cut anywhere** in the picture | 31 = 3.1 ‰ | 17 = 2.5 ‰ |

Three quarters of the refusals are honest and readable: the eye reaches only the turret, the tank
is visibly hull-down, and refusing the shot is correct. Broken out by how much of the target's
silhouette (belly → roof, five points) the eye reaches:

| what the player sees | Bystra | Prokhorovka | verdict |
|---|---|---|---|
| turret only (2/5) | 78 | 103 | ordinary hull-down, not a defect |
| to the hull top (3/5) | 25 | 35 | visibly cut |
| belly hidden (4/5) | 6 | 9 | subtle |
| **whole tank open (5/5)** | **36** | **22** | **no cue exists** |

And the whole-tank-open cases split into two unrelated causes:

- **Bystra: 35 of 36 are `ImpactSurface::Cover`.** The shell grazes a barn roof the sight ray
  clears by ~7 cm. The sight sweep probes the world with a **zero-radius** ray (`aim.rs`) while
  the shell carries its calibre radius — that difference alone is the whole 7 cm.
- **Prokhorovka: 16 of 22 are terrain**, blocking **~29 m in front of the muzzle**, with the shell
  0.34 m under the sight line. This is the pure parallax lie.

**Why it happened at all.** The sniper eye sat **0.35 m above the gun axis** — a number whose only
justification was the comment beside it ("roughly where the gunner's optics sit"). A shell sent
320 m leaves the muzzle about **2 mrad** above the line to its target, so at that departure angle
height at the origin buys enormous reach along the ground: **0.35 m of eye ≈ 175 m of it.** The
eye looked over folds of ground the shell flew into, and the fold is below the sight line by
definition, so the picture could not show it. Median block: **61 m in front of the shooter**, with
the sight clearing that crest by **0.21 m**. Median extra elevation that would have converted the
refusal into a hit: **1.0 mrad — about 19 px of mouse at ×16.**

A separate readability finding fell out of the same screenshots: the big **green ring** the player
read as "I can shoot this" is `RETICLE_LOADED`, the breech-shut flash, which lives 0.95 s. It is
`[0.40, 0.90, 0.42]` against the penetration verdict's `[0.35, 0.85, 0.40]` — **5 % apart, and the
eye cannot tell them apart.** One place on screen, two unrelated meanings, both green.

## The waves

| # | what | status |
|---|---|---|
| 1 | **The sight sits on the gun's line.** `SNIPER_SIGHT_ABOVE_TRUNNION_M` 0.35 → 0.12 m, the band a real TSh-2-22 occupies; locked by the optic-band test and the seam ratchet. | DONE |
| 2 | **A refusal names its cause.** A blocked shot prints the metres to whatever eats the round, under the range, in the broken marker's own grey; a distant impact X is led back to the crosshair by a hairline. | DONE |
| 3 | **The sight probes the world with the shell's body.** `aim_point_with_sweep` carries the selected shell's calibre radius, the one the trace beside it already flew, so the crosshair stops ON the barn roof instead of threading it. | DONE |
| 4 | **One green, one meaning.** The breech-shut ring moves to steel cyan; green belongs to the armour verdict alone. | DONE |

### Results, same instrument throughout

Per ten thousand believable shots (per mille stopped resolving after wave 3 — six refusals in
9 849 rounds to zero there):

| | Bystra refused | Bystra **open and refused** | Prokhorovka refused | Prokhorovka **open and refused** |
|---|---|---|---|---|
| before | 118 | 31 | 199 | 25 |
| after wave 1 | 38 | 28 | 52 | 12 |
| **after wave 3** | **6** | **1** | **29** | **7** |
| ratchet ceiling | 10 | 4 | 40 | 12 |

**−95 % on Bystra and −85 % on Prokhorovka**, and the population that matters — the target standing
completely open with the shot refused anyway — went from 31 cases to **1** on Bystra and from 17 to
5 on Prokhorovka.

Wave 1 did the bulk of it (−68 % and −74 %) from one constant. Wave 3 took almost all of what wave 1
left on Bystra, because what survived there was the barn-roof graze and nothing else. What survives
now is real landform: a fold the gun genuinely cannot shoot past, which wave 2 makes the sight name
in metres.

The wave 1 trade is deliberate: the sniper eye drops 23 cm, so peeking a crest costs what the GUN
needs to be exposed, not what the camera wanted. That is the honesty doctrine applied to the sight —
what blocks the shell blocks the eye, from the same height.

### What is left, and it is the honest part

Six refusals in 9 849 on Bystra, twenty-nine in ten thousand on Prokhorovka — and all but one of
them now CUT the target in the picture. That is a fold the gun genuinely cannot shoot past, with a
tank visibly sitting behind it, which is hull-down working. Wave 2 makes the sight say so in
metres. Nobody is owed that shot.

## The rule this leaves behind

The ratchet in `seam_tests.rs` is the point of the whole program. Every earlier sight fix (register
G1–G12) was a good decision that stayed a single fix; the seam none of them measured is what
reached the player. Ceilings there are **measured, not chosen**, and raising one is a decision that
belongs in this file — never a way to get a run green.

Re-measure with `cargo test -p client --lib what_the_sniper_eye_reaches -- --nocapture`.

Two smaller rules came out of the same work and are worth stating on their own:

- **A proxy assertion outlives what it protects.** The ready ring's "no vertex may be
  blue-dominant" was guarding *"do not bring back the expanding flash"* and could never have caught
  an expanding ring that happened to be green. It is now measured directly: same radius as the
  drained arc, same radius mid-hold as at the instant it closes. When a lock has to be overridden,
  the question is what it was actually protecting.
- **An instrument that shares a mistake with the thing it measures sees nothing.** The seam test
  runs two probes on purpose — the sight sweep carries the shell's body, the silhouette probe
  carries none, because light has no calibre. Giving both the same radius would have made the
  instrument agree with itself by construction. (Register H2, learned the hard way once already.)

## Not in scope, recorded

- **The T-54's −5° on a hilly map.** On Prokhorovka **3.5 %** of sight-reachable hulls sit outside
  the gun's arc entirely (329 of 9 326). That is an honest refusal with its own visible signal (the
  barrel plainly cannot get there) and it is a vehicle/map balance question, not a sight one.
