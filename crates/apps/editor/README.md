# WOT Map Editor

A blueprint authoring shell on the game's own render path: the viewport is what the
client renders, the compiled world is exactly what the game would build (every reload is
a full `map_forge::compile`), and every gesture edits ONE RON document with full undo.
On mirror-fair maps every tool stamps the twin BY CONSTRUCTION — fairness is not
optional, and the contract report enforces it live.

```
cargo run -p editor -- crates/world/map_forge/blueprints/bystra-valley.map.ron
cargo run -p editor            # a fresh scratch document (File -> New)
```

## Camera

| Input | Action |
|-------|--------|
| RMB (hold) | look (cursor captured) |
| W A S D / arrows, Q E | fly, down/up |
| Shift | sprint |
| wheel | fly speed |
| N / Shift+N | glide the camera to the next/previous report problem |

## Tools

One tool is armed at a time; the key that armed it cycles its variants, pressing past the
last disarms. `LMB` with no tool armed SELECTS (cover, scenery, points).

| Key | Tool | Gestures |
|-----|------|----------|
| B | sculpt brush (raise/lower/flatten/smooth + macros: ridge/terrace/erode) | LMB paints under the ring; `[` `]` radius, `-` `=` rate; Tab cycles the terrace step (1/2/4 m); ridge follows the drag direction, erode sheds peaks faster than it fills pits; one stroke = one undo step |
| T | structural stamp (hill/bowl/crest/deck) | click 1 anchors, click 2 places; a live ghost previews between clicks; Tab + `-`/`=` tune the inspector knobs; Esc cancels |
| C | terrain stroke (ridge/valley/plateau) | LMB draws the line (chalk + live ghost), Backspace pops, `[` `]` width, `-`/`=` height/depth/target, Enter commits the FITTED curve as a `Stroke` op (mirror twin by construction), Esc clears |
| H | grab (hills, strokes, benches as handles) | LMB holds the nearest form; drag moves (1 m snap, BOTH twins), Shift+drag lifts (0.25 m), `[` `]` width; release sets (one undo step), Esc lets go; no numbers — the ghost is the truth |
| O | object palette (buildings, cover, flora) | LMB places (ghost outline rides the cursor); flora always brings its mirror twin |
| L | road polyline | LMB adds waypoints (ribbon preview), Backspace pops, Tab surface, `-`/`=` width, Enter commits |
| G | gameplay (move spawns, nav points, zones) | LMB places; Tab cycles the nav-point role; fair maps mirror every gesture |
| V | viewshed | dark pads = DEAD ground for a turret at the cursor (hull-downs live in it); V again clears |
| F3 | water panel | `-`/`=` level (0.25 m quanta), X removes; the tint speaks gameplay classes (ford green / marginal amber / drowning red) |

Selection: LMB picks, `R` rotates a cover box (x/z swap — the boxes are axis-aligned by
the collision contract), `Del` deletes (a scatter-born tree becomes an exclusion circle —
the scatter stays procedural), Tab + `-`/`=` tune the inspector knobs, Esc clears.

## Document

| Key | Action |
|-----|--------|
| Ctrl+S | save (canonical RON) |
| Ctrl+Z / Ctrl+Y | undo / redo (every gesture is one step) |
| F5 | full recompile + report |
| F1 | the DOCUMENT panel: all 12 terrain-map layers with live counts |
| Ctrl+P | playtest: save + launch the client on this document (`MapId::Scratch`); refused while the report has Errors |

## Contracts

The report panel shows every Error (blocks shipping and playtesting) and Warning, worst
first, grouped (`xN`); problems with a world position grow pylons in the viewport and `N`
glides the camera to them. The checks cover geometry, presentation AND playability
(drive-graph reachability, named crossings, nav-skeleton density) — see
`docs/map-forge-policy.md`.

## Proof shots

`cargo run -p editor --example shell_views` renders seven PNGs under `target/` — the
shell, the report, sculpting, stamps, objects, water and the gameplay layer — composited
exactly as the live window draws them. They are the look-review artifacts; three real
bugs were caught by them before any player could.
