# Measurements — 2026-08-01 baseline

Every number here was measured on master `101068a`, not estimated. This is the baseline to compare
against after each wave.

## Simulation cost

`cargo bench -p sim --bench combat_hot_path`, 14 tanks, 128 ticks:

| scenario | total | **per tick** | % of a 60 Hz budget |
|---|---|---|---|
| open battle | 4.50 ms | **35 µs** | **0.21 %** |
| urban, 150 objects | 5.59 ms | **44 µs** | **0.26 %** |

**The simulation uses one four-hundredth of the frame budget.** Whatever stutters is not the sim —
it is the renderer and the bakes. Sim-side optimisation is wasted work.

## Bake cost — and the missing number

`cargo run -p client --release --example perf_capture`:

```
scene bake (ground+statics):      232.9 ms   (ground 56 277 v / 331 464 i;
                                              statics 362 480 v / 538 176 i)
ground maps bake (splat+macro):   107.6 ms
flow field (D8, teren A1):          5.1 ms   (release, Bystra 201²; once per
                                              GroundClassifier construction —
                                              battle setup / bake / client, never
                                              per frame; sort dominates)
ground maps bake, all 4 maps:       1.12 s   (release, teren B2 reference: full
                                              splat+macro pair per map incl. flow
                                              + road-crown distance walks, AABB
                                              gated; ~280 ms/map, absorbed by the
                                              garage prebake path)
water mesh:                         0.7 ms   (2 350 v)
statics rebuild (cover collapse):  27.9 ms
statics rebuild (single collapse):  21.9 ms  (362 382 v)
ostrogorsk statics bake:           38.1 ms   (438 552 v / 660 150 i, 101 boxes + 118 flora)
ostrogorsk rebuild (all-rubble):   49.9 ms
grass conjure:                      352 us/frame (3 653 instances)
church bake:                         0.02 ms (130 tris)
```

**Not one FPS figure. Not one frame millisecond.** The tool named `perf_capture`, cited in
`CLAUDE.md` as the performance measurement, measures bakes. The "one look" policy — MX330 @ 60 FPS,
frame drops are a game bug — has no test and no tool. This blocks any terrain densification.

## Battle pace

Scripted 7v7, Bystra Valley, seed 7, player T-54, full 7-minute timer:

| metric | value |
|---|---|
| damage events | **53** |
| penetrations | **53** |
| **ricochets** | **0** |
| tanks destroyed | 7 of 14 |
| outcome at the timer | `None` |
| first 60 s | 16 hits, 1 death (the player) |
| module hits | 16 of the 24 logged events |
| muzzle-flash latency | **1 tick = 16.7 ms**, n=7, zero variance |

## Cost of change (dispatch sites, tests excluded)

| adding a… | match arms | files |
|---|---|---|
| vehicle | 121 | 17 |
| map | 22 | 3 |
| armor zone | 54 | 5 |
| shell type | 44 | 8 |
| module slot | 33 | 4 |

Adding a vehicle means ~10 parallel tables keyed by the same enum, each maintained by hand. **The
risk is not the edit cost — it is the silent omission** (see register B1/B2).

## Vehicle stack asymmetry

| vehicle | dedicated files | lines |
|---|---|---|
| **T-54** | **23** | **~4 200** |
| Tiger I | 4 | ~510 |
| T-34-85 | 3 | ~90 |

Plus ~110 shared files the T-54 passes through: ~135 files total.

## Workspace shape

32 crates in 8 layer folders · 112 724 LOC `src/` · 34 488 LOC `tests/` · 5 574 LOC `examples/`
(46 examples, 41 of them in `client`) · 219 files with `#[cfg(test)]` · 3 TODO in the whole repo ·
`unsafe_code = "forbid"` · 30 `#[allow]` total.

Largest: `client` 29 861 LOC / 141 files (26 % of all `src/`) · `game_core` 9 910 ·
`vehicle_geometry` 8 936 · `renderer_wgpu` 7 106 · `scene_build` 6 663.

Production panic-capable calls across `server` + `net` + `sim`: **12**, nearly all documented
invariants. `HashMap` in simulation logic: **2**.

## Terrain — prototyped and measured, and it changed the plan

