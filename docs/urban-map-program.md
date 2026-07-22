# Urban Map Program ("Ostrogorsk") + Imported Flora 2.0

## STATUS (2026-07-22) — read this first if you are picking the work up

**Program A (the map): COMPLETE and merged.** PR-01..PR-15 all landed on master in one day
(#280-#294): the map plays via `WOT_MAP=ostrogorsk` — 101-box dense core, born ruins,
breachable walls, cobbles, battle invariants test-locked (`sim/tests/ostrogorsk_urban.rs`),
perf signed off in `docs/maps/ostrogorsk.md`. What remains is HUMAN work: the playtest loop
in the Map Forge editor (Ctrl+P; re-bless the golden per sculpt iteration). First recorded
sculpt candidate: the berm reads gently from deep east.

**Program B (flora): FL-1..FL-4 COMPLETE and merged** (#295-#298): UV lane, foliage atlas +
alpha cutout honest in color/shadow/SSAO, the CC0 importer (`tools import-flora`, license
gate + loud decimation + multi-texture compositing), runtime catalog + `FloraTree`/
`FloraPine` scenery kinds. Look gate verdict (see `flora_probe` example): tree ACCEPTED,
pine ACCEPTED, bush REJECTED (bad source model — do not author `FloraBush` on maps).

**Remaining, in order:**
1. **FL-5** — author `FloraTree`/`FloraPine` scatters on maps (Ostrogorsk orchards/park
   first, then retrofit), re-bless map goldens, run `perf_capture` at full scatter, keep the
   weather-fairness test green (bushes still never block LOS).
2. **Sourcing** — a real CC0 bush and a TEXTURED birch (first candidates refused by the
   import gate: no TEXCOORD_0). Poly Pizza serves Quaternius GLBs at
   `https://static.poly.pizza/<uuid>.glb` (uuid is in the model page HTML).
3. **Alpha-preserving mip generation** for the runtime atlas (single-mip today).
4. Doctrine follow-ups: map rotation, an Ostrogorsk bot battle test à la `orliny_battle.rs`.

House rules that bit us (do not relearn them): local `scripts/verify.ps1` is the merge gate
(CI billing is blocked) and long verifies need staging (fmt/clippy/test separately);
clippy demands test modules LAST in a file; PowerShell 5.1 mangles UTF-8 via
`Get-Content -Raw` (use the Edit tooling or `[IO.File]::ReadAllText/WriteAllText` with
UTF-8) and its here-strings break on embedded double quotes (commit via `git commit -F`).

This document is the live plan for the game's fourth map — **Ostrogorsk**, an urban/outskirts
hybrid — and for the parallel **Imported Flora 2.0** track that replaces close-range
procedural trees with curated CC0 assets. It extends
[map-forge-policy.md](map-forge-policy.md) (maps are data; append, never reorder) and the
destruction rules of [destruction-program.md](destruction-program.md).

## Why

Map Forge (M1–M8) gives us a complete authoring pipeline, but the content catalog is rural:
buildings are `FarmBuilding` boxes with a style guessed from proportions, roads know only
Dirt and Ballast, the only wall is an indestructible `RailCover`, and there is no street
furniture at all. A real city flank needs masonry blocks, cobbles, breachable garden walls,
ruins, and — before any of that can ship — two engine foundations, because a city triples
the object count the engine was built around:

1. **Statics are one baked mesh.** Every cover box, building, and scenery item bakes into a
   single non-instanced vertex buffer with no culling. Current maps carry ~24–38 boxes; the
   Ostrogorsk core wants 110–140. Draw cost and rebake latency both scale with the whole
   map instead of the visible slice.
2. **Sim cost is O(pairs × boxes).** Spotting LOS re-checks and per-tick collision SAT walk
   every cover box. At 14 tanks × 150 boxes the naive walk erodes the tick budget.

Separately, the 2026-07-22 direction decision: **close-range tree/bush quality comes from
imported CC0 assets, not from more procedural work.** The statics renderer has no UVs and
no textures by design (`scene.wgsl` — "no UVs, no textures, nothing swims"), so the full
variant grows the renderer first. Procedural trees 2.0 remain as the far LOD and fallback.

## Standing decisions

1. **No protocol bump for content-enum appends.** `StaticCoverKind`, `RoadSurface`, and
   `SceneryKind` never serialize onto the wire: both ends compile the same blueprint
   (handshake refuses on `map_content_hash` skew) and cover crosses the wire only as
   index-aligned, kind-agnostic phase bytes. PR-06 lands a round-trip test as the recorded
   proof. Precedent check: v33 was a *removal* (breaking); these are pure appends.
2. **Two new cover kinds**, appended after `WoodenFence`:
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
8. **Street furniture: only `Lamppost` and `DebrisHeap`** (knee-high) as scenery kinds.
   Sandbags/barricades are skipped: anything that *reads* as cover must be a cover box
   (honest-blockers rule), and authored `Wreck`/`StoneWall` boxes already fill that role.
9. **Triangle budgets rise deliberately, per style, with proof.** Tenement/FactoryHall
   ≤ 600 tris; landmark styles (Church) may reach ~1200 once bucket culling has landed.
   Every raise ships with a `perf_capture` measurement on the min-spec machine in the PR
   description (one look: a dropped frame is a game bug, not a player problem).
10. **Imported flora is CC0 only**, with a per-asset manifest (source, URL, author) in the
    repo. No CC-BY — attribution management is a liability we do not take on.

## Map identity

Fictional small railway city on the Voronezh axis, late summer 1943 (a nod to the
Ostrogozhsk–Rossosh operation, fictionalized for mirror fairness like Orliny Pereval).
`MapId::Ostrogorsk`, slug `ostrogorsk`, 1000×1000 m, MirrorZ across z = 500.

- **West flank — the city.** A flat town bench (`FlattenToRamp` + `TownRect` dampen)
  carrying street canyons of 3-storey tenements, the church square on the mirror axis,
  mirrored factory compounds with stone-wall perimeters, ~15% born-ruin blocks opening
  cross-block sightlines. Streets are 12–18 m face-to-face: the playability BFS runs on a
  5 m grid and needs at least two clear cells between opposing walls.
- **East flank — the fields.** Open patchwork with mirrored rises (`Gauss2`, HighGround +
  a sculpted hull-down shoulder) and a north–south **rail embankment** (`Gauss1` on axis X,
  center x ≈ 830) pierced by exactly three gates: the level crossing on the axis and two
  mirrored underpasses. The berm is impassable everywhere else — locked by test.
- **Middle — the boulevard.** The transition strip and the on-axis Crossing fights.
- **Gameplay.** Spawns (500, 110)/(500, 890), mirrored capture zones, 13 strategic points
  covering all five roles (Observation on the church square and grain elevators, Crossing
  at the boulevard/rail gates, FlankRoute through the factory yards, HighGround on the
  rises, HullDown in the berm shadow).

## Program A — the map (15 PRs, 6 waves)

```
W0  Doctrine:      PR-01 this document
WA  Skeleton:      PR-02 registration + playable sparse blueprint (existing vocabulary)
WF  Foundations:   PR-03 sim broadphase + urban_150 bench → PR-04 statics buckets +
                   culling + partial rebake → PR-05 Cobble → PR-06 CityBuilding/StoneWall
                   semantics → PR-07 born ruins
WU  Vocabulary:    PR-08 Tenement → PR-09 FactoryHall → PR-10 StoneWall look + breach →
                   PR-11 Lamppost/DebrisHeap scenery
WM  Dense city:    PR-12 city core (110–140 boxes, ceiling 160) → PR-13 outskirts + rail
WL  Lock:          PR-14 cross-crate battle invariants → PR-15 proofs + doc + perf sign-off
```

PR-02 lands early so the map is playable in the editor from day one; waves F and U gate the
merge of PR-12. Only PR-02/12/13 touch the map goldens; no PR bumps the protocol.

Battle invariants locked in PR-14: LOS blocked through a tenement row and open down the
same street; a scripted row collapse opens the pair over the rubble (destruction changes
the map — as a test, not a slogan); born-ruins present at Rubble in battle; the berm gates
are the only spawn-to-spawn routes east of x ≈ 780; the hot-path bench points at the real
map's box count.

## Program B — Imported Flora 2.0 (5–6 PRs, parallel after PR-04)

```
FL-1  SceneVertex UV lane (append, 52 → 60 B) + texture bind in the scene pipeline;
      existing statics get UV = 0 and render pixel-identical (comparison golden).
FL-2  Alpha-cutout in scene + shadow + SSAO passes (leaf shadow == leaf mask — honesty),
      alpha-preserving mips, foliage sway from the existing `sway` lane.
FL-3  `import-flora` baker in the tools crate: glTF parse, budget validation, quantize to
      SceneVertex+UV, asset format under assets/flora/, CC0 license manifest required —
      an asset without a manifest is rejected at bake time.
FL-4  Curated pack (spruce, birch, linden/chestnut, 2–3 bushes) as appended SceneryKinds;
      LODs: imported LOD0/LOD1, painted frustum stack stays as LOD2. Look gate: side-by-side
      render against trees 2.0, per-species accept/reject.
FL-5  Map integration (Ostrogorsk avenues/park, retrofit of the other three maps) +
      min-spec perf capture at full scatter. Weather-fairness stays green; bushes still
      never block LOS (scenery path).
FL-6  (optional, separate decision) ambient fauna — birdsong + silhouettes.
```

The chunked statics work (PR-04) lands before flora because imported canopies add geometry
that only bucket culling keeps affordable.

## Verification

- Per PR: `scripts/verify.ps1` locally (CI billing is blocked; the local run is the gate).
- Map: `cargo test -p map_forge` (goldens, report contracts, `tests/ostrogorsk.rs`), the
  editor with F5 report + V viewshed, `shell_views` proof renders, Ctrl+P playtest.
- Perf: `combat_hot_path` bench (`urban_150` fixture, then the real map) and `perf_capture`
  on the min-spec laptop at PR-04, PR-12, PR-15 and FL-5 — 60 FPS is a gate, not a wish.
- Look: proof shots plus the close-up review (the model-logic bar) at PR-08/09/10 and FL-4;
  photo references before merge.
