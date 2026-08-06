# Contact and Tracks Program

Approved 2026-08-06. The ground layer — what a hull touches, and what pushes it — rebuilt around
one solver and two tracks. This document is the plan; `docs/vehicle-movement-policy.md` and
`docs/physics-policy.md` are what it edits when each wave lands.

## Why

The player's report was "tanks cannot touch each other and yet react to each other, and hooking a
corner makes something shake". Measured on the full server tick (flat ground, both hulls under
throttle):

| Measurement | T-54 | Tiger I |
|---|---|---|
| Hitbox half-width / half-length | 1.750 / 3.2675 | 1.870 / 3.220 |
| Phantom, side (hitbox − track belt) | **0.140 m** | **0.0175 m** |
| Phantom, end (hitbox − hull plate) | **0.150 m** | **0.060 m** |
| Resting hitbox gap | 0.1217 m | 0.1222 m |
| **Visible metal gap, side by side** | **0.40 m** | **0.157 m** |
| **Visible metal gap, nose to nose** | **0.42 m** | **0.242 m** |
| First stop when driving in (metal) | ~0.55 m | ~0.39 m |

Three margins stack, and only the second is a bug in the ordinary sense:

1. **Phantom hitbox** — the box is bigger than the metal. Known and open (register M14, ceiling
   recorded at 0.141 by `vehicle_geometry/tests/gear_mesh_quality.rs`).
2. **`CONTACT_SKIN_M = 0.12`** (`physics/src/contact_impulse.rs:71`) — added so the impulse solver
   could see a resting touch at all, and it became the *resting distance*: the solver kills the
   approach velocity at the skin, so hulls park exactly one skin apart. Measured 0.1217 — the
   constant, to the millimetre.
3. **One-tick lookahead** — contact is solved against `position + velocity·dt`, so the trigger
   distance grows with closing speed. Hulls stop far out, then creep in over ~1.5 s.

And the jitter has a measured mechanism: **rotation is not collision-resolved server-side**
(`sim/src/state.rs:391` and `:410` pass an empty tank-obstacle list), so a pivoting hull swings its
box into its neighbour for free and only the phase-4 separation undoes it — as a pure position
teleport with no velocity, no impulse and no ram bill. Measured: **both hulls pushed 0.0198 m/tick
(1.19 m/s)** continuously during a stationary pivot; up to 0.090 m/tick with throttle and steer.

Ruled out by measurement: MTV axis flipping (0 flips >40°, worst 1.8°) and sign-alternating
oscillation in a straight press (gap dead stable, v = 0.000).

## Decisions taken

- **The gun stays a ghost.** No barrel capsule in motion or in aiming. Accepted consequence:
  barrels clip through hulls and buildings.
- **Rollover exists but is unreachable except one way.** See below.
- **The IS-3 keeps its geometry's consequences**, including an impaired neutral steer — *subject to
  the dossier gate* below.
- **`SteeringKind` per era**: clutch-brake / controlled differential / regenerative.
- **No gearbox, no torque curve, no engine RPM.** The engine stays one number: power.

## Rollover, as arithmetic

Tipping is about the outer edge of the track belt; the resisting lever is the belt edge, the
overturning lever is the centre-of-mass height.

**The first pass of this section was wrong, and how it was wrong is the lesson.** It put the centre
of mass at a hand-estimated 1.00 m and reported a comfortable 1.7× margin. The derived figures
(`game_core::stability`, mass-weighted from the blueprint's own heights and the installed modules'
own masses, every part read at the mid-height of its volume so the estimate leans HIGH) are
1.21–1.42 m, and the fleet's real margins are **1.15× to 1.38×**. The verdict survives; the comfort
does not.

```
a_tip  = g · belt_edge / h_com          T-54: 12.0 × 1.610/1.234 = 15.7 m/s²   SSF 1.30 g
a_slide = g · lateral_grip_mu · grip          12.0 × 0.95 × 1.04  = 11.9 m/s²
```

