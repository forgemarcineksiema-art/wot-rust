# Orliny Pereval (Eagle Pass)

The third battle map — and the first one born entirely in Map Forge: authored as a blueprint
document, compiled by `map_forge::compile`, gated by the map report, and locked by its golden
hash. Status: **opt-in** via `WOT_MAP=orliny-pereval` (the default battle map stays the Bystra
valley; there is no seeded rotation yet).

## Historical Basis

Caucasus foothills, August–October 1942: the fight for the high mountain passes during
Operation Edelweiss (Klukhor, Marukh, Sancharo). The map is a fictional pass composed for
mirror fairness — no specific engagement is depicted. Naming stays in the register of the
theatre: the pass hamlet is **Orlinoye**, the twin summits are **Sokol** (west) and **Oryol**
(east).

## The Core Idea

Where Prokhorovka is a flat steppe with an embankment and Bystra is a river valley, Orliny
Pereval is a **mountain wall between the teams**. An east–west massif sits exactly on the
mirror axis (z = 500) and is deliberately impassable — its face exceeds the 0.55 climb grade —
except at **three gates**, which are the three lanes:

1. **Dolina lane (west, x ≈ 200)** — a wide, low meadow col (~10 m floor). The fast, open
   flank, overwatched from the Sokol shoulder hull-down shelves and the west rim knolls.
2. **The high pass (center, x ≈ 500)** — a ~31 m saddle carrying the hamlet of Orlinoye and
   the serpentine pass road from both spawns. The brawl lane, and the key to everything else:
   from the col the **crest walk** climbs east and west to the two summits.
3. **The defile (east, x ≈ 840)** — a narrow rocky slot between the Oryol massif and the east
   rim. Short sightlines, wreck-marked mouths — the knife-fight lane.

