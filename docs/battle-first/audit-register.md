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
| F9 | **`editor::a_full_recompile_of_the_heaviest_map_fits_the_edit_loop_budget` is load-sensitive** — a hard 250 ms wall-clock ceiling measured in DEBUG, inside a 314-binary workspace run where tests compete for CPU. Failed once at 298 ms during the grass work, passed 3/3 solo (0.15–0.35 s) and on the very next full run. The budget it guards is real and worth guarding; the instrument is not, because a wall-clock assert cannot tell a slow compile from a busy machine. Measure the compile against a relative baseline, or run it in a serial/`--test-threads=1` lane. |
| F8 | **`turret_converges_gun_onto_the_sight_point_not_parallel_to_camera` is FLAKY** — failed once during the Jedna Trawa work (closest 0.0339 against a 0.01 gate) and passed on every re-run (3/3 solo, 2/2 full suite); the failure was unrelated to the change under test. The test's own comment names the cause: it drives a seeded battle whose roster jostles, so a neighbour can move the sight point at the sampling instant. The window-of-closest-approach fix already applied is evidently not wide enough. A gate that fails at random teaches the team to re-run instead of to read — fix or pin the scenario. |

## G. The sight — audited 2026-08-05 (master `5969209`)

A pass over the reticle from the logic down to the pixels, with a rendered contact sheet of every
sight state (`probe -- reticle_strip`) as the visual instrument. The architecture underneath is the
strongest presentation code in the repo — one authoritative trace shared with the server, dispersion
predicted at 60 Hz, the honesty matrix enforced by vertex locks — so every finding here is a
detail on top of a sound frame.

The pattern this section adds to the one at the top of this file: **the sight had three geometries
answering one question.** Each was a good decision; none became the rule, so each patched the
previous one's hole and opened its own.

| # | finding | where |
|---|---|---|
| G1 | ~~The arc test compared a WORLD elevation against HULL-relative gun limits~~ — **FIXED.** A tank nosed down on a ridge read "out of arc" while its gun was on target: the sight lied in exactly the pose hull-down exists for. Now solved through the hull pose by one shared `firing_solution`, which the gun commands also chase. | `hud/reticle.rs:118` |
| G2 | ~~A straight muzzle→aim chord could veto the trace~~ — **FIXED.** A ballistic arc rides ABOVE its own chord for the whole flight (2.7 m at 600 m for a 400 m/s shell), so a crest the chord grazed read as "no shot" while the round would have sailed over. The chord test is deleted. | `hud/reticle.rs:183` |
| G3 | ~~A flat 4.5 m arrival window~~ — **FIXED.** The honest scale is the ARRIVAL ANGLE: a plunging shot lands within centimetres of its solution, a grazing one touches shallow ground metres early (measured: 5 cm of height became 4.1 m of range at 0.013 rad — and the server flies the same trace, so it is a shared artefact, not a preview error). 4.5 m priced the grazing case into the thirty-metre street corner, where it is the whole difference between the window and the wall. | `hud/reticle.rs:7` |
| G4 | ~~The convergence signal died on a wounded gun~~ — **FIXED.** "Aim taken" was measured against the PRISTINE spec dispersion while a damaged gun recovers toward a floor up to 2.5× wider, so from roughly the first tenth of the gun module the ring could never brighten again — and a crew patch returns 25% of the pool, so repair never restored it either. | `app/reticle.rs:215` |
| G5 | ~~The gun marker was slow and nagging~~ — **FIXED.** Its fade band was fixed clip distance. Measured through each view, 0.014..0.030 clip is **8.4–18 mrad in third person (62 deg) but 0.98–2.10 mrad at the 8 deg sniper step and 0.37–0.79 mrad at maximum zoom**, so it stayed lit through the whole exponential tail of the turret's fine lay, most nagging where aiming is most deliberate. The band is now a fraction of the live dispersion ring (floors carry it in third person, where the ring itself is 1.7 px). It was also a *circle* in a sight where every circle means this gun's dispersion; it is a diamond now. | `hud/reticle_marks.rs:31` |
| G6 | ~~The verdict strobed and the mode switch popped~~ — **FIXED.** The marker resolved the honesty matrix at draw time, so it could only snap: green/red flickered at frame rate along a plate edge, and a mode switch swapped colour in one frame while the camera was still travelling into the optics. Eased over ~120 ms, and scaled by the scope dressing so the verdict arrives with the housing. | `hud/reticle_overlay.rs:91` |
| G7 | ~~A settled ring hides inside the crosshair~~ — **CORRECTED AND FIXED.** My own entry had the arithmetic wrong: I read the third-person view as 18 deg, so I put the settled T-54 ring at 0.018 clip against 0.020 arms and called it a near-tie. The view is **62 deg**, so that ring is **0.0048 clip — 1.7 px at 720p** — and the truth was worse than the entry: solid arms ran from the centre out to 0.020 (12 mrad), i.e. ink straight through the ENTIRE useful range of the circle, whose bloom ceiling is 0.028. On top of that the 0.008 hairline floor drew a settled 2.9 mrad gun at 4.8 mrad — the same 67% lie the 0.025 floor was deleted for. Fixed: the floor drops to 0.0035 (degeneracy guard only) and the marker opens a centre gap (arms 0.012..0.024), so the circle has its own space. Consequence worth stating: **the aiming circle is a sniper instrument by arithmetic** — 15 px at the 8 deg step, 40 px at maximum zoom, 1.7 px in third person, where what it reports is bloom and the settled brightening, not fine convergence. | `hud/reticle_marks.rs:50` |
| G8 | ~~The ring's outline is one-sided~~ — **FIXED.** A dark twin on each side; on pale straw the inner edge had no backing at all. The central marker and the blocked form got the same backing while I was there — the glyph the player actually aims with had none. | `hud/reticle_marks.rs:74` |
| G9 | ~~The denial pulse shares a colour with the state it answers~~ — **FIXED by motion, not colour.** Denial-red over loading-red cannot be told apart by hue, so the pulse now opens OUTSIDE the gun's own line, crosses it and collapses inside it at double stroke. Sitting on the arc's radius read as the arc briefly thickening. | `hud/reticle_marks.rs:133` |
| G10 | ~~The readouts have fixed offsets while the ring breathes~~ — **FIXED.** The column starts closer in (clip radius ~0.20, comfortably inside the 0.30 ring where incoming-hit arcs draw — the old 0.18/0.055 spot sat at 0.325, right on top of them) and steps outward only as far as the circle forces it, its own right-aligned width included. Open sky prints no range at all: the sweep now reports whether the sight ray landed on anything, so the ray's own 1200 m reach is never dressed up as a target. | `hud/reticle_readouts.rs:57` · `aim.rs:96` |
| G11 | **Doctrine, not a bug: the ring is the 100% envelope.** The server's shot offset squares a unit draw (`aim_dispersion.rs:65`), so **50% of shots land inside the inner quarter of the drawn radius** and 90% inside 81% of it. The gun therefore feels roughly twice as accurate as the circle promises. A percentile ring would read truer on average but would break the one promise that never breaks today — the shell is never outside the circle — and it would break it loudest. Recorded so the choice is deliberate. | `docs/aiming-model-policy.md` |
| G12 | ~~Unproven: a battle's first seconds may not be deterministic under load~~ — **DIAGNOSED, and it was never about load.** `confirm_garage_selection` builds the battle with `RandomBattleConfig::runtime_from_env`, whose seed is `BattleSeed::runtime()` = `SystemTime::now().as_nanos() ^ pid`. **Every test that deploys draws a different roster, spawn assignment and set of bot routes**, so anything measured downstream of where the tanks stand — the sight point, the aim bloom it commands, how close a converging turret is at tick N — is a coin flip. A busy machine only changes how often the coin lands badly, which is what made it look load-sensitive. The battle is pinned to one seed under `#[cfg(test)]` and locked by `deploying_under_test_is_the_same_battle_every_time` (verified to fail without the pin). This is very likely F8's cause as well — that test's own comment already said "a seeded battle whose roster jostles". | `battle_host/src/battle.rs:21` · `app/garage/actions.rs:241` |

