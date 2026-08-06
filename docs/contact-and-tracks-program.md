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

```
a_tip = g · (belt_edge / h_com)
T-54     12.0 × 1.610/1.00 = 19.3 m/s²   SSF 1.61 g
Tiger I  12.0 × 1.853/1.15 = 19.3 m/s²   SSF 1.61 g
```

The available lateral acceleration is `lateral_grip_mu · g` = 11.4 m/s². **Margin 1.7× — the tracks
break loose before the hull can lean.** Every reachable path checked:

| Path | Requires | Reachable |
|---|---|---|
| Turn on the flat | 19.3 m/s² against a limit of 11.4 | No |
| Side slope | tan θ ≥ 1.61 → 58° | No — the map contract stops at 0.68 (34°) |
| Trip over a curb | 10.3 m/s of pure lateral velocity, instantly arrested | No |
| Broadside ram | 280 kN·s about the far edge; friction only supplies ~7% | No |
| **Asymmetric landing** | 3 m drop onto one track → ω 2.77 rad/s against a 2.09 threshold | **Yes** |

Lift energy for a T-54 is 380 kJ — roughly the muzzle energy of its own 100 mm AP round, perfectly
aimed at rotating the tank.

So rollover is not switched off by fiat: it is in the model and provably out of reach except by an
asymmetric fall. That one path becomes a **violent but recoverable roll excursion** — the hull
swings 40–50°, the suspension pays, and it settles back. The tank is never lost to terrain.

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
- **P0.2 `com_height_m` + the tipping gate.** One researched number per blueprint. *Lock:* for every
  playable vehicle, `lateral_grip_mu · 1.15 < belt_edge / com_height_m` — "no hull can tip before it
  slides". No behaviour change.
- **P0.3 Muzzle-inside-hull probe.** With the gun a ghost, a barrel pushed into another hull fires
  from inside it, bypassing the front plate. Measure first; fix (a minimum muzzle-to-target
  distance) only if it is real.

### Wave 1 — One contact solver (5 PR)

- **P1.1 Contact feature IDs.** `obstacles_contact` reports which face/vertex produced the MTV,
  stable across ticks. *Lock:* the same pose pair yields the same ID; the ID changes only when the
  touching feature genuinely changes.
- **P1.2 Speculative contacts + soft bias.** The skin stops being a standoff: it detects the
  approach, the constraint stops the hull *at touch*. Penetration up to a slop is pushed out by a
  bias **velocity**, never by moving the position. *Lock:* resting hitbox gap ≤ 0.03 m (today
  0.1217); no tunnelling at 14 m/s closing.
- **P1.3 Warm start + accumulated friction cone.** Impulses cached per contact ID; the tangential
  impulse bounded by μ × the *accumulated* normal, not one iteration's. *Lock:* five hulls pressed
  in a queue settle to < 1 mm/tick.
- **P1.4 Rotation into the solver; delete the teleport.** Yaw is constrained like translation, and
  `separate_overlaps` stops writing positions. *Lock:* a pivot against a neighbour transfers through
  velocity and shows up in the ram bill; no code path assigns `tank.position` outside integration.
- **P1.5 Predictor on the same model.** The client stops using the hard veto
  (`predict/obstacle_tests.rs:65` currently locks it under a name the server no longer honours).
  *Lock:* predictor and server resting positions agree within 1 mm over 300 ticks in contact.

### Wave 2 — Honest shape (2 PR)

The per-track drive reads gauge and contact run from the blueprint. If the collision footprint is
still a hand-typed 1.75 while the drive uses 1.32, the sim holds two different tanks. Shape must
precede drive.

- **P2.1 Footprint from `TrackShape`.** The motion footprint becomes `outer_x × hull.half_len`; the
  hitbox stays what shells resolve against. Preceded by a written cascade map (ram, spotting,
  terrain contact, armour fixtures, bare numbers in tests). *Lock:* the phantom test asserts **0**,
  replacing the 0.141 ceiling; the P0.1 table is re-measured and the deltas are stated.
- **P2.2 Low obstacles are ground.** Anything shorter than the vehicle's step height (the road-wheel
  radius — 0.405 m on a T-54) enters the support envelope like rubble instead of blocking in plan.
  *Lock:* a 0.30 m obstacle is driven over with a tilt and a speed bleed; a 0.60 m one still blocks.

### Wave 3 — The one reachable rollover (2 PR)

The tipping edge is `outer_x` — the number Wave 2 makes honest. This wave follows it.

- **P3.1 Asymmetric-landing roll excursion.** From the existing `landing_impact_mps` plus the
  left/right support-height difference at the catch. Authoritative, one extra `f32`, decaying at a
  fixed rate — no spring, no oscillation state, replay-safe. *Lock:* a 3 m drop onto one track
  produces ≥ 35° of roll that returns to the support plane; a symmetric landing produces none.
- **P3.2 Consequences.** The gun line follows the excursion, suspension damage scales with the
  asymmetry, the camera reads it. *Lock:* gun elevation limits are computed against the excursed
  hull, not the support plane.

### Wave 4 — Force per track (7 PR)

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
- **P4.5 `SteeringKind` per era.** `ClutchBrake` (era I) / `ControlledDifferential` /
  `Regenerative`. Precedent: `SuspensionKind` already sits on the blueprint. **Blocked on P4.6.**
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
