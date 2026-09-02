# Dolina Bystrej (Bystra Valley)

Fictional Central-European river valley, 1000 m × 1000 m at 5 m samples, mirror-symmetric
across the central axis (`z = 500`). The Bystra river runs **along the axis of advance**,
splitting the map into two flanks with different games; five crossings decide the mid-game.

## Anatomy (west → east)

| x band | Feature | Role |
|---|---|---|
| 0–200 | western upland, mirrored TD knolls (`x≈105, z=500±300`) with stone walls | long-range overwatch of the fields and fords |
| ~250 on-axis | **Windmill Hill** (+11 m dome, windmill on top) | the dominant western high ground |
| ~340/365, z=500±90 | hull-down shelves behind a crest ridge on the river-facing shoulder | mask the hull, work the bridge with the turret (test-locked) |
| 300–520 | open fields, mirrored hedgerow screens | the spotting/positioning game |
| ~520–620 | **the Bystra**: meandering channel (centerline `bystra_river_center_x(z)`), water level 5.0 m | drowning-deep current everywhere except the crossings |
| ~620–650 | floodplain, riverside mill + orchard screens | approach cover to the crossings |
| 650–840 | **town of Kamienna**: 4×3 mirrored block grid on a flattened bench ramp, church + market square on-axis | the brawl; streets hold a constant honest grade |
| 860–1000 | quarry ridge (+9 m), mirrored perches (`x≈915, z=500±160`), quarry bowl on-axis | eastern overwatch and the sheltered rotation |

Flora (Inny Poziom F2, 2026-09-02): the valley plants its horizon's species — 26 oaks
(scatters + Kamienna's fixed trees), 18 willows on the river banks (bank-loving, within 22 m of
the water and never in it), 12 poplars in a loose windbreak beside the valley road, 12 fruit
trees in the orchard behind `orchard_screen`; reed and field bushes as before. The eight
bocage TreeLine boxes are planted (F3) with the mix's poplars — the one species that fills a
16–17 m wall 6 m thick; an oak fits such a wall only at half size — every tree fitted inside
its box over a hedge body. Retired imported kinds are never authored.

## Crossings (all heightmap features, not cover boxes)

| Crossing | Where | Numbers |
|---|---|---|
| Kamienna stone bridge | on-axis, `z=500` | causeway deck 6.4 m (~1.4 m freeboard), parapet walls as cover |
| fords | `z = 500 ± 180` | sill depth ~0.65 m — inside the wading band, slow and exposed |
| plank crossings | `z = 500 ± 320` | deck 5.6 m, 9 m wide — fast early flank rotation near the spawns |

## Generator design

Not a bag of Gaussians (see `crates/foundation/terrain/src/sculpt.rs`): the channel is a
**swept cross-profile along a parametric centerline**, carved to an explicit bed target
`WATER_LEVEL − depth(z)` — so "the current drowns, the ford is fordable" holds by
construction; the town bench **flattens toward an explicit ramp**; crossing decks are
**raised after the carve** so they always clear the water. Mirror symmetry: the centerline
and every mask are even in `z − 500`, features are on-axis or mirrored pairs.

## Contracts (test-locked)

- mirror symmetry of heightmap, spawns, points and cover — `terrain/tests/bystra_map.rs`
- river contract against the REAL physics constants (drowning depth, ford band, deck
  freeboard, no puddles outside the corridor, drivable approaches) —
  `sim/tests/bystra_river_contract.rs`
- 7v7 sets up dry and ticks — `server/tests/bystra_battle.rs`
- three minutes of seeded 7v7 drowns nobody AND still uses the crossings —
  `server/tests/bot_water.rs`. This is the lock that catches a DRIVE-model change from the bot
  side: the escape reads the hull's braking distance, so anything that changes how a hull sheds
  speed changes who drowns. Bot water behaviour lives in `server/src/bot_routes.rs`.

## The sculpt session (teren C3, 2026-08-05) — roads and the floodplain

The default battle map fought ROADLESS for a month (`roads: []`), with an RMS tank-scale
relief of six centimetres. The session gives it:

- **The valley's first road net**: dirt `valley_road` from each spawn to the bridge
  approach; the cobbled `bridge_street` and Kamienna's `town_street` (granite setts — and
  since teren A2 they DRIVE as stone: the paved rotation is the fast one); quarry-gravel
  (`Ballast`) `ford_track` and `plank_track` pairs — Kamienna's quarry is up the hill, and
  a crushed-stone road through a floodplain is exactly what a quarry town lays.
