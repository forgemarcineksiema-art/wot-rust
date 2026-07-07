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
| 0 | Program doc + budgets | this doc, combat hot-path bench, FX vertex budget lock | — | in progress |
| 1 | Contact-true impacts | `DamageEvent` carries plate normal + shell direction; client raycasts the visual mesh (BVH per `VehicleKind`) and anchors marks flush on the armor | v19 | planned |
| 2 | Conformal decals | penetration holes as mesh-clipped triangle patches that wrap curved castings | — | planned |
| 3 | Visible module damage | gun droop, thrown track + dropped wheels, engine-deck fire, wreck dressing — all from state already on the wire | — | planned |
| 4 | Turret pop-off | ammo-rack detonation kill detaches the turret: sim flag + trace exemption + `wreck_state` on the wire; client flies a deterministic ballistic arc | v20 | planned |
| 5 | Wreck deformation | runtime `deform`-kernel dents on per-instance wreck meshes at death; ricochet spark streaks | — | planned |
| 6 | Destructible cover | `CoverView` + `cover_states` (Intact/Rubble/Gone): HE and ramming destroy fences/tree-line segments and pound farm buildings into rubble; shell trace, movement, and spotting LOS all follow the state | v21 | planned |

Sequencing: 0 → 1 → {2, 3 in either order} → 4 → {5, 6 in parallel}.

## Protocol ledger

| Version | Phase | Change |
|---|---|---|
| v19 | 1 | `DamageEvent` += `plate_normal`, `shell_direction` |
| v20 | 4 | `TankSnapshot` += `wreck_state: u8`; shell trace skips a detached turret |
| v21 | 6 | `Snapshot` += `cover_states`; destructible cover truth |

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

## Known risks

1. Turret-ring seam: a hit zoned Turret whose visual contact is on the hull lip — the
   client retries the raycast in the other frame before falling back (phase 1).
2. Per-instance wreck meshes strain the "meshes shared per kind" assumption in the
   instance batcher — audit before phase 5 lands.
3. The `CoverView` refactor touches every consumer of `&[StaticCoverObject]`; it lands as
   its own mechanical commit, and bot routing must not cache destroyed cover (phase 6).
4. A detached turret's frame must freeze at detonation and ignore later replicated turret
   yaw (phase 4) — test-locked.