| Vehicle | com height | tips at | slides at | margin |
|---|---|---|---|---|
| Tiger II | 1.371 m | 1.36 g | 0.99 g | 1.38× |
| Tiger I | 1.398 m | 1.33 g | 0.99 g | 1.34× |
| T-54 | 1.234 m | 1.30 g | 0.99 g | 1.32× |
| Jagdtiger | 1.416 m | 1.27 g | 0.99 g | 1.29× |
| IS-3 | 1.259 m | 1.25 g | 0.99 g | 1.27× |
| Panther II | 1.361 m | 1.25 g | 0.99 g | 1.26× |
| T-34-85 | 1.210 m | 1.23 g | 0.99 g | 1.25× |
| **Centurion** | 1.391 m | 1.14 g | 0.99 g | **1.15×** |

The Centurion is the fleet's least stable hull and it is not an accident: it is the tallest
silhouette in the roster on one of the narrower gauges. Every path checked, with the T-54's derived
numbers:

| Path | Requires | Reachable |
|---|---|---|
| Turn on the flat | 15.7 m/s² against a limit of 11.9 | No |
| Side slope | tan θ ≥ 1.30 → 52° | No — the map contract stops at 0.68 (34°) |
| Trip over a curb | 8.3 m/s (30 km/h) of pure lateral velocity, instantly arrested | No |
| Broadside ram | 278 kN·s about the far edge; friction supplies ~8% of it | No |
| **Asymmetric landing** | 3 m drop onto one track → ω 2.50 rad/s against a 1.87 threshold | **Yes** |

Lift energy for a T-54 is 338 kJ — roughly the muzzle energy of its own 100 mm AP round, perfectly
aimed at rotating the tank.

So rollover is not switched off by fiat: it is in the model and provably out of reach except by an
asymmetric fall. That one path becomes a **violent but recoverable roll excursion** — the hull
swings 40–50°, the suspension pays, and it settles back. The tank is never lost to terrain.

One finding to carry forward: **the margin is thin because the game's lateral grip is deliberately
WoT-high**, not because these hulls are tippy. `lateral_grip_mu = 0.95` is authored to make the hull
"grip like WoT and only drift in genuinely hard turns"; at a physical 0.7 every margin above would
be half again as large. Wave 4's friction ellipse is where that number gets revisited, and this
table is what it has to answer to.

## Waves

### Wave 0 — Instruments (3 PR)

Everything after this changes numbers, and this repo has been burned by trusting an unverified
instrument (see `docs/battle-first/audit-register.md` and the playtest retraction).

- **P0.1 Mobility harness — LANDED.** `crates/runtime/physics/tests/mobility_baseline.rs`. Eight
  vehicles × three surfaces × six measurements, taken through the shipped drive step, held as a
  text table inside a 0.5% band. Three findings came out of the first run and none of them belongs
  to this PR:
  - **steady-state gradeability is ~0.42, not the 0.60 `MAX_CLIMB_GRADE` names.** On a plane the
    contact sampler reports `roughness == grade`, and `traction` is cut by roughness *and* slope, so
    the grip cap shrinks on the very face it is climbing. Arithmetic closes at 0.417. This is the
    standing-start figure; a momentum run-up still reaches higher, so it is a reason to check
    whether a map can author ground the fleet cannot climb — not yet proof that one can. **P4.4
    owns the cap; the check itself is a register entry.**
  - **gradeability is a fleet constant, not a vehicle trait** — one `longitudinal_grip_mu` for
    everybody. The lone exception is the Jagdtiger (0.355), the only hull whose climb is
    engine-limited rather than track-limited (4.7 m/s² of `P/v` thrust against a 5.3 m/s² cap).
    That is the kind of difference per-track forces should produce fleet-wide.
  - **the ground materials barely separate anything** — grip spans 0.95..1.04 by design, so the
    surface axis is almost purely rolling resistance today. P4.1 is what gives it teeth.