- **The second floodplain terrace** (−0.3 m over a wider band): the valley used to drop a
  metre over sixty and read as a hard bank; the structural moisture rule (teren A1) wets
  the wider lowland into meadow on its own.
- **Meadow swales** (0.7 m, mirrored, west slope) and the **bridgehead bluffs** (1.6 m
  arcs curling around the bridge exit — a real foothold on the far bank).
- **The bridge causeway** — `RoadProfile`'s debut: the approach rides half a metre over
  the floodplain, an earthwork that cannot drift from the street it carries. Its crown
  (teren B2) sheds into the baked normal.
- The `bot_water` lock was renegotiated WITH the measurement that forced it: the old
  far-bank clause passed on a single 4 m vanguard excursion (seed 23, one tick window) —
  a knife-edge any map change flipped. The intent now carries margins: every seed's
  vanguard reaches the crossing corridor (51 m of margin), and at least one seed takes the
  far bank outright (221 m of margin, seed 5 takes the town).

## The W2 approach sculpt (Atlas audit, 2026-08-25)

The terrain atlas (`cargo run --release -p tools -- map-atlas`) read the valley as an OPEN
BOOK: ~10% of standable ground hidden from the enemy half, a 0.3-1.5% hull-down band, 54%
clear sight lines even at 400-550 m - every metre of the march to the crossings under eyes.
The sculpt answers with the grammar a floodplain honestly carries:

- **3 m flood-levees** (mirrored pairs, west of the fords and on the mill side): movement
  masks for the crossing walks, breached by the valley road at its own gate. Measured, not
  guessed: the first cut laid 1.7 m banks and the exposure sweep did not move - the sight
  rule's 0.3 m graze slack plus a 2.53 m turret makes anything under ~3 m nearly
  transparent at range. Crossing a levee anywhere is a slow 0.56-grade climb by design.
- **East-bank lips** (CrestShelf 1.6 m, census-band): the far bank finally answers Windmill
  Hill with hull-down of its own over the bridge and fords.
- **Four bocage TreeLine pairs**: the honest mass-masker - an 8 m hedgerow blocks what no
  tank-scale earthwork can.
- **The market-square capture zone** (745, 500, r 28): the DEFAULT map carries an objective
  at last; the brawl flank holds it, the crossings feed it, the windmill overwatches it.

Re-measured after the sculpt (final geometry): clear-LOS fell across every long band
(200-300: 75.6→70.9%, 300-400: 62.1→57.3%, 400-550: 53.8→48.7%), the hull-down share
grew (S 1.5→1.7%, N 0.3→0.8%), and the fightable-crest inventory is ratcheted at ≥28
(32 measured, `bystra_carries_a_fightable_crest_inventory`). The reticle seam ratchet
holds at its 10-per-ten-thousand ceiling (master baseline 7 — the four 3 m levees cost
~3 poses; falloff 10 and lips at sigma 8 are where the seam and the census stop pulling
the knife apart). Two fixture covenants surfaced and are honoured: the west bocage
dodges BOTH the predictor's warm-up steering circle and the practice-duel corridor at
(340, 300)-(340, 340) — a crushable hedge in either exposed the predictor's
cover-crush seam, filed for the netcode register.

## Status

DEFAULT battle map (the opt-in gate now selects AWAY from it); historical note: playable behind the opt-in gate: set `WOT_MAP=bystra-valley` before launching the client.

The bots respect deep water: the route brain probes its drive line and detours through
`Crossing` points, and the per-tick survival check escapes a hull that is in the channel — or
carries too much momentum to stop short of it. Locked by `server/tests/bot_water.rs`. What is
still open here is HUMAN playtesting; the map has never met a second player.

## Destructible cover (Inny Poziom Z3, 2026-09-02)

68 destructible cover objects on the compiled map — 20 farm buildings, 8 kamienice, 8 tree lines, 2 stone walls, 2 wooden fences, 2 wrecks, 26 oak boles — and the map report refuses a
compile under that floor (`destructible_floor`; `DESTRUCTIBLE_FLOOR` in `map_forge::report`).
Wrecks are damageable since Z3 (500 hp, shelled down to a hull-line mound at 0.45 of their
height, then to scrap); rail covers and crags stay indestructible. The floor is today's count;
the target above it is this dossier's to raise.
