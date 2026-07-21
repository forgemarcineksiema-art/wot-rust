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

## Environment, Materials, Weather

The map's presentation lives in the blueprint the same way its truth does:

- `materials`: the four ground layers (albedo/detail/gloss) + macro-normal and field-patch
  strengths. `scene_build::terrain_material_set_for` binds the palette to the renderer;
  the art-direction ground envelope (vegetation saturation ≤ 0.45, worst-case field-patch
  lift ≤ 0.62, albedo/detail/gloss lanes, straw-over-grass and rock-over-dirt value order)
  is a report Error. A blueprint without the section wears the neutral fallback set.
- `environment`: one look per shipped weather variant, each a **named lighting preset +
  sparse overrides** (fog, exposure, clouds, saturation; sky/rain/wetness per look) —
  never 30 raw knobs. `map_forge` stays renderer-free: `LightingPreset` names are the
  vocabulary, `scene_build` binds them to the hand-tuned `SceneLighting` profiles. The
  FIRST look is the default and the fallback; the server's `supported_weather` reads the
  list in authored order (the order feeds the seeded weather roll). Look coherence
  (non-empty, one look per variant, rain/wetness in 0..1) is a report Error; the
  fog-fairness bound at the 400 m view range stays a `scene_build` lock over the blueprint
  looks (it needs the renderer's fog math): no sky may hide a legitimately spotted target.
- `meta.version` marks the schema: additive sections stay `serde(default)`, a breaking
  document change bumps the version so the editor knows what to migrate.
- World objects own their surface vocabulary: `world_forge::WorldMaterial` (9 semantic
  surfaces with PBR-lite albedo + roughness defaults) — walls, roofs, plinth stone, glass,
  joinery, timber, straw, bark, canopy. The vehicle `MaterialRole` survives only as a
  carrier encoding inside the shared `GeometryVertex`, behind a test-locked bijection;
  consumers decode semantics, never vehicle roles. Buildings keep the honesty rule (mesh ⊆
  collision footprint, rubble form from the same numbers) and their per-instance palettes;
  statics take weather wetness through the same scene-shader lane as vehicles.

## The Editor (planned — M3+)

The interactive editor (`crates/apps/editor`, winit + wgpu viewport + egui through a custom
WGSL painter — no new dependencies) is a blueprint authoring surface: everything it does
writes ops into the document. Live 3D preview through the same render path the client
uses, the layer checklist from `TerrainMapPlan`, undo/redo as an op stack, the contract
report as a jump-to-problem dashboard, and one-click playtest. Details and milestones live
in `docs/map-editor-plan.md`.