---

---

## H. Contact and tracks program — found by measurement (2026-08-06)

| # | finding | severity |
|---|---|---|
| **H1** | **Bots DO drive themselves into the drowning channel, and the test that promises they do not only looks at two seeds.** `bot_water.rs` opens with "no bot ever drives itself into the drowning channel" and asserts `depth < DROWN_DEPTH_M` (1.5 m) — over seeds 5 and 23. Measured across eight seeds on master, **seed 1234 reaches 2.107 m**, six hundred millimetres past the drowning line, on a live hull. The rest of the population sits at 0.91–1.28 m. This is not new and not caused by the contact work (the same probe reads 2.218 m after it — inside the ±0.11 m the change moves every seed by, in both directions). `terrain/src/ground.rs` already diagnosed the mechanism in its own words: the escape *is* engaged and "simply cannot win: on a descending slick bank, reverse thrust does not overcome gravity plus the water's drag… Extracting it needs the escape to steer ALONG the contour rather than straight back — a control redesign, not a constant." **Repro:** add `1234` to the seed list in `crates/runtime/battle_host/tests/bot_water.rs`. | `battle_host/tests/bot_water.rs:35`, `bot_routes.rs` |

| **H2** | **A hull can be steered INTO its neighbour, and the contact solver cannot stop it.** Reported from the game 2026-08-06 with a screenshot and reproduced: driving alongside and leaning in buries the hull **0.115 m**, leaning on a parked one **0.445 m**, a pivot against one **0.099 m**. Cause: a contact is ONE point, and a hull pinned at one point turns about it freely — the rotation violates nothing there, so the solver applies nothing while a corner elsewhere digs in. Every Wave 1 probe read 0.0000 m on these manoeuvres because they measured the distance between hull CENTRES along one world axis, which cannot see a corner in a flank; `physics::footprint_penetration_m` now exists so no probe repeats that. The fix is a two-point manifold — built and measured at **0.039 m worst case** — but not shipped: a Jacobi pass handed two coupled points destabilises stacks (a queue of three went from 0.0014 m of sink and no motion to 0.110 m and 0.044 m/tick). It needs mass splitting across the contacts holding a hull, or sequential iteration over a deterministically sorted list. **Repro:** `sim/tests/steering_into_a_neighbour.rs`, which ratchets today's numbers. | `physics/src/contact_impulse.rs`, `collision.rs` |

The lesson underneath H1 is the register's own recurring one: a soak that samples two seeds is a
soak that promises a population and checks a pair. The promise is the right one; the sample is not.

And H2 is the same lesson worn the other way round: three separate probes in one programme agreed
on 0.0000 m, and all three were the same wrong ruler. Agreement between instruments that share a
mistake is not evidence.

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
