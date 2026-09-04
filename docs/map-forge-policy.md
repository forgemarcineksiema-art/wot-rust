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
- `symmetry`: the fairness contract when the map has one — checked, not assumed. Two
  vocabularies: `MirrorZ` (reflection across the z axis — the WoT-like north/south
  fairness, walls and gates ON the axis) and `Rot180` (half-turn about the map centre —
  each team sees the same map turned around, so diagonal layouts stay fair without the
  wall-on-the-axis grammar). Every pairing rule in the pipeline (roads, town grids,
  scenery pairs, the report's twin hunts, the editor's ghosts and brushes) asks
  `SymmetrySpec::twin`, so both speak everywhere at once. The legacy RIVER machinery
  speaks MirrorZ only (its fairness algebra is even about the axis) — a river on a
  Rot180 map is a report Error; half-turn maps author standing water instead.
- `river`: the centerline as data (`terrain::RiverSpec`); every river-relative thing
  (carve, water mesh, minimap, bots, backdrop) follows the same line.
- `horizon`: the optional backdrop enclosure (rolling hills + closure band + the river's
  exit gap). `None` means the terrain program simply continues (an open steppe).
- `terrain`: the heightfield program — a base surface plus **ordered structural ops**.
  Order is the design: "decks applied last" is visible in the document, not in a comment.
- `water`: standing water — the map-wide table plus bounded `bodies` sheets (teren W6),
  each a rect holding its own level; depth is `level − ground` by construction in both.
  ONE resolution rule (`terrain::WaterView::level_at` — sheets first in document order,
  table as fallback) drives wading, drowning, shell splashes, the water mesh, the
  minimap and the report's drown cells alike. Sheet contracts (report Errors): rects
  in-bounds and non-overlapping, every rect edge DRY under the sheet's level (the only
  way into a pool is down through its surface — what keeps the splash's analytic planes
  complete), the table never wetting ground inside a sheet's rect, and on a fair map
  sheets pair under the symmetry. A sheets-only map (mountain tarns) sets the table
  under the terrain floor.
- `objects`: grounded cover boxes and mirrored town grids, in document order.
- `scenery`: seeded, mirrored scatter/row/fixed dressing with declarative exclusion rules.
- `roads`: painted polylines, explicit or mirrored pairs — and DRIVEN, not render-only:
  since teren A2 the surface routes the ground rule (Ballast/Cobble take the rock lane,
  grip 1.04 / rolling 0.9; Dirt wears to worn earth), so laying or moving a road retunes
  every drive across it (locked by `a_paved_road_is_never_the_slowest_ground`).
- `gameplay`: spawn zones, strategic points, named features.

### The op vocabulary

Ops are structural — they carry guarantees by construction, like `sculpt.rs` always did:

- `SlopeEases` base, `Relief` (sinusoid terms × damping masks), `Gauss2`/`Gauss1` groups,
  `RidgeGated` (an embankment with crossings), `CrestShelf` (hull-down crest+shelf pairs),
  `FlattenToRamp` (a town bench), `FlattenToGauss` (a quarry floor), `RiverBandAdd`,
  `CarveChannel` (bed = `water − depth(z)`; the depth profile IS the design — drowning-deep
  everywhere except the ford sills), `Deck` (raised after the carve so it always clears),
  `ClampMin`.
- `Stroke` (Ręce do terenu W1): a DRAWN line — a fitted polyline with a cross-profile band
  swept along it (`band_mask(distance_to_polyline)` → Ridge / Valley / Plateau). The
  document stores the fitted curve (smoothed, resampled, 0.5 m-quantized by the authoring
  tool; 2..=64 points — a report contract); evaluation stays dumb and pure, and the shared
  `terrain::polyline_distance` walk keeps road paint and stroke terrain from ever
  drifting apart. Because `band_mask`'s support ends exactly at `half_width + falloff`,
  the compiler culls samples outside a stroke's rectangle with bitwise-identical results
  (test-locked); the backdrop skirt evaluates ops directly and stays in agreement.