All four shipped maps: `size_m: (1000, 1000)`, **`cell_m: 5.0`** → 201² samples. A T-54 is
6.2 × 3.3 m, so the tank is one cell.

Swept on Bystra through the real compile path, frame cost from `perf_capture`:

| `cell_m` | samples | ground verts | ground indices | **scene work p50** | contracts |
|---|---|---|---|---|---|
| **5.0** | 201² = 40 k | 56 277 | 331 k | **12.52 ms** | pass |
| **2.5** | 401² = 161 k | 176 677 | 1 051 k | **14.83 ms** (+18 %) | pass |
| **2.0** | 501² = 251 k | 266 877 | 1 591 k | **15.99 ms** (+28 %) | pass |
| **1.25** | 801² = 642 k | — | — | — | **FAILS** |

**1.25 m is not a performance question — the map stops being playable.**

```
bystra_valley fails its contracts:
  "strategic point 'windmill_hill' is unreachable from spawn team 1"
```

The passability rule is a **gradient**: `|(there − here) / distance| > CLIMB_GRADE` at 0.55
(`report.rs:185`). A coarse grid *averages* a slope over five metres; a fine grid *resolves* it —
and the same authored hillside contains local pitches above 0.55 that the 5 m sampling was hiding.
Ground that was drivable stops being drivable.

**This inverts the plan's assumption.** Densifying was supposed to *add* places to fight over. It
also adds **walls nobody authored**. It is not a rendering change; it is a re-authoring of every
map's playability.

**2.0 m is the practical cost ceiling**: scene work alone eats 15.99 of the 16.67 ms 60 FPS budget,
on hardware well above the stated minimum spec, before vehicles, HUD, FX or present.

**Third finding: `cell_m` is constrained by the mirror.** 3.33 m and 1.67 m fail a *different*
contract — `heightfield mirror broke` — because `symmetry: MirrorZ` needs a true centre row, so
`1000 / cell_m` must be an even integer. Legal values are 5.0, 2.5, 2.0, 1.25 and nothing between.
That authoring rule was written down nowhere.

**Revised W2.1: 2.5 m, one map, with the playability contracts re-run — and 1.25 m only behind a
sculpt rewrite that targets the gradient, which is its own program, not a step.**

## Terrain — the other half of the ledger, and it reverses the recommendation (2026-08-02)

Everything above measures what densifying COSTS. Nothing measured what it buys. The case for it is
that a tank is one cell at 5 m, so the ground cannot make a fold to hide a hull behind. That is a
claim about relief at tank scale, and relief at tank scale is measurable.

Bystra, both grids, sampled on a common 1 m lattice over the central 800 x 800 m. Local relief at
scale *s* is the height against the mean of four neighbours *s* metres away — RMS over the lattice:

| scale | 5.0 m grid | 2.5 m grid | change |
|---|---|---|---|
| 5 m | 0.061 m | 0.070 m | +15 % |
| 10 m | 0.153 m | 0.165 m | +8 % |
| 20 m | 0.302 m | 0.310 m | +3 % |
| 40 m | 0.667 m | 0.676 m | +1 % |

Read the absolute column, not the percentages. **RMS relief at tank scale is seven centimetres**,
and after densifying it is seven centimetres. Ground with a bump over half a metre at 5 m scale
covers **0.14 % of the map, rising to 0.21 %** — about 900 m² becoming 1 340 m², on a square
kilometre.

**Sampling cannot create what was never drawn.** The compile evaluates authored sculpt ops; halving
the sample spacing renders the same gentle valley more faithfully, it does not add a ditch nobody
cut. What the finer grid does resolve is the EDGES of those ops, which is exactly why 1.25 m breaks
passability: the new steepness is op boundaries turning into walls, not cover turning up.

**The grid was never the limit.** Two cells is the finest feature a grid can hold, so a 5 m grid
represents any crest from ~10 m wavelength upward, at any height. A hull-down position is a
10-15 m crest 1.5-2 m high — comfortably inside that, and the running-gear model already resolves
hull attitude from the support envelope, so a drawn crest gives real hull-down today. Bystra has
almost no tank-scale relief because nobody authored any, not because 5 m could not hold it.

