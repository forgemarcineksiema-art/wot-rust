# Audit register — 2026-08-01

Every entry verified against master `101068a`. Check an entry still exists before working it.
Struck rows carry the PR that closed them; rows marked **still open 2026-08-03** were re-verified
against master `49a6f18` (#428).

## The pattern under most of these

The codebase knows how to do things right, and does them right **once**:

| lesson | applied | skipped |
|---|---|---|
| assert a floor instead of `continue` | `game_core/tests/spaced_armor.rs:53` | 18 other `for kind in VehicleKind::ALL` loops |
| golden without an opt-in | `tools/tests/studio_goldens.rs` | `client/tests/look_goldens.rs` |
| scratch buffer instead of allocating | `frame_p95_scratch`, `audio_out`, `fx` | `client/vehicle/render_frame.rs` |
| exhaustive match on an identity enum | `blueprint_ron()` | `armor/vehicle_volumes.rs:48` |
| data contract asserts PRESENCE | per-map `spawn_zones.len() == 2` | `map_forge/report.rs` — the editor's gate |
| a boundary bound by a TYPE | `net::Transport` (3 impls) | `MaterialRole` ↔ `vehicle.wgsl` |
| tests in a sibling file via `#[path]` | `bot_combat.rs`, `bot_aim.rs` | `bots.rs`, `bot_routes.rs` |

`docs/vehicle-forge-policy.md` already diagnosed this exactly — seven of eight guns shipped with
24–28 inconsistently wound edges while every gate stayed green: *"A contract nobody runs on the
real thing is a document, not a gate."* The diagnosis was perfect; the fix was an edit.

---

## A. Gameplay — found by playing (the first measured battle, 2026-08-01)

| # | finding | severity |
|---|---|---|
| A1 | ~~Zero ricochets~~ — **WITHDRAWN, my measurement error.** The harness logged only events with `damage_hp > 0`, and a ricochet deals zero by definition. Re-measured: **8 ricochets in 79 impacts (~10 %)**, and of the 10 impacts above the 70° threshold, **8 ricocheted** — the other two are overmatch, which correctly disables ricochet. The armour model works. | — |
| A2 | ~~A 7v7 does not resolve inside its timer~~ — **WITHDRAWN, my error.** I assumed a 7-minute limit; `RANDOM_BATTLE_TIME_LIMIT_S = 600`. The battle ends exactly on the limit. | `server/battle.rs:105` |
| A3 | ~~Bots barely fight~~ — **OVERSTATED, same filtered log.** Re-measured: **79 impacts and 9 of 14 tanks destroyed** over the full battle. There is a real fight. Target selection is still only `bot_nearest_engageable_enemy`, so the *depth* critique stands; the "passive" claim does not. | `server/bot_combat.rs` |
| **A5** | **A 4-to-1 battle is declared a draw.** At time expiry team 1 held 4 alive against team 2's 1, and the outcome was `Draw { TimeExpired }`. A decisive tank advantage ending as a draw is the least satisfying resolution available; most games award the win on tanks or damage remaining. **This is the finding that survived the corrections.** | `server/local.rs` |
| A4 | ~~Camera freezes on player death~~ — **WITHDRAWN, my instrument again.** The death camera works and is already test-locked: `render.rs:434` ticks the spectate every frame, `present.rs:44` drifts the orbit, the boom flows to 1.3× the map ceiling over ~2 s, and `death_gives_the_wreck_a_wide_slow_orbit_and_refuses_the_scope` asserts both. My harness builds a fresh `BattleCameraController::default()` per frame and never enters that path. | — |

### Measured shot geometry (the numbers that replaced A1)

Full battle, Bystra, seed 7, 600 s: **79 shell impacts** — 60 penetrated, 8 ricocheted, 11 absorbed
without either. Impact angles spread across the whole 0–89° range rather than clustering flat.
Median margin of penetration over effective armour: **+58 mm** (min −796, max +168). Most-hit zone:
**HullSide, 36 of 77** — the flanks are found and used.

**Lesson recorded on purpose — and it is now a pattern, not an incident.**

**Every finding this audit took from the scripted harness was wrong or overstated. Every finding it
took from reading code held up.** Four of four: zero ricochets (a filtered log), the unresolved
timer (an assumed constant), passive bots (the same filtered log), the frozen death camera (a
hand-rolled camera that never enters the real path). The one gameplay finding that survived — the
draw at 4-to-1 — survived *because it was independently confirmed in `local.rs` before it was
believed*.

The harness renders with its own camera and logs a filtered subset of events. **It is not the
game.** Anything it reports is a hypothesis about the real code path until that path is read. This
is the same failure the register documents elsewhere as *"a contract nobody runs on the real thing
is a document, not a gate"* — committed four times by the person quoting it.

## B. Honesty — silent failures of the core promise

| # | finding | where |
|---|---|---|
| B1 | **Silent armor.** `_ => return None` ends the match; `shell_trace/tank.rs:54` then resolves against BOX bands instead of convex volumes. No panic, no log, no red test. | `armor/vehicle_volumes.rs:48` |
| B2 | The defence that should catch B1 **skips the broken case**: 18 fleet loops use `else { continue }`. | `sim/tests/turret_taper.rs:140` +17 |
| B3 | **`MaterialRole` 9/10/11 misroute three ways** — torn-armor albedo, the rubber texture layer, and INTERIOR aperture lighting. The enum's own doc comments argue these must be distinct roles; all three render identically. | `pbr_mesh.rs:50-61` vs `vehicle.wgsl:120,249,343` |
| B4 | **Silent cap.** `.take(16)` forced by the `Vec<u16>` mask — tank #17 spots nobody. Invisible at 7v7, on the boundary at 8v8. | `sim/spotting.rs:270` |
| B5 | **Data contract without a presence assert** — the report validates spawn zones that exist, never that each team HAS one; the server then panics. | `map_forge/report.rs`, `server/setup.rs:131` |
| B6 | **The ammunition system breaks the doctrine the module system upholds** — APCR/HE derived by ×1.20/×1.25/×0.85 multipliers, while `modules/catalog.rs:78-81` states "authored… **not a synthetic multiplier**". | `weapon.rs:194-213` |
| B7 | ~~Fire deals no damage at all~~ — **FIXED.** Fire now costs hit points, is credited to whoever lit it, and the crew smothers it. See "Fire" below. | `sim/src/fire.rs` |
| B8 | ~~`fuel_fire` never reached the screen~~ — **FIXED.** `sync_tanks` carried it into the component correctly and the projection back out hardcoded `false` on the line below the correct `engine_fire` mapping, killing a replicated damage state one line before two waiting consumers. Locked by a test asserting both flags survive the projection. | `engine/src/world.rs:205` |
| B9 | ~~A fuel fire can never be extinguished~~ — **FIXED.** Extinguishing is now the fire system's own job on its own clock, and the engine patch no longer clears the flag as a side effect (one owner per flag). | `sim/src/fire.rs` · `repair.rs` |

### Fire — implemented 2026-08-01 (branch `fix/fuel-fire-reaches-presentation`)

Author's direction: *fire must kill and must be extinguishable, but tanks must not catch fire often
or easily.*

**Ignition is earned, deterministically, with no roll.** The internal path already knew where a
round still carried enough energy to throw fragments, so ignition asks about energy at the component
(`InternalPath::energetic_*`) rather than contact with it — but at its OWN threshold,
`FIRE_ENERGY_MM = 100`, far above the 8 mm that spawns spall. Fragments come off almost any
penetration; a fire does not.

Before: any penetrating engine kill lit the deck, and a T-54's 150 hp engine dies to one AP round,
so every centreline penetration was a guaranteed fire; fuel lit if the ray so much as brushed a
tank.

**What the tuned threshold implies is the point:** a round needs ~300 mm of muzzle penetration to
light this engine *through the glacis*, and the era's real guns carry 175–200. **A frontal hit
essentially never starts a fire — fires are what the flank and the rear cost you.** Locked by
`wrecking_an_engine_is_not_the_same_as_lighting_it` (260 mm wrecks without lighting, 340 mm lights).

**The burn is knowable.** `ENGINE_FIRE_HP_PER_S = 9`, `FUEL_FIRE_HP_PER_S = 15`, `FIRE_FIGHT_S = 12`
— a fought fire costs exactly 108 or 180 hit points and nothing else, on a 1 550 hp hull. Both
alight is one vehicle burning at the worse rate, never the sum. Damage drains in one-second pulses
like drowning: per-tick damage would round to zero at 60 Hz (0.3 hp a tick), and per-tick *events*
flooded the log — **a measured battle produced 2 001 damage events against 82 after pulses.**

**Tuned against measured battles, not intuition** (Bystra, seed 7, full 7 minutes, 14 tanks):

| ignition threshold | fires per battle | hp burned |
|---|---|---|
| shared with spall (8 mm) | 8 | 2 226 |
| 60 mm | 5 | 690 |
| **100 mm (shipped)** | **2** | **360** |
| 140 mm | 0 | 0 |

Two fires is one per team per battle. Re-measure with the same seed before moving any of these
numbers.

## C. Depth — strong skeleton, shallow last 30%

| # | finding |
|---|---|
| C1 | **Terrain `cell_m = 5.0` on all four maps.** A T-54 is 6.2 × 3.3 m — the tank is one cell. No feature smaller than a tank; every hull-down is authored, never emergent. |
| C2 | **Normalization is a flat constant** (AP 5°, APCR 2°), independent of caliber against thickness. `resolve.rs:222-227` |
| C3 | **Ricochet is a hard step** at 70°/85° with no transition band and no near-glance energy loss. `resolve.rs:230-238` |
| C4 | **Damage does not depend on what was hit** — a turret penetration and an engine-bay penetration deal identical HP. |
| C5 | **Physics is a 2.5D model** — `integrate_hull_position` integrates X and Z only; attitude is a 52-line rate limiter, deliberately spring-free. No roll-over, no track slip, no load transfer. |
| C6 | **Spotting is pure LOS + range** — no camouflage, no concealment contest. Follows from the doctrine, but removes scouting, ambush and light-tank identity. Needs to be a decision on the record. |
| C7 | ~~No frame-time measurement exists anywhere~~ — **CLOSED (#374, W1 1.5).** Frame-time p50/p95/p99 measurement landed; the one-look policy has a number. |

## D. Structure

| # | finding |
|---|---|
| D1 | **God objects**: `ClientApp` 64 fields; `EditorApp` 36 fields + a 1465-line impl / 48 methods / **0 tests**; `scene_build/battlefield.rs` 1267 LOC of code splitting into 8 modules with no cycles. |
| D2 | ~~App-to-app edges~~ — **CLOSED (W4).** `client→server` burned by the `battle_host` extraction (#414); `editor→client` burned by `ui_kit` (#424); `APP_TO_APP_ALLOWLIST` is empty (`layer_rules.rs:49`). The upward `scene_build → renderer_api` edge still stands, allowlisted (`layer_rules.rs:41`). |
| D3 | **Passthrough facade** — `client/lib.rs` re-exports 28 `scene_build` items **only for the client's own examples**; several have no consumer at all. |
| D4 | **Dead renderer layer ≈400 LOC** — `RenderBackend`, `WgpuRenderer` (`render_frame` is `Ok(())`), `basic_tank.wgsl`, and four test-only modules (`readback_queue`, `texture_upload`, `render_frame_batch`, `gpu_diagnostics`). **Still open 2026-08-03**: the no-op stands at `renderer_api/src/lib.rs:148` and `renderer_wgpu/src/renderer.rs:83`. |
| D5 | **A lying getter** — `pipeline_registry.rs:20,63`: the field is never incremented, so `compilation_requests_during_draw()` always returns 0. |
| D6 | ~~Dead dependencies~~ — **CLOSED (#407).** `rapier3d` removed; `parry3d` stays and is LIVE — `physics/parry_query.rs` runs the footprint-intersection query, so "zero production uses" was already half wrong when written. |
| D7 | **REWRITTEN 2026-08-03.** `shell` and `experimental_geometry` deleted (2026-08-02, zero dependents). `panel` was deleted with them and then CAME BACK — restored by `073dfe1` with a real consumer (`vehicle_build/src/t54_fender.rs:19`), which is the right way for an orphan to leave the list. `ORPHAN_ALLOWLIST` holds only `quality` itself. Still standing: `kernels/solid` exports 15 `t54_`-named functions — fleet content in a kernel crate. |
| D8 | **The T-54 stack**: 23 dedicated files / ~4200 lines vs Tiger I's 4 / ~510 and T-34-85's 3. Fifteen `if kind == T54_1951` sites. "Hybrid" named CAD+SDF; the SDF half was retired for `cast_loft`, so the name is dead. `t54_hybrid()` re-types 11 numbers that already exist in the RON, and one pair already drifted (`hybrid.rs:296-302`). |
| D9 | **Naming has no rule**: two module conventions inside one crate; three test conventions plus a hidden fourth (`fx/budget.rs`, `fx/contact_lock.rs` are `#[cfg(test)]` with no marker, next to a production `vehicle/damage_budget.rs`); three grouping axes (`packs_german` vs `packs_is3`); `forge` vs `build` undefined; `shell` (CAD) collides with `shell` (projectile). |
| D10 | **Manifests**: `png` declared 8× outside the workspace table, one raw path dep, two manifests not inheriting `version`/`rust-version`. |

## E. Documentation and gate

| # | finding |
|---|---|
| E1 | **`engineering-rules.md` contradicts `verify.ps1`** — five required gates listed, three run; the decision to drop `cargo check` lives only in a script comment. Two of its rules are broken and unmeasured. |
| E2 | ~~`architecture_rules.rs` hard-requires 29 `docs/` paths~~ — **CLOSED (#406).** The doc-path list is gone from the gate. |
| E3 | ~~44% of the gate asserts about markdown~~ — **CLOSED (#406).** The phrase greps went with the doc-path list; the replacement is editorial, not executable: a policy cites its enforcing test. |
| E4 | **`spotting.rs:7-9` warns of an anti-wallhack hole that no longer exists** — the per-viewer filter runs on both the remote and the local path. **Still open 2026-08-03.** |
| E5 | **`weapon.rs` describes the drag ODE as `dv/dt = -c·v`** while the implementation is linear in distance. **Still open 2026-08-03** (the comment now sits at `weapon.rs:108`). |
| E6 | **The blueprint migration lock has no expiry** — `#[cfg(test)]` golden fixtures with "delete once the fleet has lived on RON long enough" and no trigger. |

## F. Adopted from `docs/backlog.md` (file deleted 2026-08-02)

The backlog was a done-ledger from the June reviews; its thirteen still-open boxes were triaged
here rather than deleted with it. Not adopted, with the reason on record: renderer-contract
fiction and the dead draw-counter are D4/D5; real network transport is the W1 production list;
`map_plan`/large-world inertness was superseded by Map Forge (M1–M8); `latest_snapshot()`,
`ServerTickConfig` dedup and the `tools` lib extraction are cosmetic, gone with the file.

| # | finding |
|---|---|
| F1 | ~~Vehicle JSON assets are stale (all eight)~~ — **CLOSED (#428).** `assets/vehicles/*.vehicle.json` regenerated. |
| F2 | **Enemy health bars render through terrain** and show exact HP at any range — no occlusion check; revisit with spotting. |
| F3 | **Tiger II turret ratio may invert the hull-down incentive** — a blueprint *data* question (glacis vs turret), wants a per-vehicle armour-ratio test. |
| F4 | **The sight and meshes ignore terrain tilt** — sim+net+render work; the biggest remaining camera-feel gap on a hilly 1000 m map. |
| F5 | **No boom-length smoothing** — the camera cut against slabs/terrain is exact but instantaneous; wants a critically-damped boom spring. |
| F6 | **Turret yaw is interpolated, not predicted** — hull-only prediction; belongs with W1 1.1. |
| F7 | **glTF `convert` loads no geometry** — rename to "manifest summary" or load buffers (unverified since June). |

---

## Withdrawn after verification

- ~~"Maps-are-data is broken"~~ — measurement error: I counted test fixtures. Real dispatch is **22
  match arms in 3 files**. The doctrine holds.
- ~~"The UDP reassembler can be panicked"~~ — the code is correct; length, magic, `count` clamp,
  `index >= count`, inconsistent count and duplicates are all rejected before any write.
- ~~"The domain vocabulary has drifted"~~ — `cover` vs `scenery` is a principled contract
  distinction, not sprawl. Only `obstacle`/`blocker`/`structure`/`building` are loose.
- ~~"Do not touch the T-54, its program is warm"~~ — Model Idealny **closed 2026-07-29**.
- ~~"Muzzle flash is ≤50 ms late"~~ — **measured at 1 tick = 16.7 ms** locally, over seven
  shots with zero variance.
