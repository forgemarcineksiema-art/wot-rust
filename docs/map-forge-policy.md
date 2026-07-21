# Map Forge Policy

A battlefield is DATA, not code. One RON blueprint per map is the single source of truth;
a deterministic compiler turns it into the runtime `BattlefieldMap` both ends of the wire
agree on; a golden hash is the deliberate-change review gate. This document is the contract
for map authoring, the compiler, and the editor that grows on top of them.

## Ownership

- `map_forge` (`crates/world/map_forge`) owns the blueprint schema, the structural op
  vocabulary, the compiler, the contract report, the shipped-map catalog, the backdrop
  evaluation and the golden hashes. It is **renderer-free** (same rule as `world_forge` /
  `vehicle_forge`, enforced by a quality gate).
- `terrain` owns the runtime truth types (`BattlefieldMap`, `HeightMap`, `RiverSpec`,
  `WaterBody`, cover/scenery/road types) and the shared authoring helpers (`map_build`,
  `math`, `sculpt`, `scenery`) the compiler drives. There is exactly one implementation of
  grounding math in the workspace.
- `MapId` (in `terrain`) stays the wire identity of a map. It is append-only (bincode
  discriminants): a new map appends a variant, a blueprint document, a golden hash and a
  `docs/maps/*.md` page — never reorders. `map_forge::battlefield(MapId)` compiles the
  catalog document; the map itself never crosses the network.

## The Blueprint Document

`MapBlueprint` (RON, pretty, shortest-round-trip floats — hand-editable and
re-canonicalizable by construction):

- `meta`: id, name, historical basis, design notes.
- `grid`: size, cell size, minimum terrain elevation (the final clamp).
- `symmetry`: the fairness contract when the map has one (`MirrorZ`) — checked, not assumed.
- `river`: the centerline as data (`terrain::RiverSpec`); every river-relative thing
  (carve, water mesh, minimap, bots, backdrop) follows the same line.
- `horizon`: the optional backdrop enclosure (rolling hills + closure band + the river's
  exit gap). `None` means the terrain program simply continues (an open steppe).
- `terrain`: the heightfield program — a base surface plus **ordered structural ops**.
  Order is the design: "decks applied last" is visible in the document, not in a comment.
- `water`: standing water level (depth is `level − ground` by construction).
- `objects`: grounded cover boxes and mirrored town grids, in document order.
- `scenery`: seeded, mirrored scatter/row/fixed dressing with declarative exclusion rules.
- `roads`: painted polylines (render-only), explicit or mirrored pairs.
- `gameplay`: spawn zones, strategic points, named features.

### The op vocabulary

Ops are structural — they carry guarantees by construction, like `sculpt.rs` always did:

- `SlopeEases` base, `Relief` (sinusoid terms × damping masks), `Gauss2`/`Gauss1` groups,
  `RidgeGated` (an embankment with crossings), `CrestShelf` (hull-down crest+shelf pairs),
  `FlattenToRamp` (a town bench), `FlattenToGauss` (a quarry floor), `RiverBandAdd`,
  `CarveChannel` (bed = `water − depth(z)`; the depth profile IS the design — drowning-deep
  everywhere except the ford sills), `Deck` (raised after the carve so it always clears),
  `ClampMin`.

The editor's brushes write these ops (quantized), so an edited map stays deterministic and
diff-readable. Undo is popping an op; the document never hides state.

## Determinism And The Review Gate

- `compile(blueprint)` is pure: same document → same map on any machine. The migration gate
  proved both shipped maps bit-identical to the historical generators (40401 height samples
  plus every list entry, per map) before the generators were deleted.
- `MAP_GOLDEN_HASHES` locks every shipped map's compile hash (FNV over the whole
  `BattlefieldMap`). A change here is a deliberate map change — blessed, never an accident.
- Baked assets (`assets/maps/*.terrain.json`) regenerate from the catalog; the migration
  regenerated them with zero differing height samples. (The regenerated files also gained
  the new `river` field and cover entries the committed assets predated — map content added
  after their last bake — so the asset diff is additions only, never a changed sample.)

## The Contract Report

Compilation always returns a `MapReport` next to the map. Entries carry check name,
severity (Error blocks shipping / Warning is deliberate), a message, and a world position
so the editor can jump the camera to the problem. Checks: heightmap sanity, bounds, cover
overlap, spawns (dry, gentle, balanced, clear of cover), roads (bounds, grade), scenery
(grounded, out of cover), mirror symmetry, and the water contract (drowning channel between
crossings, fordable sills, clear decks, no puddles outside the corridor, drivable
approaches) against the gameplay thresholds. `map_forge::battlefield` refuses a map with
errors — a broken map is a build-time bug, never a runtime surprise.

## Environment, Materials, Weather (planned — M2)

The map's presentation moves into the blueprint the same way its truth did:

- `materials`: the four ground layers (albedo/detail/gloss) + macro-normal and field-patch
  strengths; the splat bake and procedural grass already key off map data, and the
  art-direction saturation window becomes a report check.
- `environment`: a default look plus weather variants, each a **named lighting preset +
  sparse overrides** (exposure, fog, sun, clouds, rain, wetness) — never 30 raw knobs.
  `map_forge` stays renderer-free; `scene_build` binds the data to `SceneLighting`. The
  fog-fairness bound at the 400 m view range becomes a report check: no sky may hide a
  legitimately spotted target.
- World objects get their own `WorldMaterial` (albedo + roughness, PBR-lite) in
  `world_forge` instead of borrowing vehicle `MaterialRole`; buildings keep the honesty
  rule (mesh ⊆ collision footprint, rubble form from the same numbers).

## The Editor (planned — M3+)

The interactive editor (`crates/apps/editor`, winit + wgpu viewport + egui through a custom
WGSL painter — no new dependencies) is a blueprint authoring surface: everything it does
writes ops into the document. Live 3D preview through the same render path the client
uses, the layer checklist from `TerrainMapPlan`, undo/redo as an op stack, the contract
report as a jump-to-problem dashboard, and one-click playtest. Details and milestones live
in `docs/map-editor-plan.md`.
