# Ostrogorsk

The fourth battle map — the urban-map program's city (docs/urban-map-program.md), authored
as a Map Forge blueprint, compiled by `map_forge::compile`, gated by the map report, and
locked by its golden hash. Status: **opt-in** via `WOT_MAP=ostrogorsk`, and **dense-core
stage** (program PR-12): a 93-box city — a 48-block `CityBuilding` tenement grid in cobbled
street canyons plus boulevard-front and back-street ranks, mirrored factory compounds
behind breachable `StoneWall` yards with east gates, three mirrored born-ruin pairs opening
cross-block sightlines, garden walls screening the boulevard seam, lamps and debris through
the core. The outskirts are polished (PR-13): crossing parapets high on the berm fill,
complete elevator compounds (head house + silo tower + receiving shed), a far windbreak
pair, the elevator orchard and berm-foot brush.

## Historical Basis

Voronezh axis, late summer 1943: a small railway city behind the Ostrogozhsk–Rossosh line.
The map is a fictional city composed for mirror fairness — no specific engagement is
depicted. Naming stays in the register of the theatre: the city is **Ostrogorsk**, a nod to
Ostrogozhsk without depicting it.

## The Core Idea

Where Prokhorovka is the open steppe, Bystra a river valley and Orliny Pereval a mountain
wall, Ostrogorsk is a **city + open outskirts hybrid** — every playstyle gets its flank:

1. **The city (west, x ≤ ~470)** — a flat masonry bench carrying a street-grid town, the
   church square on the mirror axis, and the mirrored mill compounds on the deep flank.
   Short sightlines, corner fights, the brawl flank.
2. **The boulevard (center, x ≈ 470)** — the north–south seam where the city meets the
   fields; the capture zone sits on the boulevard square, and every rotation between the
   flanks passes through it.
3. **The fields (east, x ≥ ~470)** — open worked patchwork overwatched by mirrored rises
   with a sculpted hull-down shoulder, walled off at x ≈ 830 by the **rail berm**: 7 m of
   fill above the climb grade, passable ONLY at the three gates (the level crossing on the
   axis, two mirrored underpass cuts). The long-range flank, with the grain elevators as
   far-side observation anchors.

The signature mechanic: the berm is the east wall — the eastern routes ARE the gates, and
the wreck-marked level crossing is the axis door both teams can pre-sight.

## Playable Shape

- Size: 1000 m × 1000 m; height samples 201 × 201 at 5 m; `min_height_m` 0.2; dry map.
- Symmetry: `MirrorZ` — heightfield, cover, and points mirror across z = 500
  (report-enforced).
- Terrain program: `SlopeEases` base falling 14 → 8 m west-to-east, damped `Relief`, a
  `FlattenToRamp` city bench (14.2 m at the west edge easing to ~12 m at the boulevard —
  one honest grade under every street), mirrored outskirts rises (`Gauss2`, +6 m) with a
  `CrestShelf` hull-down line on their west shoulders, the berm (`Gauss1` axis X, σ 6,
  +7 m) pierced by three subtractive `Gauss2` gate cuts, `ClampMin`.
- Backdrop: `HorizonSpec` with `hills_base_m` 24 — low steppe enclosure, no river gap.
- Looks: ClearAfternoon wearing **HazyNoon** (dusty summer default), GoldenEvening,
  Overcast (LeadOvercast), RainSqualls (wet streets). Static weather program.
- Materials: dusty late-summer palette — worn verge green, sun-cured stubble, packed
  earth, grey rubble-stone; `field_patch_strength` 0.85 (the east flank is worked right up
  to the berm).
- Flora (FL-5): imported broadleaf trees in the orchard/park behind the west edge and the
  **boulevard avenue** (mirrored rows with a deliberate gap at the axis road), imported
  pines at the south elevator, and procedural field brush on both sides of the berm. The
  rejected imported bush is not authored.

## Gameplay Layer

- Spawns: team 1 (500, 110) facing north, team 2 (500, 890) facing south — on the
  boulevard seam, equidistant from both flanks.
- Capture zone: **town_square** (470, 500) r 30 on the boulevard square.
- 13 strategic points: church square (Observation, axis), boulevard square + level
  crossing (Crossing, axis), underpass pair (Crossing), mill yard pair (FlankRoute),
  outskirts rise pair (HighGround), rise shoulder hull-down pair (HullDown), grain
  elevator pair (Observation).
- Cover (dense core, 93 boxes): the 48-block `CityBuilding` tenement `TownGrid` (four
  columns, six mirrored rows), boulevard-front rank (x 415) and back-street rank (x 130),
  three mirrored born-`ruin` pairs punched into the row lines, the on-axis church, factory
  compounds (`CityBuilding` halls + `StoneWall` yards with east gates + yard wrecks),
  garden-wall runs on the boulevard seam, the crossing wreck pair, field windbreaks, the
  elevators.
