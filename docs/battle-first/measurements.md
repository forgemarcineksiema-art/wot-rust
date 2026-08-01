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

## Terrain

All four shipped maps: `size_m: (1000, 1000)`, **`cell_m: 5.0`** → 201² samples. A T-54 is
6.2 × 3.3 m. Going to 2.5 m is 4× the samples; 1.25 m is 16× (642 k floats ≈ 2.5 MB — memory is
not the constraint, meshing, collision and authoring are).
