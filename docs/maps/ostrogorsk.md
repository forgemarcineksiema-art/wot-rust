# Ostrogorsk

The fourth battle map — the urban-map program's city (docs/urban-map-program.md), authored
as a Map Forge blueprint, compiled by `map_forge::compile`, gated by the map report, and
locked by its golden hash. Status: **opt-in** via `WOT_MAP=ostrogorsk`, and **dense-core
stage** (program PR-12): a 93-box city — a 48-block `CityBuilding` tenement grid in cobbled
street canyons plus boulevard-front and back-street ranks, mirrored factory compounds
behind breachable `StoneWall` yards with east gates, three mirrored born-ruin pairs opening
cross-block sightlines, garden walls screening the boulevard seam, lamps and debris through
the core. Outskirts polish (elevator dressing, orchards, berm detail) follows in PR-13.

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
- Flora: orchards and oaks behind the west edge, field brush on both sides of the berm,
  the poplar **boulevard avenue** (mirrored rows with a deliberate gap at the axis road),
  a fixed pine pair at the south elevator.

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

Plus the shared gates every shipped map passes: the map report (symmetry, spawns,
in-bounds, playability BFS from every spawn to every point and the capture zone), the
goldens review gate (`blueprints/goldens.ron`), determinism, and the weather fog-fairness
test across `MapId::ALL`.

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