- **P0.2 Centre of mass + the tipping gate — LANDED.** `game_core::stability` and
  `crates/runtime/physics/tests/rollover_unreachable.rs`. The centre of mass is **derived**, not
  authored: published centre-of-gravity figures for these vehicles are scarce and inconsistent, so
  eight hand-typed numbers would be eight numbers nobody can check. Every input is instead already
  researched 1:1 — the blueprint's heights and the modules' masses — and every part is read at the
  mid-height of its own volume, which biases the estimate HIGH and therefore pessimistic for the
  question being asked. Swap a heavier turret in the garage and the centre of mass rises, because
  it does. *Lock:* for every playable vehicle, on the worst loadout the garage will build and the
  grippiest surface in the game, `tip_threshold > slide_threshold × 1.10`, plus a second test that
  the gate still touches the fleet rather than clearing by miles. No behaviour change.
- **P0.3 Muzzle-inside-hull probe — LANDED, and it was not a defect.**
  `crates/runtime/sim/tests/ghost_barrel_honesty.rs`. The worry was that shells spawn at the
  visible muzzle, so a barrel shoved into somebody would spawn its round past their armour and
  shoving your gun into an enemy would become a way to delete their frontal plate. Measured as a
  difference between two shots rather than against a written-down millimetre count, so it keeps
  meaning something when the armour is re-authored:

  | Shot | Plate | Nominal | Effective | Round arrives with |
  |---|---|---|---|---|
  | barrel buried (8 m, muzzle 1.2 m inside the target) | Mantlet/TurretFront | 200 mm | 247 mm | 188 mm |
  | across the field (55 m) | Mantlet/TurretFront | 200 mm | 247 mm | 186 mm |

  Same plate, same facing, same steel. The only difference is the 2 mm of penetration the closer
  round keeps by bleeding less velocity — `pen(v)` doing its job, not the armour going missing.
  Nothing to fix; the measurement stays as a lock, because it is exactly the property a future
  change to muzzle placement or spawn logic could quietly break.

### Wave 1 — One contact solver (5 PR)

- **P1.1 Contact feature IDs — LANDED.** `obstacles_contact` now reports a `ContactFeature`: which
  hull owns the reference face, which of its axes the face lies on, which side, and which corner of
  the other hull is pressed into it — all in the hulls' own frames, so a pair grinding past each
  other keeps one identity instead of minting a new one as they rotate. Additive: the normal and
  the depth are untouched, so no verdict anywhere moves.

  **Finding, and it is an input to P1.3 rather than a defect here.** Two hulls at a small relative
  yaw have two nearly equal ways out — across one flank or the other — and which is shallower
  CROSSES OVER as they slide past each other. Measured: the identity turns over up to twice across
  a 4 m grind, in the middle of exactly the sustained contact warm starting exists to smooth. The
  normal only swings by the relative yaw when it happens (11° in the test case, not the 90° an axis
  flip could cost), so this is not the chatter the separation pass was measured for in the opening
  audit — but it *is* a cache miss. **P1.3 must bias toward the incumbent axis**, which needs the
  previous tick and therefore belongs to the pass that owns the cache, not to the SAT.

  Second finding, smaller: a genuinely FLAT contact has both corners of the touching face at equal
  depth, and "the deepest one" is then a coin toss f32 noise can call differently every tick. A
  100 µm tie width settles it deterministically, but that is a tie-break and not hysteresis — the
  real answer for a face pressed flat on a face is TWO contact points, and that manifold belongs
  with the solver that would use it.