**Recommendation: do not densify. Author the relief.** Densifying pays 1.87 ms of a 16.67 ms frame
— 11 % of the budget, permanently, on every frame of that map — for one centimetre of RMS relief
at the scale that matters, spread uniformly and therefore mostly where no fight happens. The Ridge
brush with its drag tangent, the terrace mode and stroke authoring all shipped with the terrain
programme; a designer can put a 2 m crest exactly where a duel should be fought, for zero frame
cost.

**What would reopen it:** authored relief hitting the 5 m wall — designers wanting folds narrower
than ~10 m. At that point densification has a named purpose and a known place, instead of being a
uniform tax paid in the hope that something useful appears.

## Weakspot smear retirement (2026-08-03, #428)

Probe: deterministic all-T-54 7v7, seeds 11/21/33, 600 s limit, idle player; damage events
deduped by `event_id`. Before = facet multipliers 0.82-0.95 on `hull_front`/`turret_front`;
after = 1.0 fleet-wide (the front's weakness is its PATCHES: mantlet, cupola #426, bow ports
#427).

| | pens | non-pens | ricochets | kills |
|---|---:|---:|---:|---:|
| before | 88 | 40 | 8 | 14 |
| after | 92 | 68 | 13 | 12 |

Frontal exchanges respect armour honestly (+70% non-pens); battle outcome tempo dips mildly
(kills -2/battle at the clock) - the tempo item predates this and stays separately ranked.
Bots aim centre-mass, so the cupola/port hits (1 each across three battles) are incidental;
teaching bot aim to PREFER patches when centre-mass shows no pen is the natural follow-up.

## Bot weakspot aiming (2026-08-03, bots-aim-at-patches)

Probe: `battle_host/tests/probe_weakspot_aim.rs` (committed this time, `#[ignore]`) — mixed-fleet
7v7, seeds 11/21/33, 600 s, idle player, EVERY damage event deduped by `event_id` (so the counts
sit higher than #428's table, which was a narrower filter; before/after below share one method).
Before = centre-of-mass aim (master 2b279e6); after = the gunner switches to the largest
penetrable weakspot disc when the centre shows no pen.

| | pens | non-pens | ricochets | kills | GlacisPort hits |
|---|---:|---:|---:|---:|---:|
| before | 92 | 157 | 13 | 13 | 1 |
| after | 89 | 152 | 9 | **14** | **3** |

Seed 21 is bit-identical before/after — proof the penetrable-centre path changes NOTHING.
The effect is real but modest, and the probe says why: the bounce mass sits on the UPPER GLACIS
of heavies (IS-3, Tiger II, Jagdtiger, Panther II) whose blueprints author NO bow ports yet
(`glacis_ports: [None, None]`), and the dispersion-feasibility line honestly retires a 0.11 m
port past ~250-300 m for 2-3 mrad guns. Bench `random_7v7_tick`: 82.4 -> 87.1 us mid-battle
(+5.7%, near the bench's known 9% spread; 0.5% of the 16.7 ms tick budget).

Follow-ups this measurement names: (a) author the heavies' bow details -> their ports exist to
aim at; (b) the tempo item (all battles at the clock) stays separately ranked.

## Cloud shade: the baked tile's cost, measured A/B (2026-08-04)

`perf_capture` frame section, Bystra Valley 1080p, offscreen, three runs each on the same
machine with `WOT_CLOUD_SHADOWS` as the single variable. Machine under background load, so the
read is the scene-work p50 spread across repeats, not one clean run:

| variant | scene work p50 across 3 runs |
|---|---|
| cloud shade OFF | 23.05 / 22.28 / 20.55 ms |
| cloud shade ON (baked tile) | 17.67* / 22.19 / 23.91 ms |

(*first run of the set, cold caches — read as an outlier.) Both medians sit at ~22–23 ms; the
on/off delta is ~0.5 ms inside the machine's own run-to-run noise (~±1.5 ms). Compare the
procedural implementation's measured +4.837 ms on the same map (`docs/art-direction-program.md`,
the refusal table) — the baked tile delivers the same shade for an order of magnitude less, and
that measurement is what let D21 close with the feature IN the shipped canonical look.
