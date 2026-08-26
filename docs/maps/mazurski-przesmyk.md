# Mazurski Przesmyk

**Status: shipped (opt-in).** The fifth map, and two firsts at once: the first `Rot180`
half-turn map (each team sees the SAME battlefield turned around — no wall on the axis, no
reflected halves) and the first map built on the standing-water schema (four named sheets
at two levels; the global table sits under the terrain floor). Plays via
`WOT_MAP=mazurski-przesmyk`.

## Identity

- **Historical basis:** Masurian Lakeland, January 1945 — the fight for the lake defiles
  during the East Prussian offensive. The land bridges between the great lakes (the
  Lötzen Gap pattern) were the armor question of the whole province. A fictional defile
  composed for half-turn fairness; no specific engagement is depicted.
- **Size:** 1000 m × 1000 m at 5 m samples, `Rot180` about the map centre.
- **Reading:** water is the architecture. Two drowning-deep glacial lakes own the NW and
  SE quarters — they DENY movement, they do not host it. Two sunken cut-peat ponds flank
  the central causeway: 47 m of land between two splash hazards, the map's only capture
  zone, and its knife point.

## The half-turn grammar

`MirrorZ` maps fight ACROSS an axis; this map fights ALONG its diagonal. From either
spawn the picture is the same: a great lake on the left hand behind reed belts, open
moraine with an esker spine on the right, and the twin mills of the causeway dead ahead.
Three honest lanes per team:

1. **The causeway** — short, contested, overlooked from both worked shelves; the only
   capture zone. A shell that misses the pinch raises a fountain in the peat ponds.
2. **The shore road** — the long left flank behind the reeds, rounding the near lake to
   emerge at the enemy's far corner (`far_corner_*` observation points).
3. **The moraine lane** — the open right flank under the esker crest, bending into the
   east peat defile where the lake's shore runs the right hand out of land.

The peat-pond defiles (`west_defile` / `east_defile`) stitch the lanes together at
mid-map.

## Water (the standing-sheet showcase)

| Sheet | Rect | Level | Depth (max) | Role |
| --- | --- | --- | --- | --- |
| west lake | (60, 560)–(380, 940) | 6.0 m | ~2.6 m — drowning | denies the NW quarter |
| east lake | (620, 60)–(940, 440) | 6.0 m | ~2.6 m — drowning | rot twin: denies the SE |
| west peat pond | (428, 440)–(494, 560) | 4.0 m | ~2.0 m — drowning | the causeway's west wall |
| east peat pond | (506, 440)–(572, 560) | 4.0 m | ~2.0 m — drowning | the causeway's east wall |

The global `surface_level_m` is 0.0 — under the 0.2 m terrain floor, so every drop of
water on the map is a NAMED sheet. The lake margins pass through the 0.9–1.5 m band (a
real reed marsh: crossable in desperation at a crawl, refused by the bots) before the
drowning line; the report's shoreline gate proves every rect edge dry, so the only way
into a pool is down through its surface.

## Landmarks & cover

- **The twin mills** (`causeway_mill_*`, 15 m) — one at each end of the causeway; the
  pair names the objective from a kilometre out.
- Defile farmsteads (8-house `TownGrid` rot pair), moraine barns, lakeshore alder
  screens (`TreeLine` reed belts), burned hulls at the causeway approaches, and a pair
  of **glacial erratics** as honest `Crag` boulders.
- Scenery: reed `Bush` belts on the dry shore bands, oaks on the eskers, and an erratic
  `Rock` field over the open moraine — every scatter's rot twin dresses the other half
  by the machinery.

## Contracts

Locked by `crates/world/map_forge/tests/mazurski_przesmyk.rs`:

- `the_lakes_drown_and_the_causeway_stays_dry` — both lakes and both ponds reach the
  drowning band through the live resolution rule; the causeway line between the ponds is
  dry its whole length.
- `the_three_lanes_stay_drivable` — causeway, shore road and moraine lane all hold under
  the climb grade, measured as lines.
- `the_mills_flank_the_causeway_as_a_rot_pair` — the signature pair stands where the
  dossier says, as exact half-turn twins.

Plus the shared gates every shipped map passes: the map report (Rot180 symmetry, spawns,
in-bounds, playability BFS from every spawn to every point, the standing-water sheet
contracts), the goldens review gate (`blueprints/goldens.ron`), determinism, and the
weather fog-fairness test across `MapId::ALL`.

## Tools

Open in the editor:

```powershell
cargo run -p editor -- crates/world/map_forge/blueprints/mazurski-przesmyk.map.ron
```

Playtest directly:

```powershell
$env:WOT_MAP = "mazurski-przesmyk"; cargo run -p client
```
