# Armored Vehicle Forge Policy

The Armored Vehicle Forge is the authoring-and-bake layer that sits above low-level procedural
geometry. Its goal is World-of-Tanks-beta-like *readable realism*: one benchmark vehicle that reads
as a specific, photo-backed machine — recognizable silhouette, cast turret mass, plate seams, track
recess — without hand-modeling every tank in a DCC tool. Procedural source stays the source of
truth; the runtime renders baked, optimized assets rather than rebuilding tanks each frame.

This policy locks the philosophy. It is the bar every later Forge task is measured against: *does
this move the benchmark vehicle closer to the Forge quality target?*

## Locked Decisions

- **Model pipeline:** procedural source + baked assets + runtime variation. Not runtime full-tank
  generation, and not hand-authored DCC meshes.
- **First quality benchmark:** the **T-54/T-55 family**. One excellent vehicle before shallow
  upgrades to all vehicles.
- **Renderer target:** PBR-lite with baked maps (albedo, normal, AO/roughness, optional cavity).
- **Name:** *Armored Vehicle Forge*. The old flat `VehicleBlueprint` is a prototype stepping stone,
  not the destination model.

## Relationship To Existing Work

- `vehicle_geometry` remains the low-level, renderer-neutral mesh/kernel crate. It is not renamed or
  deleted; the Forge grows on top of it. See [vehicle-geometry-policy.md](vehicle-geometry-policy.md).
- `VehicleBlueprint` (in `game_core`) is the current single shape source of truth for the few
  migrated vehicles. It is treated as a prototype: useful, but gradually superseded by the Forge's
  semantic part graph. It is not ripped out early.
- The pre-Forge lineup screenshots are kept as the comparison baseline. New Forge output is judged
  against them, not against a blank slate. The reference audit lives in
  [vehicle-geometry-photo-analysis.md](vehicle-geometry-photo-analysis.md).

## Ownership And Boundaries

The `vehicle_forge` crate owns the layers above raw mesh construction: reference packs, semantic
vehicle models, bake profiles, and Forge artifacts. It may depend on `vehicle_geometry`,
`game_core`, `glam`, and `serde`.

It must **not** depend on or reference any renderer backend: `renderer_api`, `renderer_wgpu`,
`wgpu`, `winit`, or `egui`. The Forge is an authoring/bake layer, not a renderer layer. This is
enforced by `quality::architecture_rules::vehicle_forge_stays_renderer_free`, which checks both the
manifest and the source. The renderer consumes Forge *artifacts*; it is never consumed by the Forge.

## The Six Layers

1. **Reference Layer.** Collects sources — photos, side/front/top views, dimension data,
   interpretation notes — and records ratio targets per vehicle: length/width/height, track height,
   turret width, gun protrusion, wheel count, mantlet size. Output is a `ReferencePack`: the proof
   of where the proportions come from. Ratio reports state percentage deltas, not just pass/fail, so
   a mesh that is technically valid but *proportionally wrong* still fails.

2. **Semantic Vehicle Model.** Replaces the single flat blueprint with a part graph: hull plates,
   lower tub, sponsons, fenders, track runs, road wheels, turret shell, turret cheeks, mantlet, gun,
   cupola, hatches, hooks, welds. Each part carries a local frame, material role, gameplay role,
   source note, and LOD policy. Mount frames are derived from semantic parts, not hand-typed magic
   values.

3. **Forge Geometry Kernel.** The existing `extrude`/`revolve`/`chamfered_prism` operators stay as
   the foundation. The Forge adds stronger operators: plate boxes with thickness/bevels/normal
   seams, multi-section lofts for hulls and turrets, a cast-shell builder for asymmetric turret
   cheeks, a real track belt, a wheel train (road wheels, idler, drive sprocket, rollers), and
   detail scatter for bolts, hatches, handles, and welds. The kernel emits not just positions but UV
   islands, tangents, material IDs, and bake metadata.

4. **Bake Artifact Layer.** The Forge produces an asset, not just an in-memory mesh. The target
   layout is a manifest (`manifest.json`: vehicle, variant, LODs, materials, source hash), geometry
   (`meshes.bin`), maps (`albedo.png`, `normal.png`, `ao_roughness.png`, optional `cavity.png`), and
   review renders (front, rear, profile, top, battle-oblique). Early bakes may live in memory, but
   the artifact format is designed up front.

5. **PBR-lite Vehicle Renderer.** A separate vehicle pipeline rather than bloating `SceneVertex`.
   `VehicleVertex` (position, normal, tangent, uv, material_id, tint_mask) feeds a shader with normal
   mapping, AO/cavity, roughness specular, and sun + sky fill. Terrain and simple scene meshes stay
   on the existing lightweight path. Armor tint remains a layer over the material, not the vehicle's
   identity.

6. **Runtime Variation Layer.** The runtime adds state to a baked benchmark — hit decals, mud/dust/
   snow, camo/team markings, damaged modules, broken tracks, optional equipment — but it never models
   the full tank. This layer comes only after a stable baked benchmark.

## Gameplay Honesty

A prettier model must not break gameplay:

- Hull and turret/casemate visual bounds stay inside the gameplay hitbox/turret plan.
- The mantlet/gun may protrude, but it has a distinct role.
- Mount frames come from semantic parts, not hand-typed constants.
- Casemate vehicles keep turret yaw ignored.
- The render pose chain stays: hull origin → turret ring → trunnion → muzzle.

## Tests And Gates

Every Forge phase lands with executable checks alongside the prose:

- ratio reports for the benchmark vehicle against its `ReferencePack`, reporting percentage deltas.
- deterministic bakes (stable hashes); source-hash changes are detectable.
- LODs preserve mount frames and hitbox honesty.
- UVs stay inside atlas bounds; tangents are finite and normalized enough for normal mapping.
- the renderer loads vehicle textures and falls back cleanly when a debug texture is missing.
- the review screenshot set contains all required camera views.
- non-Forge vehicles keep rendering through the fallback path until migrated.

The canonical gate remains `./scripts/verify.ps1`. Focused crate tests (`cargo test -p
vehicle_forge`, `-p vehicle_geometry`, `-p renderer_wgpu`) are fine for tight loops, but no phase is
complete until the full gate passes.

## Milestones

0. **Lock the philosophy** — this document; benchmark and baseline chosen. *(current)*
1. **Reference pack and ratio tests** — `ReferencePack` for T-54/T-55, photo-derived ratio tests.
2. **Semantic part graph** — move T-54/T-55 from flat constants to a `ForgePartGraph`.
3. **Geometry operators for real tank forms** — plate/loft/cast-shell/track-belt/wheel-train, UVs,
   tangents.
4. **PBR-lite vehicle pipeline** — `VehicleVertex`, material textures, normal/AO maps, shader path,
   screenshot regression.
5. **Bake artifact and toolchain** — Forge CLI writes artifact folders; client loads baked assets.
6. **First production benchmark** — T-54/T-55 with LOD0/1/2, full screenshot set, passing ratio/
   geometry/renderer/perf gates.
7. **Runtime variation** — decals, dirt/camo, equipment, damage and track state.
8. **Migrate other vehicles** — Jagdtiger, Tiger I, Tiger II, then Panther II after an explicit
   interpretation decision.

Each migrated vehicle must arrive with a `ReferencePack`, part graph, ratio tests, LODs, screenshot
review, and a baked material set.