- **`RoadProfile` (teren C1) promotes that anti-drift from a discipline to a construction**:
  the op names a road and carries NO points of its own — the compiler resolves the polyline
  from the expanded road (`effective_terrain_ops`), so an embankment and the paint it lifts
  cannot diverge even in principle. Both evaluation paths (compile sampling and the backdrop
  skirt) walk the resolved list — the apron seam is test-locked over a profiled edge road.
  An unknown road id evaluates as the identity and errors in the report
  (`check_road_profiles`): the editor survives every keystroke. A `MirroredPair` is profiled
  per expanded twin (`…_south` / `…_north`) — order is the design, one visible op per
  earthwork.

The editor's brushes write these ops (quantized), so an edited map stays deterministic and
diff-readable. Undo is popping an op; the document never hides state.

## Determinism And The Review Gate

- `compile(blueprint)` is pure: same document → same map on any machine. The migration gate
  proved both shipped maps bit-identical to the historical generators (40401 height samples
  plus every list entry, per map) before the generators were deleted.
- `blueprints/goldens.ron` locks every shipped map's compile hash (FNV over the whole
  `BattlefieldMap`) — DATA since M3, so the editor can bless a deliberate map change
  without editing code; the diff review stays. `ServerHello.map_content_hash` (protocol
  v35) carries the same hash to the wire: both ends prove they compiled the same world or
  nobody plays.
- Baked assets (`assets/maps/*.terrain.json`) regenerate from the catalog; the migration
  regenerated them with zero differing height samples. (The regenerated files also gained
  the new `river` field and cover entries the committed assets predated — map content added
  after their last bake — so the asset diff is additions only, never a changed sample.)

## The Sculpt Layer, Stamps And Tools (M4—M7)

- The `sculpt` section (map-editor D1) holds brush strokes as ONE sparse quantized delta
  grid over the terrain program — pointwise, floor-clamped, zero on the border ring (the
  apron seam stays exact by construction), mirrored sample-for-sample on fair maps.
- Structural stamps place quantized `TerrainOp`s (Hill/Bowl, the hull-down Crest, the
  river Deck) into the program; `FlattenToRamp`/`RidgeGated` stay RON-authored — their
  masks are axis-coupled by the fairness design.
- Objects, roads, water and the gameplay layer (spawns, strategic points, capture zones)
  are all document edits; every editor gesture on a fair map lands with its mirror twin
  BY CONSTRUCTION, so the symmetry Error can never fire on a gesture.

## The Contract Report