- Roads (render-only): the cobbled boulevard (axis), the cobbled high street pair, four
  cobbled row-street pairs through the canyon corridors, the ballast mainline on the berm
  crest, the dirt underpass lane pair.
- Street furniture: a lamppost row on the high street's east walk, debris heaps scattered
  through the core (render-only scenery).

## Contracts

Locked by `crates/world/map_forge/tests/ostrogorsk.rs`:

- `the_berm_is_impassable_between_the_gates` — crossing the berm anywhere between the gate
  skirts exceeds the 0.55 climb grade (the inverse of the usual drivability check: it
  locks the three-gate east flank itself).
- `the_gates_and_the_streets_stay_drivable` — all three gates, the boulevard, the high
  street, a row street and the rail line over the gate stay under a 0.5 worst 5 m-step
  grade.
- `the_city_bench_holds_one_grade` — every long walk across the town rect stays under a
  0.12 step grade: streets never climb stairs.
- `the_rise_shoulder_offers_hull_down_over_the_boulevard_exit` — a hull masks (> 0.4 m)
  while the turret clears (> 0.4 m) on both mirrored shelves, scanned as a line.
- `the_gameplay_layer_mirrors` — every strategic point sits on the axis or carries its
  mirror twin, and the full 13-point layer ships.

And by `crates/runtime/sim/tests/ostrogorsk_urban.rs` (the battle invariants, on the
compiled shipped map):

- `a_tenement_row_blocks_until_it_collapses_and_the_street_always_carries` — a standing row
  blocks the cross-block sightline, the street canyon carries the eye, and a scripted
  collapse opens the TURRET line over the mound while the hull line stays covered
  (destruction changes the map — as a test; the per-kind rubble fraction keeps a felled
  11 m block under turret eyes).
- `the_born_ruins_open_the_city_from_tick_zero` — the three mirrored ruin pairs are rubble
  in the initial states and on the wire.

Plus the shared gates every shipped map passes: the map report (symmetry, spawns,
in-bounds, playability BFS from every spawn to every point and the capture zone), the
goldens review gate (`blueprints/goldens.ron`), determinism, and the weather fog-fairness
test across `MapId::ALL`.

## Performance sign-off (min-spec laptop, release, PR-15)

```text
ostrogorsk statics bake (101 boxes):                    10.2 ms  (88886 v / 153342 i, one-off)
ostrogorsk statics rebuild (all-rubble):                 5.0 ms  (worst case, on the F7 worker)
ostrogorsk statics rebuild (single collapse, 1 bucket):  3.39 ms (the real per-collapse cost)
```

Numbers from `cargo run -p client --release --example probe -- perf_capture`. The sim side is locked
by the `urban_150` bench fixture in `combat_hot_path` (150 boxes > the shipped 101). Review
renders: `cargo run -p client --example probe -- ostrogorsk_views` (street canyon at tank-eye level,
church square, the berm from the fields, and the imported-tree boulevard with the runtime
foliage atlas bound). Known playtest candidate: the berm reads gently from deep east — a
sculpt-session candidate after the first human playtest.

FL-5's full-scatter release capture (2026-07-23, warm median of three runs after the release
build) explicitly counted **118 imported flora instances** in the Ostrogorsk bake:

```text
ostrogorsk statics bake (101 boxes, 118 imported flora): 46.3 ms (438552 v / 660150 i)
ostrogorsk statics rebuild (all-rubble):                 49.0 ms
ostrogorsk statics rebuild (single collapse, 1 bucket):  30.31 ms
```

These are one-off/worker CPU costs, not per-frame work; bucket culling remains the runtime
draw strategy. The same capture keeps the full scatter present instead of benchmarking a
special reduced flora fixture.

The runtime frame gate is separate from those CPU bakes:

```text
NVIDIA GeForce MX330, Vulkan, release, 1920x1080, avenue view, hot 120-frame batches
without imported flora (82,398 statics vertices):  9.483-10.467 ms/frame median
full 118-instance scatter (438,552 vertices):      13.281-14.545 ms/frame median
imported-flora delta:                              +3.798 to +4.077 ms/frame
60 FPS budget:                                     PASS (16.667 ms/frame)
```

Two consecutive runs of
`cargo run -p client --release --example probe -- flora_frame_probe` produced those medians. The probe
warms both renderers, alternates baseline/full order and drains the GPU queue after each batch;
it measures hot frame throughput, while `perf_capture` continues to own the CPU bake/rebuild
numbers above.

## Asset

```text
assets/maps/ostrogorsk.terrain.json
```

Regenerate with:

```powershell
cargo run -p tools -- generate-map --map ostrogorsk --output assets/maps/ostrogorsk.terrain.json
```

Open in the editor:

```powershell
cargo run -p editor -- crates/world/map_forge/blueprints/ostrogorsk.map.ron
```
