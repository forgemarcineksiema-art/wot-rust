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
  by the machinery. Inny Poziom F2 (2026-09-02): 28 pines on the eskers and over the
  erratic field, 12 willows on the dry shore band above the reeds, 12 oaks; the reed-belt
  and far-shore TreeLine screens are planted with pines (the species of the mix that fills a
  17 m wall), fitted inside their boxes over a hedge body (F3) — they were "alder screens"
  in name and slabs in the bake.

## Atlas verdict (v1, measured 2026-08-26)

From `cargo run --release -p tools -- map-atlas` — the instrument's numbers, not hopes:
9.4 m relief, 91.7 % comfort grades (no walls: the lakes are the walls), 8.3 % standing
water, sim-vs-mesh parity 0.00 m. Exposure: ~10–12 % hidden, ~1.5 % hull-down, 75 %
clear LOS at 400–550 m. **The open read is honest for a lakeland — the long lines run
over water, and the lakes deny the flanks the way Orliny's massif denies its middle.**

The first verdict here called the hull-down inventory "v1-thin" and promised a ~30-site
crest wave. **The census disproved the promise's premise** (measure > thesis, again): the
same `hull_down_positions` gauge the bots and the report read measures **40 fightable
positions** on this map — more than Bystra's 32 — out of the drumlins, the ridgelets, the
peat-pit rims and the shore banks; the inventory is ratcheted at ≥ 36 in the map's tests.
The thin ATLAS number describes a different thing: the fleet-integrated hull-down BAND,
which stays narrow on any map whose long sightlines run over open water — that is the
lakeland's character, not a missing wave. The three ridgelet pairs from the first
iteration stay as worked approaches; no further sculpt is owed.

## Contracts

Locked by `crates/world/map_forge/tests/mazurski_przesmyk.rs`:

- `the_lakes_drown_and_the_causeway_stays_dry` — both lakes and both ponds reach the
  drowning band through the live resolution rule; the causeway line between the ponds is
  dry its whole length.
- `the_three_lanes_stay_drivable` — causeway, shore road and moraine lane all hold under
  the climb grade, measured as lines.
- `the_mills_flank_the_causeway_as_a_rot_pair` — the signature pair stands where the
  dossier says, as exact half-turn twins.

And in `crates/runtime/battle_host/tests/mazurski_battle.rs` (the map met the bots
2026-08-26 — the dossier's own precondition for the crest wave):

- `a_7v7_sets_up_and_ticks_on_mazurski_przesmyk` — 14 hulls deploy on DRY ground on their
  own diagonal ends and the authoritative loop runs.
- `the_bots_march_between_the_lakes_and_nobody_drowns` — after 60 s of battle most of the
  fleet has left its spawn apron AND nobody, living or wrecked, sits below the drowning
  line. This soak stands on the closed W6.2 seam: the route brain now reads the map's
  COMPLETE water (`bot_routes::water_depth_at` through `water_view()`), so the lakes read
  as deep water and a lake-crossing line detours through the causeway — before the fix the
  table-only read answered "dry" over both lakes (locked, falsified, in `bot_routes`).

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

## Destructible cover (Inny Poziom Z3, 2026-09-02)

30 destructible cover objects on the compiled map — 12 farm buildings, 4 tree lines, 2 wrecks, 12 oak boles — and the map report refuses a
compile under that floor (`destructible_floor`; `DESTRUCTIBLE_FLOOR` in `map_forge::report`).
Wrecks are damageable since Z3 (500 hp, shelled down to a hull-line mound at 0.45 of their
height, then to scrap); rail covers and crags stay indestructible. The floor is today's count;
the target above it is this dossier's to raise.