- **P1.2 Speculative contacts + soft bias — LANDED.** The SAT now reports a SIGNED separation, so
  the solver is told how far apart a pair still is instead of only whether they overlap, and the
  constraint allows exactly the closing that shuts that gap this tick and no more. The skin is gone
  as a concept — what is left (`SPECULATIVE_MARGIN_M`, 0.05 m plus the ground the pair can cover in
  a tick) decides only how early the arithmetic starts, never where the hulls come to rest. Overlap
  past the slop is asked back as a bias **velocity** rather than taken as a position.

  | | before | after |
  |---|---|---|
  | resting hitbox gap, head-on under throttle | 0.1217 m | **−0.0000 m** |
  | gap drift over 3 s pressed together | — | 0.0000 m, worst step 0.00000 m/tick |
  | interpenetration on a full-speed charge | — | 0.0000 m, no tunnelling (T-54 and T-34-85) |

  Visible metal, with the phantom hitbox margins still in place: T-54 side by side **0.40 → 0.28 m**,
  Tiger I side by side **0.157 → 0.035 m**. The Tiger genuinely touches now; the T-54's remaining
  gap is entirely its 0.14 m of phantom box, which is Wave 2's job.

  **One test moved, and the measurement that moved it turned up something bigger.** The Bystra bot
  soak's soft "never a near-drowning" bound (1.35 m) was exceeded by 10 mm on seed 5. Measured
  before/after across eight seeds, the change moves each seed by −0.04 to +0.11 m in both
  directions — a reshuffle of a chaotic statistic, not a shift in the mechanism — so the bound was
  renegotiated to 1.40 with that table written into the test. The same probe found **seed 1234
  wading 2.107 m on master, six hundred millimetres past the drowning line the soak's own opening
  line promises no bot reaches.** Filed as register **H1**; the mechanism was already diagnosed in
  `terrain/src/ground.rs` and needs a control redesign, not a constant.
- **P1.3 Warm start + accumulated friction cone — LANDED, and the program predicted the wrong
  symptom.** The lock written here was "five hulls pressed in a queue settle to < 1 mm/tick".
  Measured before touching anything: **they already did — 0.00000 m/tick at 2, 3, 5 and 7 hulls.**
  A queue does not shake. What it does is SINK, and the deeper the queue the worse, exactly where
  the arithmetic says a Jacobi pass runs out of reach (it propagates one hull per iteration, so
  four iterations reach four hulls).

  | queue | before | after |
  |---|---|---|
  | 2 hulls | 0.0202 m | **0.0006 m** |
  | 3 hulls | 0.0215 m | **0.0014 m** |
  | 5 hulls | 0.0277 m | **0.0137 m** |
  | 7 hulls | 0.0368 m | **0.0197 m** |

  Every length now rests inside the 0.020 m of overlap the solver is designed to allow; a
  seven-hull queue used to blow through it by 84%. Drift stays 0.00000 m/tick throughout.

  Three pieces, and they only work together. The solver keeps a **memory** (`ContactCache`) keyed
  by vehicle identity and touching feature — never by roster slot, a lesson the ram bill already
  paid for — so each contact starts the tick holding the impulse its predecessor ended with. The
  normal impulse is **accumulated and clamped at zero** rather than recomputed per iteration, which
  is what lets a contact give back impulse it no longer needs without ever pulling. And friction is
  bounded by μ × the **accumulated** normal: capping each increment separately let four iterations
  spend four times the budget one contact actually has, which is not a cone at all.

  The fourth piece is P1.1's finding, cashed in: the SAT now **holds the axis that was already
  winning** while the two ways out are within 0.05 m of each other. Without it the reference face
  changes hands mid-grind and the cache misses in the middle of exactly the sustained contact it
  exists to smooth.

  Cheaper, too: the geometry is projected once per tick instead of once per iteration, because
  positions do not move during a solve — only velocities do.
- **P1.4 The teleport is gone — LANDED, and it needed no yaw veto.** The plan said "constrain yaw
  like translation". Measured first: with P1.2 and P1.3 in place a pivot into a neighbour already
  leaves **zero** overlap — the solver's angular term holds it, and adding a hard veto on top would
  only have reintroduced a stop where a shove belongs. What was still there was the positional
  pass, and on the case it existed for it was at its worst:

  | spawn overlap | before (position) | after (velocity) |
  |---|---|---|
  | 3.00 m | 1 tick, **1.489 m/tick — 89 m/s** | 97 ticks, **0.033 m/tick** |
  | 1.50 m | 0 ticks, 0.740 m/tick | 69 ticks, 0.021 m/tick |
  | 0.10 m | 0 ticks, 0.040 m/tick | 19 ticks, 0.008 m/tick |

  Eighty-nine metres per second of travel that never happened, on a vehicle whose top speed is
  fourteen. `separate_overlaps` is deleted; the recovery ceiling scales instead — a hull separates
  at a metre per second plus another for every metre it is buried, so gross overlap is gone inside
  two seconds and it got there by driving. The pivot squirt halved as a side effect
  (0.0148 → 0.0077 m/tick): the teleport had been contributing there too, clearing its own evidence
  inside the tick.

  **Deleting it exposed a defect it had been papering over.** A hull nobody commanded that tick was
  never stepped, so it took contact impulses it never spent — shoved every tick, moving never, its
  velocity climbing without bound. Every living hull is stepped now, commanded or not, the same
  rule `settle_wrecks` already carries for the vertical. That is the movement policy's own
  invariant: nothing may DELETE momentum, and a hull that never spends its momentum has had it
  deleted just as surely.

  Queue penetration halved again on top of P1.3: 5 hulls 0.0137 → **0.0032 m**, 7 hulls
  0.0197 → **0.0106 m**.
