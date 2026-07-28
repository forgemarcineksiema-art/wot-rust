# Destruction Program ("Honest Steel")

This document is the live plan for battlefield destruction: contact-true shell impacts,
visible vehicle damage, spectacular vehicle deaths, and destructible cover. It is a
deliberate direction change from the original scaffold disclaimer ("does not optimize for
full-world destruction") — but it stays inside the domain rule of
[armored-battle-domain.md](armored-battle-domain.md): **destruction is selective gameplay
state, not full-world destruction.** Cover has phases, modules have health, wrecks have
flags; the world never becomes a voxel sandbox.

## Why

The picture must never lie about collision. Today it does, three ways:

1. Shell hits resolve on collision geometry (baked armor volumes / legacy boxes) while the
   visual mesh is deliberately inset (`crates/vehicle/vehicle_build/tests/hitbox_fit.rs`),
   so impact marks float in the air next to the tank.
2. The server computes the true struck-plate normal (`SegmentImpact::Tank.plate_normal`)
   but never transmits it; the client reconstructs a coarse cardinal normal.
3. Destroyed modules, ammo-rack detonations, and dead tanks all read as a tint change —
   the damage state is on the wire, the picture ignores it.

## Standing invariants

Every phase preserves these; a phase that cannot is redesigned, not excused.

- **Server authority.** Gameplay truth (what hits, what pens, what blocks, what spots)
  lives in `sim` and replicates; the client only presents.
- **Gameplay stays on armor volumes.** The hybrid decision (2026-07-07): hit/penetration/
  ricochet keep resolving on the baked convex armor volumes. Presentation becomes
  contact-true against the visual mesh client-side. Collision geometry is not reshaped to
  the render mesh.
- **Determinism, no combat RNG.** All variation (thrown-track poses, turret pop-off arcs,
  dent placement) is splitmix64-hashed from replicated inputs — identical on every client,
  replay-stable.
- **When the picture and collision would disagree, collision truth changes honestly,
  server-side** (a popped turret stops blocking shells; destroyed cover stops blocking
  LOS) — never papered over visually.
- **Budgets are executable.** The combat hot-path bench and the FX frame-vertex budget
  test lock costs; raising a budget is a conscious diff, not drift.

## Phases

| # | Phase | Scope | Protocol | Status |
|---|---|---|---|---|
| 0 | Program doc + budgets | this doc, combat hot-path bench, FX vertex budget lock | — | implemented |
| 1 | Contact-true impacts | `DamageEvent` carries plate normal + shell direction; client raycasts the visual mesh (BVH per `VehicleKind`) and anchors marks flush on the armor | v19 | implemented |
| 2 | Conformal decals | penetration holes as mesh-clipped triangle patches that wrap curved castings | — | implemented |
| 3 | Visible module damage | gun droop, thrown track + dropped wheels, engine-deck fire, wreck dressing — all from state already on the wire | — | implemented |
| 4 | Turret pop-off | ammo-rack detonation kill detaches the turret: sim flag + trace exemption + `wreck_state` on the wire; client flies a deterministic ballistic arc | v20 | implemented |
| 5 | Wreck deformation | runtime `deform`-kernel dents on per-instance wreck meshes at death; ricochet spark streaks | — | implemented |
| 6 | Destructible cover | `CoverView` + `cover_states` (Intact/Rubble/Gone): HE and ramming destroy fences/tree-line segments and pound farm buildings into rubble; shell trace, movement, and spotting LOS all follow the state | v21 | implemented |
| 7 | Honest Steel T-54 | bounded persistent armor channels, multi-module interior path, pose-aware mantlet scars, engine fire and authored thrown-track gap | v25 | implemented |
| 8 | Real perforations | fleet-wide reusable apertures, v26 contours, analytic color/depth/shadow cut, T-54 interior lighting and local CPU remesh | v26 | implemented |
| E | Wreck epilogue (Inna Liga D6) | thrown-track ribbon prop shed onto the field (unit link mesh, deterministic S-curve, budgeted pool), wreck burn-out (~20 s of flames, then smolder), ricochet sparks leave along the deflection from the wire's plate normal + shell direction | — | implemented |

Sequencing: 0 → 1 → {2, 3 in either order} → 4 → {5, 6 in parallel} → 7 → 8 → E.

## Protocol ledger

| Version | Phase | Change |
|---|---|---|
| v19 | 1 | `DamageEvent` += `plate_normal`, `shell_direction` |
| v20 | 4 | `Snapshot` += `detached_turrets: Vec<TankId>` (the doc's earlier `wreck_state: u8` sketch); shell trace skips a detached turret |
| v21 | 6 | `Snapshot` += `cover_states`; destructible cover truth |
| v25 | 7 | `TankSnapshot` += `armor_breaches`, `track_break_t`, `engine_fire`; `DamageEvent` += module hit/destroy masks |
| v26 | 8 | Ammo-specific aperture contours, no-eviction merge policy, ingress/egress identity and deterministic thermal age |
| v27 | 8 | `TankSnapshot.fuel_fire` (a holed fuel tank burns as itself), component mask widened to u32; suspension penetrations degrade the struck side only (behavioral, no wire) |

## Honest Steel T-54 contract

- `ArmorBreachSet` is authoritative and replay-stable for every vehicle. Its 12 reusable aperture
  groups never evict old steel; nearby hits union as lobes and distant overflow becomes a scar.
  State exists only on the individual tank; shared production meshes are never mutated.
- A channel owns its semantic surface, armor material and `Hull` / `Turret` / `Mantlet` pose frame.
  T-54 presentation cuts the same contour analytically in color, depth and shadow passes. Its
  per-instance worker replaces the affected LOD0 patch and merges a paint-bearing torn lip plus a
  dedicated exposed-steel section/tunnel into the damaged skin. The server admits a later
  projectile only when its complete physical cross-section clears the irregular union.
- T-54 module damage resolves along the complete internal segment, nearest first, consuming the
  residual penetration budget. The former upper-glacis-to-gun shortcut is removed.
- A freshly thrown track records its normalized belt-path location. Presentation omits five links
  around it; crew re-seat clears the gap in the same tick that restores the track pool.

Phase 8 is intentionally landing as reviewable slices. Fleet physics, v26 state, analytical
clipping, detailed component traces, aperture-only interior light, the bounded remesh worker and
the grouped cross-frame remesh cases are implemented. The remesh contract is locked by tests on a
weld bead, a cast cheek, an undercut fold and an off-axis mantlet (kernel), plus one split shot
whose Hull/Turret/Mantlet fragments bake three independent per-frame skins on the production T-54
(client): contour vertices are seated barycentrically on the real curved steel, no whole source
triangle outside the patch may vanish, and `armor_surface_basis` keeps a true tangent basis for a
square-on shot on a rotated plate (the f32 residue there used to collapse every projection built
on it, including aperture clearance). The museum-reference detail pass (driver's station, V-54 ancillaries — the engine and
transmission bays were already dressed), the damaged/burning interior variants (per-instance
charring keyed by module state), the fire audio (procedural crackle bed) and the fleet-wide
visible breaches all landed in the W0 wave of the release program. What remains of phase 8 is
the final showcase review against the acceptance close-ups. The first museum slice now replaces generic fighting-
compartment blocks with T-54 D-10T/SG-43 equipment and the documented 1951 ammunition groups. Its
layout is checked against period T-54 drawings and the official MiniArt 37007 configuration; T-55
drawings are explicitly excluded. T-55 receives only the fleet physics contract; it never inherits
the T-54 mesh or interior.

The bounded state now counts 12 physical perforation groups rather than 12 mesh fragments. One
group may carry up to four independently posed ingress/egress fragments across Hull, Turret and
Mantlet; each frame is baked and cached separately. The client records a rolling 128-sample p95 for
worker build and main-thread integration, while integration remains capped at one completed damage
mesh per rendered frame. The performance gate is a representative capture, not a CI timing assert:
`cargo run --release -p client --example damage_budget_capture` drives a deterministic 150-hit
battle sequence over twelve production T-54s through the real worker (one integration per simulated
frame) and prints the rolling p95 against the budgets. Baseline (2026-07-13, dev laptop, release):
worker build p95 2.8 ms (budget 8 ms), main-thread p95 0.34 ms per frame (budget 0.5 ms), all 148
scheduled bakes completed. The schedule path itself no longer re-runs the full vehicle bake — the
catalog forges each kind's authoritative LOD0 bake once and shares it between the base meshes,
damage skins and wreck denting; before that fix every new hit re-baked the whole hybrid T-54 on the
main thread. CI locks only the plumbing (every scheduled bake completes, the telemetry window
fills), never the timings.

Each bump follows the established procedure (`docs/testing-and-regression.md`): append-only
fields, regenerated `crates/runtime/net/tests/snapshots/*_vNN.hex` fixtures, old-version
rejection tests kept.

## Budgets

- **Measured 2026-07-28** (dev laptop, release, interleaved A/B against master, 3 rounds — the
  bench has a ~9% run-to-run spread on this machine, so single readings prove nothing): the
  14-tank battle is unchanged at ~8.5 ms / 128 ticks (~66 µs/tick), and `urban_150` improves from
  ~13.1 ms to ~11.2 ms (~102 → ~88 µs/tick). The improvement is the live-cover `Cow` plus
  `LiveCover` resolving both views in one pass. Getting there took two corrections the bench
  caught and review would not have: resolving the movement and sight slices separately paid the
  whole 150-object rebuild twice, and the cover-crush fix had replaced a four-float-compare
  predicate with a full four-axis SAT run against every box on the map for every moving hull
  (~+18 µs/tick until it got the same circumradius broadphase the blocking test already had).
- **Combat hot path**: `crates/runtime/sim/benches/combat_hot_path.rs` — a 14-tank battle
  with live shells and cover, 128 ticks at 60 Hz through
  `SimulationState::apply_commands_on_battlefield`. The bench is the measurement; the
  budget is a review gate, not a flaky assert. New per-tick work (cover damage, detached
  turrets) must show its cost here before landing. Baseline at phase 0 (2026-07-07,
  dev laptop): ~6.7 ms per 128 ticks — ~52 µs/tick, comfortably inside the 60 Hz frame.
- **FX frame vertices**: the budget test in `crates/apps/client/src/fx/budget.rs` locks
  the worst-case vertex count of every capped FX pool (particles, terrain scars, tank
  decals). Phases that add stamps or raise caps must update the locked number in the same
  diff — the laptop target (integrated GPUs) is the reference machine.
  Protocol v26 removes penetration-hole quads completely: analytical apertures plus exposed
  rim geometry replace the black disk and streak fan, reducing the locked cap to 18,432.
  Fizyczny Świat P2 (v30) replaces the radial ground mark with the physically-true kinetic
  furrow (elongated gouge + forward spoil, fewer stamps than the old crater), lowering the
  locked cap again to 17,040; P3 replaces the radial HE mark with rim+bowl+clods (16,638); P4c drapes the rim and bowl over the true deformation (25 cells each), raising the cap to 35,070 — the price of marks that line the bowl instead of sinking into it.

## Honesty corrections (2026-07-28)

An audit of the shipped physics and destruction code found four places where the implementation
had drifted from the doctrine above. Each is fixed with its own locking test; none needed a
protocol bump, because all four are behavioural and land on the shared server/predictor path.

- **A ram's outcome was a function of roster order.** The pair loop charged `tanks[right]` the
  full bill and `tanks[left]` half, and the closing speed it measured is symmetric — so the model
  had no notion of an aggressor at all, and a stationary defender broadsided by a charging enemy
  paid half if it happened to sit earlier in the array. The impulse is now shared (Newton's third
  law) and the asymmetry that survives is geometry: each hull is charged by the FACE it met the
  collision with (bow 0.6, flank 1.4, rear 1.0 of the base severity). A charger paying less than
  the hull it t-bones is now a consequence of where the plates are, not of where the tank is
  stored. `sim/tests/ramming_contact.rs`.
- **A round through an existing perforation dealt nothing.** `admits_existing_channel` correctly
  decides that a projectile's full cross-section clears an open aperture — and then the shell step
  teleported the round past the hull with `last_penetrated_target` set: no damage, no module
  touched, no `DamageEvent`, no `ShellImpact`. Shooting the same hole twice was punished, and the
  crew that fired got no feedback at all. What an open channel buys the round is now the ENTRY
  STEEL and nothing else (`resolve_penetration_through_open_channel`): it pays no armour and cuts
  no second ingress wound, but it resolves the internal module path, the damage and the egress
  exactly like any other perforation. `sim/src/combat.rs`.
- **Cover crushing reached sideways.** The crush test put the hull's HALF-LENGTH plus the approach
  slop around its centre as a per-axis box, ignoring yaw — so a T-54 driving parallel to a
  hedgerow flattened it from 2 m off its own flank, and since one `TreeLine` box is a whole run of
  hedge, a single clean pass deleted tens of metres of it without contact. The crush now carries
  the hull's real oriented footprint one approach-length ALONG ITS TRAVEL and runs the same SAT
  movement collides with (`physics::footprint_overlaps_cover_object`). `sim/tests/cover_destruction.rs`.
- **Wrecks did not obey gravity.** The drive step is skipped for dead hulls, and the vertical
  resolution went with it, so a hull killed in mid-flight hung at the altitude it died at for the
  rest of the battle — blocking shells and hulls, as `StaticCoverKind::Wreck` does, from a hole in
  the sky. `sim::wreck::settle_wrecks` now resolves the vertical (only the vertical) for every
  dead hull, commanded or not, on the same support envelope a live hull reads. A wreck already
  resting on its support is a bit-identical no-op, which is what keeps replays stable.
  `sim/tests/wreck_settle.rs`.

Two supporting changes ride along:

- The crater ledger's cap used to evict `craters[0]` unconditionally. Filling a hole back in
  un-deforms the heightmap, and `resolve_vertical`'s rule that rising ground always carries the
  hull turns that into a snap of up to the full 1.2 m depth cap in one tick — a hull teleported
  upward by a shell that landed elsewhere. Eviction now takes the oldest crater NOBODY is standing
  in; when every hole is occupied the burst leaves no record, which is the smaller lie.
- `live_cover_for_blocking` borrows the authored slice while nothing is broken. A
  `StaticCoverObject` carries two `String`s, so the old unconditional rebuild cost ~300 heap
  allocations per tick on a city map. The server had already hand-rolled this `Cow` for the bots'
  copy; the rule now lives once, in `sim`, where every caller gets it.

## Rubble is terrain (2026-07-28)

The audit's seventh finding was not a bug but a missing feature, and closing it is the reason
`CoverPhase::Rubble` now means something different to different consumers.

`physics::cover::footprint_blocked_by_cover` reads neither `position.y` nor a box's height — every
cover box is an infinitely tall prism for movement. So the care `live_cover_for_blocking` took to
lower a collapsed building reached the shell trace and the spotting LOS and **never reached the
hull**: a flattened block walled a tank exactly as the standing block had. "Destruction opens the
map" was true for fire and for sight and false for manoeuvre, which is half the reason to bring a
building down in a 7v7.

**The rule.** A collapsed building is debris, and debris is ground.

- The one resolved live slice became two, and every call site states which question it is asking:
  `live_cover_for_sight_and_shells` (shell traces, spotting LOS, camera solids — a mound is still a
  lowered box that blocks) and `live_cover_for_movement` (hull collision — a mound is simply
  absent).
- The mound reaches movement through the SUPPORT ENVELOPE instead, as `terrain::RubbleMound`: a
  truncated pyramid with a flat top and flanks at the angle of repose of broken masonry
  (`RUBBLE_REPOSE_GRADE` = 0.78, ~38°). Debris stays inside the authored footprint, so streets do
  not silt up, and the surface is continuous with the ground at the footprint edge.
- `track_contact::rest_line` and `contact::sample_tank_terrain_contact` both read `max(terrain,
  debris)`. Both, deliberately: the support envelope decides where the hull RIDES, the probe cross
  decides what the drive's forces resolve against. A mound that raised the hull while leaving the
  probes on flat ground would be a pile you climb for free.
- An empty mound list short-circuits to the plain heightmap read, so a battlefield nothing has
  knocked down yet is bit-identical.

**What the geometry actually does, which is not what the design predicted.** The repose grade sits
above the momentum-climb ceiling (0.68), so the flank is steeper than anything a tank climbs — and
yet every mound the shipping `rubble_height_frac` values produce is still crossable. A barn mound
is 2.4 m of debris over a 3.1 m talus; a T-54's running gear spans 4.4 m between end stations. The
rigid beam BRIDGES a flank shorter than its own wheelbase, exactly as it bridges a trench narrower
than its wheel pitch. A pile smaller than the tank is something a tank drives over.

So there is no angle-of-attack gate on rubble, and the "skill climb" the plan expected does not
exist at these pile sizes. What does exist is a real cost: the crossing pitches the hull up, bleeds
speed through the same force model every slope uses, and puts the belly in the air on the crest.
The wall mechanism is still there for a flank the gear cannot bridge (it needs ~5 m of debris) and
is test-locked, but no map authors one today. If a mound should ever gate manoeuvre, the knob is
`rubble_height_frac`, not a special case in the drive.

Locked by `physics/tests/rubble_support.rs` (the envelope rests on debris, the crossing costs tilt
and ground, an unbridgeable flank walls a charge) and `sim/tests/cover_destruction.rs` (a hull
drives over a collapsed building; a shell still dies in the same mound; a STANDING building is
still a wall and nothing gets onto its roof).

## Known risks

1. Turret-ring seam: a hit zoned Turret whose visual contact is on the hull lip — the
   client retries the raycast in the other frame before falling back (phase 1).
2. Per-instance wreck meshes strain the "meshes shared per kind" assumption in the
   instance batcher — audit before phase 5 lands.
3. The `CoverView` refactor touches every consumer of `&[StaticCoverObject]`; it lands as
   its own mechanical commit, and bot routing must not cache destroyed cover (phase 6).
4. A detached turret's frame must freeze at detonation and ignore later replicated turret
   yaw (phase 4) — test-locked.