The signature mechanic: the summits (~65/71 m) stand ON the axis — contested equally — but
are reachable **only through the pass col**. Taking the middle unlocks the high ground, and
the high ground dominates both other lanes (the mountain version of Bystra's "the crossings
decide the mid-game").

## Playable Shape

- Size: 1000 m × 1000 m; height samples 201 × 201 at 5 m; `min_height_m` 0.2.
- Symmetry: `MirrorZ` — heightfield, cover, scenery, and points mirror across z = 500
  (report-enforced).
- **Dry map by design.** The blueprint schema has one global water level; a mountain tarn at
  the pass would flood every lowland. A dry highland also differentiates the map from Bystra
  and skips the whole water contract. (A perched tarn is a schema feature request, not this
  map's problem.)
- Vertical drama: valley floors ~9–10 m, pass col ~31 m, Oryol summit ~71 m — roughly 60 m of
  relief against Bystra's ~25 m.
- Terrain program: `SlopeEases` base, damped `Relief`, a broad `Gauss1` massif swell,
  **`RidgeGated`** (amp 34, σ 30 — the wall; gates at x = 200/500/840), two on-axis `Gauss2`
  summits (+16 each), a saddle fill, two `CrestShelf` hull-down lines, mirrored rim knolls,
  rocky knuckles over the defile (tight `Gauss2` pairs that push the combined slope past the
  splat's rock break, so the slot reads as the stone gate it plays as), a `FlattenToGauss`
  hamlet bench (target 31 m), `ClampMin`.
- Backdrop: `HorizonSpec` with `hills_base_m` 60 (vs Bystra's 27) — a true mountain
  enclosure; no river gap. Its ring is a pine belt at last (`flora`: Pine 0.70, Oak 0.30 —
  Inny Poziom F1; the `pine_belt` view used to show hexagonal oak cones).
- Looks: ClearAfternoon (default), GoldenEvening (alpenglow on the rock), Overcast
  (LeadOvercast). DawnFog stays Bystra's signature. Static weather program (no timeline
  branch).
- Materials: alpine palette — cool meadow green, sun-cured straw, worn dirt, grey granite;
  steep faces auto-break to the rock channel in the splat. `field_patch_strength` 0.55 (no
  worked plots this high).
- Flora (Świat 2.0): procedural oaks on the Dolina floor and crest-walk shoulders (Fixed
  mirrored pair); procedural bushes remain render-only. Retired imported kinds are never
  authored.

## Gameplay Layer

- Spawns: team 1 (500, 140) facing north, team 2 (500, 860) facing south — flat aprons,
  roughly equidistant from all three gates.
- 15 strategic points: Orlinoye hamlet (Observation, axis), Sokol + Oryol summits
  (HighGround, axis), Dolina col + defile gate (Crossing, axis), 4 hull-down shelf points
  (mirrored pairs on the Sokol shoulder and the Oryol face), west rim overwatch pair
  (Observation), defile mouth pair (FlankRoute) — and, since the W3
  counter-perch pass (Atlas audit, 2026-08-25), the EAST rim overwatch pair (Observation,
  930, 220/780): the knolls stand proud of the defile approach and of the bowl toward the
  Oryol face, while the pass col sits beyond the longest parked view range (516 m > 440 m),
  so the side that lost the summits finally has a standoff answer on the defile flank
  without the perch reaching into the middle lane (locked by
  `the_east_rim_answers_the_defile_flank_but_not_the_col`).
- The two on-axis Crossing points name the massif gates for the bots' route planner; the
  water-crossing machinery never fires on a dry map (`bot_routes` helpers return "safe" with
  no water body).
- **The defile's stone (teren W3b, 2026-08-26):** six mirrored `Crag` boxes — the new
  ninth cover kind, the honest big rock (indestructible, uncrushable, jointed-granite bake
  strictly inside the collision box, seated on the lowest footprint corner so slope-standing
  stone never floats). The knife-fight lane finally has walls a flank can lean on, and the
  eye sees the granite the splat always claimed; the gate road corridor stays clear by the
  drive graph's hull margin.
- **The col landmark (teren W3b tail, 2026-08-26):** the Orlinoye watchtower — a Svan-style
  20 m `StoneTower` (the new tenth cover kind) on the axis at the capture point itself. The
  pale limewashed shaft crests the pass skyline from BOTH spawn roads (locked in `sim`
  through the live LOS rule), so the objective draws the eye from the first minute. It is
  destructible masonry (900 HP), and its rubble fraction is 0.11 by the destruction
  doctrine: the felled tower leaves a 2.2 m stump under the benchmark turret line (2.53 m)
  and above the hull-down floor (1.49 m) — the landmark falls into a fighting mound on the
  cap, with both sight lines locked in `sim/tests/orliny_watchtower.rs`. 20 m is the honest
  Svan height and the skyline necessity: a 15.4 m first cut lost "tallest silhouette" to
  the chapel standing on the rising Oryol shoulder (ground 37.3 m vs the 31.0 m bench).
- **The sea of fog (teren W3b tail, 2026-08-26):** a fourth look — `DawnFog`. The
  two-layer fog model's valley haze pools below 12 m, which on this map is exactly the
  valley floors (9–13 m): the spawn aprons and the Dolina meadows drown in dawn mist
  while the pass bench (31 m) and the summits stand clear as islands — you climb OUT of
  the fog toward the fight. Pure data (one `LookSpec`); the fairness sweep bounds the fog
  sum like any other look, and the recorded golden holds the metric gates.
- Cover: the Orlinoye `TownGrid` (12 houses) + on-axis chapel + the watchtower (the
  tallest silhouette of the col), mirrored shepherd barns in the Dolina, serpentine
  retaining walls, summit sangar breastworks, wrecks at the defile mouths, hedgerow
  screens on the Dolina lane.
- Roads (render-only): the mirrored serpentine pass road + the hamlet street, the Dolina
  road, the defile road.

## Contracts

Locked by `crates/world/map_forge/tests/orliny_pereval.rs`:

- `the_wall_is_impassable_between_the_gates` — crossing the massif anywhere between the gate
  skirts exceeds the 0.55 climb grade (the inverse of the usual drivability check: it locks
  the three-lane design itself).
- `the_gates_and_the_crest_walk_stay_drivable` — all three gates and both crest walks stay
  under a 0.5 worst 5 m-step grade.
- `sokol_shoulder_offers_hull_down_over_the_dolina_lane` and
  `oryol_face_offers_hull_down_over_the_pass_approach` — a hull masks (> 0.4 m) while the
  turret clears (> 0.4 m), scanned as a line, not one tuned point.
- `the_summits_are_the_roof_of_the_map` — Oryol crowns the map; the massif carries real
  vertical drama (> 60 m).
- `the_watchtower_crowns_the_col_as_the_tallest_silhouette` — no cover raises a taller
  silhouette from its own footprint (≥ 1.5 m half-height margin), and within the hamlet
  nothing stands higher in absolute terms.

And in `crates/runtime/sim/tests/orliny_watchtower.rs` (through the live LOS rule):

- `the_watchtower_crests_the_skyline_from_both_spawn_roads` — the crown's near arris is
  visible from a commander's eye at either spawn.
- `the_felled_watchtower_leaves_a_hull_down_stump_on_the_col` — intact blocks the cross-col
  lines; felled, the turret line opens over the 2.2 m stump while the hull line stays
  covered.

And in `crates/runtime/battle_host/tests/orliny_battle.rs`:

- `the_bots_march_on_the_pass_instead_of_grinding_the_wall` — the wall has no bot probe the
  way water does, so this locks that after 30 s of battle most of the fleet has left its
  spawn apron instead of grinding the massif.

Plus the shared gates every shipped map passes: the map report (symmetry, spawns, in-bounds,
playability BFS from every spawn to every point), the goldens review gate
(`blueprints/goldens.ron`), determinism, and the weather fog-fairness test across
`MapId::ALL`.

## Asset

```text
assets/maps/orliny_pereval.terrain.json
```

Regenerate with:

```powershell
cargo run -p tools -- generate-map --map orliny-pereval --output assets/maps/orliny_pereval.terrain.json
```

Open in the editor:

```powershell
cargo run -p editor -- crates/world/map_forge/blueprints/orliny-pereval.map.ron
```

Playtest directly:

```powershell
$env:WOT_MAP = "orliny-pereval"; cargo run -p client
```