- **P1.5 Predictor on the same model — LANDED.** The client ran the authority's tick in the
  authority's order now: decide a velocity, exchange contacts against it, spend what survived.
  Neighbours enter the solve as **immovable** bodies — the client is not authoritative over them,
  so it may predict being stopped and shoved by one, never predict shoving one. It keeps its own
  `ContactCache`, because a predictor that rediscovers every contact from nothing while the
  authority does not would disagree by more the longer the two leaned on anything.

  *Lock:* predictor and authority rest **within 1 mm** while pressed against the same hull, over
  the last 100 of 300 ticks.

  The test that used to guard this was called `prediction_is_blocked_by_other_tanks_like_the_server`
  and locked the predictor's hard veto — a stop the server had not used since contact started
  carrying momentum, and until P1.2 it fired a whole 0.12 m later than the authority's. Two models,
  one name, and a test that swore they agreed. Its replacement measures the two side by side.

**Wave 1 is complete.** With the predictor off it, the hard veto (`physics::tank_resolve`) has no
production caller left in the workspace — every `tank_obstacles` list handed to the drive step is
now empty — so it goes the way rapier went, and `TankWorldObstacles` stops carrying a field nobody
fills.

### Wave 2 — Honest shape (2 PR)

The per-track drive reads gauge and contact run from the blueprint. If the collision footprint is
still a hand-typed 1.75 while the drive uses 1.32, the sim holds two different tanks. Shape must
precede drive.