Compilation always returns a `MapReport` next to the map. Entries carry check name,
severity (Error blocks shipping / Warning is deliberate), a message, and a world position
so the editor can jump the camera to the problem (N cycles, the camera glides). Checks:
grid/sculpt/river coherence, heightmap sanity, bounds, cover overlap, spawns (dry,
gentle, balanced, clear of cover), roads (bounds, grade), scenery (grounded, out of
cover), mirror symmetry, the water contract (drowning channel between crossings, fordable
sills, clear decks, no puddles outside the corridor, drivable approaches) against the
gameplay thresholds, the presentation envelope (ground-albedo window, look coherence) —
and since M7 PLAYABILITY: a coarse drive graph must connect every spawn to every
strategic point and capture zone, a river's sills and decks must be NAMED by Crossing
points, and a starved nav skeleton warns. `map_forge::battlefield` refuses a map with
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
  fog-fairness bound — no sky may hide a legitimately spotted target — stays a
  `scene_build` lock over the blueprint looks (it needs the renderer's fog math). View
  range is per-vehicle (400/440 m today, `TankSpec::view_range_m`), so the bound
  answers to the longest parked range; the lock (`scene_build/src/weather.rs`)
  still asserts at the 400 m figure and owes the 440 m raise.
- `meta.version` marks the schema: additive sections stay `serde(default)`, a breaking
  document change bumps the version so the editor knows what to migrate.
- World objects own their surface vocabulary: `world_forge::WorldMaterial` (9 semantic
  surfaces with PBR-lite albedo + roughness defaults) — walls, roofs, plinth stone, glass,
  joinery, timber, straw, bark, canopy. The vehicle `MaterialRole` survives only as a
  carrier encoding inside the shared `GeometryVertex`, behind a test-locked bijection;
  consumers decode semantics, never vehicle roles. Buildings keep the honesty rule (mesh ⊆
  collision footprint, rubble form from the same numbers) and their per-instance palettes;
  statics take weather wetness through the same scene-shader lane as vehicles.

## Content Vocabulary — Standing Decisions

Absorbed from the urban-map program (Ostrogorsk + Imported Flora 2.0, #280–#298, complete
2026-07-23; the program document is retired — these decisions are doctrine for every map,
not history):

1. **No protocol bump for content-enum appends.** `StaticCoverKind`, `RoadSurface`, and
   `SceneryKind` never serialize onto the wire: both ends compile the same blueprint
   (handshake refuses on `map_content_hash` skew) and cover crosses the wire only as
   index-aligned, kind-agnostic phase bytes. A round-trip test is the recorded proof.
   Precedent check: v33 was a *removal* (breaking); these are pure appends.
2. **`CityBuilding` / `StoneWall` semantics** (appended after `WoodenFence`):
   - `CityBuilding` — 1500 HP, leaves rubble at the standard 0.4 height fraction (a hull
     still stops behind the mound; a turret-height shot clears it). Masonry durability is
     stated in the map document, not inferred from box proportions.
   - `StoneWall` — 150 HP, **crushable** (a 30 t hull breaches a brick garden wall).
     Destroyed or crushed it goes to **Gone** plus a cosmetic knee-high rubble line inside
     the footprint (the felled-tree-line pattern) — never a hull-blocking mound.
3. **Building style hints are explicit id substrings** (extends the church/windmill
   precedent): `"tenement"` → Tenement, `"factory"` → FactoryHall. The proportion heuristic
   stays as fallback (`half.y >= 5.0` → Tenement, existing rules below that).
4. **Ruins are born ruined via id substring `"ruin"`.** A shared `initial_cover_states()`
   spawns the object at Rubble/0 HP on the server and in the client's pre-first-snapshot
   bake. Server-authoritative snapshots make convergence free.
5. **`RoadSurface::Cobble` only.** Granite setts are the 1943 identity; asphalt is wrong
   for the setting and is skipped (append later if ever needed).
6. **Statics strategy: chunked single buffer, not instancing.** Per-building seeds and
   palettes mean every building mesh is unique ("no clones"), so instancing buys nothing.
   Bake into a 4×4 XZ grid of buckets (plus an always-drawn backdrop/skirt bucket) as index
   ranges over one vertex buffer; frustum-cull per bucket AABB; on a cover-phase change
   rebake **only the dirty bucket** on the existing worker.
7. **Sim guardrail: segment-vs-AABB broadphase, not a spatial index.** Prefilter LOS and
   shell-trace slab tests by segment-vs-box XZ overlap, and movement SAT by an XZ distance
   early-out. Deterministic, no data structure, provably result-identical (property test),
   and the `urban_150` bench fixture proves the budget instead of assuming it.
8. **Scenery admits itself by two measured lines, not by a list of allowed names**
   (Skały 1.0, 2026-08-07 — this replaces „only `Lamppost` and `DebrisHeap`", which was a
   roster where a rule belonged). Both numbers come off the vehicles, not off taste:
   - the **belly line** — the fleet's lowest ground clearance, **0.40 m** (the T-34-85, not
     the benchmark T-54's 0.425). Derived in `scene_build::clutter::BELLY_LINE_M` and locked
     by `the_belly_line_is_the_fleet_measurement`, so a lower-slung vehicle moves the rule.
   - the **climb line** — ~0.80 m, the documented vertical obstacle for these tanks
     (`docs/contact-and-tracks-program.md`).

   The rule: **a SOLID scenery object stays under the belly line; only LOOSE dressing may go
   above it.** A hull drives through grass, brush and a spilled rubble heap and nothing lies;
   it cannot drive through a metre of granite, so a boulder that tall is not scenery — it is
   a cover box, and above the climb line it is one that blocks movement too.

   This is the honest-blockers rule with its second half written down. The eye half still
   stands (anything that *reads* as cover must be a cover box); the belly half is what caught
   `SceneryKind::Rock` shipping a **solid 1.24 m** cuboid that blocked nothing. Sandbags and
   barricades stay skipped for the eye reason; authored `Wreck`/`StoneWall` boxes fill that role.
9. **Triangle budgets rise deliberately, per style, with proof.** Tenement/FactoryHall
   ≤ 600 tris; landmark styles (Church) may reach ~1200 now that bucket culling has landed.
   Every raise ships with a `perf_capture` measurement on the min-spec machine in the PR
   description (one look: a dropped frame is a game bug, not a player problem).
10. **Flora is procedural-only — AMENDED 2026-09-02 (route 2, the owner's call after the
    F7 frames: "the trees look tragic, nowhere near real models"): a tree may be DATA.**
    The oak is: a Sapling Tree Gen skeleton grown in Blender 5.2 (`scripts/flora/bake_oak_tree.py`,
    exported per rung to `assets/flora/oak/tree_{near,mid}.bin`), eight leaf-cluster sprites
    rendered by Cycles from in-house leaf geometry (`bake_oak_clusters.py` → the bottom half of
    the foliage atlas), and a CC0 bark tile from Poly Haven (`assets/flora/bark/…/LICENSE.md`)
    projected triplanar onto every bark fragment. Everything embeds in the binary
    (`world_forge::tree::authored`) and every file is hash-locked; the runtime keeps the
    species ladder, the wind, the impostor splat and the honesty boxes. What stays true of the
    original rule: no imported tree MODELS, no glTF pipeline, a licence file for every
    third-party texel. The paragraph below describes the procedural species that remain.
    (Original text:) (Świat 2.0 decision, 2026-08-06: „Florę CC0 usuwamy").
    No imported meshes, no glTF/`.flora.*` pipeline, no license provenance to police — down
    to the last texel: even the leaf masks are baked from SDF composition
    (`world_forge::tree::leaf_atlas`). Since Drzewa 3.0 (2026-08-22) a species is a
    **growth program**: a Weber–Penn-style skeleton table (`tree::skeleton`) grown once as
    pure data, bark swept under the roundness law, and a cluster-card canopy cutting the
    procedural atlas — the LOD rungs and the impostor only FILTER that one growth. The
    battlefield tree is the procedural **oak**: it draws through the instanced LOD ladder
    (`scene_build::tree_lod`, true crossed-quad impostor past 150 m) and its trunk is a
    gameplay solid (`TreeTrunk` cover derived in `map_forge`). The retired `FloraTree` /
    `FloraPine` / `FloraBush` kinds, and since the owner's verdicts `Willow` (2026-09-03)
    and `Pine` (2026-09-04), keep their wire identity (append-only enum) but are
    **never authored** — `map_forge::RETIRED_KINDS` and `flora_integration` lock that.
11. **A map grows its own species, and grows several** (Inny Poziom F2, 2026-09-02). The
    compile report refuses a dressed map (12 trees or more) that plants fewer than three
    tree species, lets one species exceed 70 % of its trees, or names a species in its
    horizon mix (`HorizonSpec::flora`, at 0.10 or more) that stands nowhere inside the map —
    the ring past the border continues the map, it does not invent one
    (`report::check_species_mix`; the goldens test keeps every shipped map dressed). A
    `TreeLine` cover is planted from the same mix (`scene_build::tree_line`, F3 / F7b):
    ladder trees fitted inside their box — tip under the top, crown within 2.5 m of the faces,
    crown inside the ends — so the LOS wall and the trees the eye reads are one object. The
    box is map data and never moves for its dressing.

## The Editor

The interactive editor (`crates/apps/editor`) is a blueprint authoring surface:
everything it does edits the document through one door (`apply_edit` — one gesture, one
undo step) and every viewport reload is a full recompile, so the editor can never show a
world the game would not build. The viewport is the game's own render path; the panels
are the shared instrument UI toolkit `crates/ui/ui_kit` (D3 — one look, one
implementation; since #424 the editor takes it without depending on the client — the
app-to-app dependency allowlist is empty). Playtest
(Ctrl+P) saves the document and launches the client on it through `MapId::Scratch` +
`WOT_MAP=<file>.map.ron` — one local process, one document, and the v35 content hash
guards the remote case. The tool manual lives in `crates/apps/editor/README.md`; the
program's milestones and decisions are in git history (`docs/map-editor-plan.md`, removed
after completion — M1–M8, PRs #258–#268).
