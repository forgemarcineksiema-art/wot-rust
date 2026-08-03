# Target architecture

## The thesis

The crate structure is broadly fine. What is missing is not a better diagram — it is **enforcement**.

> **At 1M LOC, the architecture is whatever is enforceable. The rest is a suggestion, and nobody
> remembers a suggestion after thirty thousand commits.**

## Three laws

**1. The law of the ratchet.** An architectural decision lands as a **rule** in `quality`, never as
an edit. It ships green with a visible allowlist of today's violations, burned down over time.
*A fix without a rule is unfinished.*

**2. The law of the boundary.** Every boundary — layer, `enum`↔shader, code↔data, wire — is bound by
a **type or a test**, never by a number repeated on both sides. `MaterialRole` against
`vehicle.wgsl`, and `16` against `u16` in the spotting mask, are the same disease.

**3. The law of coverage.** Validation asserts **what must exist**, never iterates what happens to
exist. No `continue` past missing data, no `_ =>` on an identity enum, no `.take(N)` without a named
bound and a test.

These three are the entire onboarding for anyone — human or model — who must not make the codebase
worse.

### Why an allowlist and not "fix first"

A rule that waits for a big cleanup PR does not exist until that PR lands, and in the meantime
fresh violations keep arriving. A rule that ships with a visible allowlist starts protecting
immediately and makes the debt countable. (Salvaged from the retired W0 gate-rules spec; the rules
themselves are the 18 test files in `crates/tooling/quality/tests/`.)

## Seven layers, one rule

**A crate in layer N depends only on layers below N.** That is the whole rule; a test enforces it.

```
L0  foundation/   pure data + mathematics. No I/O, no GPU, no clock.
                  game_core · mesh_core  ← (vehicle_geometry)

L1  kernels/      stateless mesh operations. They know only mesh_core.
                  solid · revolve · sweep · sdf · sdf_mesh · cast_loft · deform · detail · panel
                  ✗ shell, experimental_geometry — deleted (zero dependents). panel was deleted
                    with them, then restored (073dfe1) when t54_fender gave it a real caller

L2  content/      CONTENT built from kernels. The fleet and the maps live here.
                  vehicle_build · vehicle_forge · world_forge · map_forge

L3  runtime/      the world in motion.
                  terrain · physics · sim · net · engine · audio
                  battle_host  ← (apps/server as a library)

L4  render/       renderer_api · renderer_wgpu
                  scene_build  ← (from world/ — it always depended on renderer_api)

L5  ui/           ui_kit  ← (from client: hud/font, primitives, theme, icons)

L6  apps/         client · server(bin) · editor · tools
```

### What the single rule dissolves

| today | after |
|---|---|
| `editor → client` — 30k LOC (winit, wgpu, cpal, audio, server) for **four symbols** | `editor → ui_kit` (L6→L5) |
| `client → server` | `client → battle_host` (L6→L3) |
| `world/scene_build → render/renderer_api` | `render/scene_build` — the layer now matches |
| `vehicle_geometry` a "kernel" that every kernel depends on | `foundation/mesh_core` at L0 |
| `client::` re-exporting 28 `scene_build` items for its own examples | examples import `scene_build::` directly; `lib.rs` drops from 74 to ~25 lines |
| 41 examples in `client`, 37 cloning the same prologue | one `probe` binary with a subcommand — 41 compilation units become one |

**Score 2026-08-03**: the `editor→client` and `client→server` rows are burned (#424 `ui_kit`,
#414 `battle_host` — `APP_TO_APP_ALLOWLIST` is empty) and the probe consolidation landed (#408) —
as ONE subcommand example binary at `crates/apps/client/examples/probe/`, not the new app the L6
row once proposed. Still standing: `scene_build → renderer_api` (the one `UPWARD_ALLOWLIST`
entry) and `vehicle_geometry` in `kernels/`.

Two edges remain legal but worth noting: `net → sim` (the wire layer knows the simulation — an
inversion, not a cycle) and fleet content still inside `kernels/` (`solid/t54*.rs`), which goes on
rule 1's allowlist and is burned down with the T-54 stack work in Wave 4.

## The T-54 stack — the goal is not tidiness

23 dedicated files against Tiger I's 4. Fifteen `if kind == T54_1951` sites. Each of those fifteen is
a **capability that should exist for the whole fleet, gated on the presence of DATA** — and is
instead gated on the identity of one vehicle.

> **`if kind == T54_1951` becomes `if let Some(detail) = bp.visual_detail()`.**

Then the T-54 stops being an exception and becomes the vehicle with the most complete data, and the
next museum-grade vehicle costs a RON file instead of 23 Rust files.

The codebase already named this anti-pattern itself, in `vehicle_blueprint/mod.rs:150-153`:
*"This lived as `if kind == T54_1951` inside the shared bake — a content decision hiding in code."*
Diagnosed once, committed fifteen more times.

## Documentation

**A policy cites its enforcing test. A rule with no test is not a policy — it is a note.**

**Status 2026-08-03: the tree below was NOT executed** — `docs/` keeps its flat shape, and this
layout stays a proposal. The boldface principle above is what actually governs.

```
docs/
  ARCHITECTURE.md     three laws, seven layers, a map of where things live.
                      The only document required before a first PR.
  rules/              enforceable policies. Each opens with:
                      "Enforced by: crates/tooling/quality/tests/<file>.rs::<test>"
  programs/active/    programs in flight
  programs/done/      archive — dated, not deleted
  notes/              everything without a test. No authority, no upkeep obligation.
  battle-first/ maps/ vehicles/ ui/ ops/
```

The side effect is the point: **a document cannot go stale in secret.** If it states a rule, the
rule is tested; if it is not tested, it lives in `notes/` and nobody pretends it binds.

### What is deliberately NOT a rule

Assertions that a document exists or contains a phrase. Removed 2026-08-02 (#406): the
hard-required `docs/` paths and the phrase checks — roughly 44 % of the old gate — ratcheted
AGAINST cleaning up stale documentation, making deleting a stale document harder than keeping it.
Their replacement is editorial, not executable: the boldface rule at the top of this section.