- **P2.1 Footprint from the blueprint — LANDED.** `game_core::HullPlan` is the rectangle a hull
  MOVES as: the outer face of the track belt and the hull's own plates, read from the same
  blueprint the mesh is built from. The hitbox is untouched and still owns hit resolution. Five
  call sites moved (cover collision, cover crushing, the roster's contact bodies, the predictor's
  local hull, the predictor's neighbours) and nothing else in the code had to.

  Phantom removed, per vehicle, from between two parked hulls — side / end:

  | | | | |
  |---|---|---|---|
  | T-54 **0.280 / 0.300 m** | Centurion 0.240 / 0.120 | Tiger II 0.160 / 0.100 | Panther II 0.060 / 0.110 |
  | Jagdtiger 0.040 / 0.100 | Tiger I 0.035 / 0.120 | T-34-85 0.020 / 0.300 | IS-3 0.000 / 0.110 |

  **The cascade was not in the code — it was in the tests.** Five of them had quietly adopted the
  hitbox as the hull's outline and had to be told which question they were asking: the contact
  measurements from Wave 1, the cover-stop lock, the T-bone ram (which typed `3.20 + 1.75 = 4.95`
  out by hand), the nose-to-tail stop, and the predictor's own parity pair. Each got a derived
  number instead of a copied one, and two of them state a STRONGER promise afterwards — the plates
  now stop AT a barn wall where the phantom used to hold them 0.15 m short of it.

  Found on the way and fixed with it: **the renderer scrolled the belts on a gauge taken from the
  hitbox half-width** (1.75 m against a real 1.32 m centre line on a T-54), so the inner and outer
  tracks disagreed by a third too much through every turn. Its test carried a second copy of the
  same wrong number and therefore passed — an instrument calibrated against the thing it is meant
  to judge. Both now read `ContactFootprint::half_gauge_x`.

  M14 is **halved**: `a_hull_is_blocked_by_exactly_the_metal_it_is_drawn_with` asserts zero, not a
  ceiling. The shell half keeps the 0.141 m ceiling and stays the user's decision.
- **P2.2 Low obstacles are ground — MEASURED OUT, deferred.** The plan was that anything shorter
  than a vehicle's step height should enter the support envelope instead of blocking in plan.
  Measured across all four shipped maps first: **272 cover objects, none below 0.80 m.** The
  shortest object anywhere is a 1.10 m rail cover; the shortest KIND in the vocabulary is the
  1.30 m wooden fence, which is already crushable. The mechanism would have had nothing to
  reclassify.

  Two things came out of measuring rather than building. The step height itself was wrong — the
  design conversation guessed "the road-wheel radius" (0.275–0.415 m across the fleet) where the
  documented vertical obstacle for these tanks is around 0.8 m, so hulls would have climbed half of
  what they should. And the real gap is not physics: **terrain already carries a hull over
  everything small** (ruts, crater rims, rubble all go through the support envelope), so what is
  missing is that a map author has no low object to place.

  Deferred as a **paired content-and-physics item**: a low tier in `StaticCoverKind` (append-only,
  wire identity — the user's call) shipping in the same PR as the physics that carries a hull over
  it, so the mechanism arrives with a caller. Building it first would be the shape this repo
  purged rapier, `tank_resolve` and the parry query for.

### Wave 3 — The one reachable rollover (2 PR)

The tipping edge is `outer_x` — the number Wave 2 makes honest. This wave follows it.

- **P3.1 Asymmetric-landing roll excursion — LANDED, and P3.2 came free with it.** A hull that
  took off level and comes down across a bank arrives at an angle to it: one track touches first,
  the landing impulse acts a half-gauge out from the centreline, and the hull turns about that
  belt. The rate is `m·v·b / I` about the contact edge and the excursion is the share of the
  TIPPING energy that rate represents, mapped onto the tipping angle — so the size comes from the
  fall and the vehicle, not from taste.

  | fall | peak roll | settles to |
  |---|---|---|
  | 3 m onto a 10° bank | **35.5°** | 10.0° — the bank it is standing on |
  | 3 m onto level ground | **0.00°** | level |
  | 3 m onto 8°/15°/25°/30° | 35.6 / 33.4 / 29.3 / 30.0° | always under the 52.5° tipping angle |

  **Nothing new is stored.** The excursion is added to the authoritative `hull_roll_rad`, and the
  attitude system's existing rate limit (1.4 rad/s) walks it back over the following half second. A
  spring would have carried oscillation state into the authoritative simulation, and every attitude
  in this game is deliberately spring-free so replays and the client predictor stay exact.

  That choice is also why **P3.2 needs no PR of its own**: because the excursion IS the hull's roll,
  the gun line follows it, the hitbox tilts with it, armour impact angles include it, the camera
  reads it and the wire carries it — all through paths that already existed. Suspension damage
  already scales with the landing impact that produced it.

### Wave 4 — Force per track (7 PR)

**Decided 2026-08-06: climbing is a DISCIPLINE**, and that decision is what this wave is now for.

Climbing was never a feature anybody wrote. It falls out of the force model resolving against
`forward_slope` — the grade along the hull's HEADING — so taking a face obliquely presents a
shallower one. Measured and now locked (`physics/tests/climb_envelope.rs`):

| | head-on | 15° | 30° | 45° | 60° off square |
|---|---|---|---|---|---|
| T-54, standing | 0.56 (**29°**) | 0.56 | 0.61 | 0.74 | 0.73 |
| T-54, run-up | 0.68 (**34°**) | 0.70 | 0.79 | 0.96 | **1.36 (54°)** |
| Tiger II, standing | 0.49 (26°) | 0.49 | 0.53 | 0.64 | 0.69 |
| any hull, run-up | 0.68 | 0.70 | 0.79 | 0.96 | 1.36 |

Two properties worth keeping on purpose: **from a standstill the vehicle matters** (power-to-weight
shows: Tiger II 26° against a T-54's 29°), and **with a run-up every hull is identical** — climbing
is a driver's skill, not a stat anyone can buy.

**What Wave 4 is actually for, restated.** Today a face past the momentum ceiling zeroes the
forward speed: the hull stops dead, which is the least readable failure mode available. With two
track forces the uphill belt loses normal load, spins, and the hull slews off and slides back down
— a failure you can see and learn from. That is the one place in this wave where per-track forces
buy LEGIBILITY rather than fidelity, and with climbing a discipline it is the headline.

**And the envelope above is the acceptance test.** The failure mode changes; the envelope does not.
A hull that can suddenly scrabble up faces the map contract walls off has not been improved, it has
been unmapped.

The idea in one line: **a track is a friction constraint with a non-zero target velocity.** The
drive is not a force added to the hull; it is a commanded belt speed, and the solver computes the
force that realises it, bounded by the friction cone on that track's normal load.

This does not add a mechanism — it *replaces seven special cases* in `physics/src/forces.rs`: the
static hold block, `climb_slip`, the direction-change brake hack, the designed-wall grade cut,
`turn_scrub`, and the independent lateral cap.

- **P4.1 Normal load per station + ground per track.** `N_i` from the support plane plus
  longitudinal and lateral load transfer (no oscillation state). Ground material sampled under each
  belt centre line, not once under the hull origin (`physics/src/contact.rs:105`).
- **P4.2 Two track forces.** `F_thrust = F_L + F_R`, `M_yaw = (F_R − F_L)·B/2 − M_scrub`. Calibrated
  so the P0.1 table is reproduced on grass within ±10%. Today's authored `turn_rate_rad_s` stops
  being an input and becomes the calibration target.
- **P4.3 Friction ellipse.** One budget per track, shared by thrust and scrub, so a hard turn costs
  acceleration because it spends grip — not because a constant says so.
- **P4.4 Retire the special cases.** Each deletion in its own commit with the behaviour it replaces
  measured. The **momentum-climb band (0.60–0.68) is a deliberate design** and must be re-added on
  purpose as a slip-dependent grip falloff, not left to die quietly.
- **P4.5 `SteeringKind` — LANDED, and the split is not the era ladder.** The plan sketched three
  kinds mapped onto the three eras, later meaning better. The research says otherwise: the **1942
  Tiger I turns about its own centre and the 1951 T-54 does not.** What separates them is the
  design school — the British Merritt-Brown triple differential and the Argus unit Henschel derived
  from it are regenerative, while the Soviet school standardised on two-stage planetary side
  mechanisms and the German mediums on single-radius gearboxes.

  A gearbox that can drive one belt BACKWARDS turns the hull about its centre. One that can only
  slow or stop a belt turns the hull about THAT: still a pivot, but around a point a half-gauge off
  centre, at half the rate, and it walks forward while it does it. Two variants, not five, because
  those are the two behaviours the sources actually distinguish.

  | | gearbox | turns in 3 s | walks |
  |---|---|---|---|
  | Centurion | Merritt-Brown Z51R triple differential | **100°** | **0.00 m** |
  | Tiger I | Argus regenerative (Merritt-Brown derived) | **92°** | **0.00 m** |
  | T-34-85 | clutch-and-brake side clutches | 67° | 1.29 m |
  | T-54 | two-stage planetary (ПМП) | 66° | 1.34 m |
  | Panther II | MAN single-radius | 53° | 1.12 m |
  | IS-3 | two-stage planetary (ПМП) | 48° | 0.94 m |
  | Tiger II | L 801 double-radius (inferred from a 2.08 m minimum) | 37° | 0.83 m |
  | Jagdtiger | Tiger II chassis (L 801) | 26° | 0.53 m |

  Sources are cited per vehicle in `game_core::SteeringKind::for_vehicle`, including which claim is
  the weakest (the Tiger II's, inferred from a stated minimum radius rather than from an explicit
  statement about neutral steer).

  **The P0.1 table caught it and the diff is the argument.** Exactly one column moved —
  `pivot_rad_s` — and only for the six hulls that cannot counter-rotate. Top speed, launch,
  braking, turn radius under power and gradeability are identical to the digit. That is what "this
  is about the pivot, not the drive model" looks like when it is true.

  Two tests had promised the old behaviour and were corrected rather than re-numbered:
  `neutral_steer_pivots_in_place_without_throttle` asserted that a T-54 rotates without
  translating, which was true of the model and false of the tank.

- **P4.6 IS-3 running-gear dossier.** Our blueprint says contact run 4.60 m on a 2.50 m gauge
  (L/B 1.84); published figures are nearer 4.30 on 2.44 (L/B 1.76). Before "the IS-3 can barely
  pivot" becomes a character trait, the geometry gets the 1:1 treatment the T-54 got — and the claim
  about its steering mechanism is sourced, not assumed.
- **P4.7 Terrain can throw a track.** Hard lateral scrub on rock, a landing above a threshold, or a
  belt driven into a hard step at speed. Rare and telegraphed — tuned to a stated rate the way
  `docs/` tunes fires. `BROKEN_ONE_DRIFT_BIAS` is deleted: the pull toward the dead side falls out
  of the geometry.

Optional tail: per-side track scroll distance in the snapshot, so the two belts visibly run at
different speeds in a turn (protocol addition, presentation payoff).

## The geometry that does the work

Contact run over gauge — the classic tracked-vehicle design ratio. Below ~1.2 a hull will not track
straight; above ~1.8 it will not turn. Every number below is already in the blueprints.

| Vehicle | B (gauge) | L (contact run) | L/B | pivot margin `2/(L/B)` | authored `turn_rate` |
|---|---|---|---|---|---|
| Tiger I | 2.98 | 3.60 | 1.21 | 1.66 | 0.58 |
| Panther II | 2.70 | 3.50 | 1.30 | 1.54 | 0.52 |
| Tiger II | 2.94 | 4.12 | 1.40 | 1.43 | 0.45 |
| T-54 | 2.64 | 3.84 | 1.46 | 1.37 | 0.78 |
| Jagdtiger | 2.80 | 4.20 | 1.50 | 1.33 | 0.40 |
| Centurion | 2.55 | 4.10 | 1.61 | 1.24 | 0.62 |
| IS-3 | 2.50 | 4.60 | **1.84** | **1.09** | 0.58 |

The IS-3 and the Tiger I carry the same authored turn rate from opposite ends of the scale. That
difference is real and today it does not exist.

The pivot margin is `2·(μ_scrub/μ_long)⁻¹ / (L/B)` — mass cancels, so it is pure geometry. **The
ratio μ_scrub/μ_long is therefore one fleet-wide constant that decides which hulls can neutral-steer
at all**, and it belongs in the same measured-once, stated-in-docs category as `GRAVITY_MPS2`.
Working band 1.0–1.2: above ~1.4 even the T-54 loses its pivot.

Calibration evidence that the model does not overturn today's numbers: the scrub-resistance moment
predicts a stationary pivot at today's authored rates costing 190 kW on a T-54 (engine ~390 kW) and
211 kW on a Tiger I (515 kW) — 40–60% of the engine on both. The model explains the existing figures
rather than contradicting them.

## What breaks, and the recipe

Breaks: every movement test asserting a speed or turn number, every replay, bot navigation (it plans
around a turn rate), and the fleet's mobility table.

Does not break: the wire protocol (track forces are computed per tick from state that already
exists — no new fields), the shared server/predictor step, the map contract's gradeability (it stays
`μ_long`).

1. Measure before (P0.1).
2. Build behind the existing `TankControllerSettings` interface.
3. Calibrate to the "before" table within the stated band.
4. *Then* let the differences that should appear appear: L/B, per-track ground, damaged-track pull.
5. One deliberate replay re-bless, with the before/after table in the commit message.
