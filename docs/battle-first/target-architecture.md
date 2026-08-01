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

## Seven layers, one rule

**A crate in layer N depends only on layers below N.** That is the whole rule; a test enforces it.

```
L0  foundation/   pure data + mathematics. No I/O, no GPU, no clock.
                  game_core · mesh_core  ← (vehicle_geometry)

L1  kernels/      stateless mesh operations. They know only mesh_core.
                  solid · revolve · sweep · sdf · sdf_mesh · cast_loft · deform · detail
                  ✗ panel, shell, experimental_geometry — deleted (zero dependents)

L2  content/      CONTENT built from kernels. The fleet and the maps live here.
                  vehicle_build · vehicle_forge · world_forge · map_forge

L3  runtime/      the world in motion.
                  terrain · physics · sim · net · engine · audio
                  battle_host  ← (apps/server as a library)

L4  render/       renderer_api · renderer_wgpu
                  scene_build  ← (from world/ — it always depended on renderer_api)

L5  ui/           ui_kit  ← (from client: hud/font, primitives, theme, icons)

L6  apps/         client · server(bin) · editor · tools · probe(new)
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

This also removes the current ratchet in the wrong direction — `architecture_rules.rs` hard-requires
29 `docs/` paths, which makes deleting a stale document harder than keeping it.
