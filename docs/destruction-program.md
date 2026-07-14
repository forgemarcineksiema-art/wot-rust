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
  locked cap again to 17,040; P3 replaces the radial HE mark with rim+bowl+clods (16,638).

## Known risks

1. Turret-ring seam: a hit zoned Turret whose visual contact is on the hull lip — the
   client retries the raycast in the other frame before falling back (phase 1).
2. Per-instance wreck meshes strain the "meshes shared per kind" assumption in the
   instance batcher — audit before phase 5 lands.
3. The `CoverView` refactor touches every consumer of `&[StaticCoverObject]`; it lands as
   its own mechanical commit, and bot routing must not cache destroyed cover (phase 6).
4. A detached turret's frame must freeze at detonation and ignore later replicated turret
   yaw (phase 4) — test-locked.
