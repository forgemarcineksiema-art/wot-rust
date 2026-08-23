# Architecture

This workspace starts as a modular native foundation for armored vehicle battles
on large terrain maps, not a general-purpose engine.

## Crates

The workspace is 33 crates in 9 layer folders under `crates/` (`members = ["crates/*/*"]`).
The folder IS the dependency depth, enforced by
`crates/tooling/quality/tests/layer_rules.rs`: nothing depends upward, and an app never
depends on another app. The ranks are foundation 0 → kernels 1 → vehicle/world 2 →
runtime 3 → render 4 → ui 5 → apps 6 → tooling 7; the upward allowlist has exactly one
entry (`scene_build → renderer_api`, a render-layer crate sitting in `world` pending a
move), and the app-to-app allowlist is empty.

`foundation` (rank 0):

- `game_core`: pure gameplay data — tanks, vehicle blueprints, modules, guns, shells, armor
  zones/volumes, damage, shared math (including `GRAVITY_MPS2` and the shell integrator).
- `terrain`: heightmap sampling, terrain chunk data, and the runtime map truth types
  (`BattlefieldMap`, `WaterBody`, cover/scenery/road data) plus the shared grounding helpers.

`kernels` (rank 1) — the renderer-free procedural geometry family:

- `sdf` / `sdf_mesh`: constructive signed-distance fields and their Surface Nets mesher.
- `solid`: constructive convex-solid (CAD/B-rep style) geometry for exact armor plates.
- `revolve`: surfaces of revolution — barrels, road wheels, rollers, sprockets.
- `sweep`: cross-sections swept along path frames — tracks, hoses, rails.
- `cast_loft`: superelliptic station stacks skinned into watertight cast shells (turrets).
- `panel`: thin fabricated plate with edge treatment — deck panels, fenders, grille frames.
- `deform`: bake-only constrained displacement (cast asymmetry, dents, wear).
- `detail`: deterministic semantic detail — bolts, welds, rails, seams, louvres.
- `vehicle_geometry`: the deterministic vehicle mesh kernel — mount frames, unit meshes,
  hitbox-fit validation data.

`vehicle` (rank 2):

- `vehicle_build`: the parametric vehicle-description layer routing `VehiclePart`s to the
  kernels; parts → baked meshes.
- `vehicle_forge`: the authoring/bake pipeline above raw geometry — reference packs, bake
  passes, review artifacts. Renderer-free by rule.
- `vehicle_recipes`: per-vehicle procedural recipes plus the shared family components.

`world` (rank 2):

- `map_forge`: map authoring as data — the RON blueprint schema, structural terrain op
  vocabulary, deterministic blueprint→battlefield compiler, contract report, shipped-map
  catalog, and golden compile hashes. Renderer-free by rule.
- `world_forge`: structures and procedural flora authored like vehicles — parameterised
  generators (`building`, `tree`), golden hashes as the review gate.
- `scene_build`: turns a `BattlefieldMap` or the hangar into renderer-ready meshes
  (terrain, cover, foliage, grass, water, backdrop, weather looks); depends on
  `renderer_api` types only.

`runtime` (rank 3):

- `sim`: deterministic fixed tick simulation — movement, aiming, combat, spotting,
  destruction, replay regression fixtures.
- `physics`: the custom deterministic movement math — SAT footprint collision, terrain
  contact, the support envelope, water wading. No physics engine: `parry3d` survives only
  as the currently uncalled `parry_query` seam (see `docs/physics-policy.md`).
- `net`: binary protocol messages, transport framing, the per-viewer snapshot filter, and
  wire snapshot tests.
- `battle_host`: the authoritative battle loop, local and remote — commands in, fixed
  ticks, filtered snapshots and the reliable personal-event lane out.
- `audio`: the whole audible world as pure DSP — renderer- and device-free.
- `engine`: thin client presentation ECS on `bevy_ecs`; projects the snapshot buffer into
  presentation entities, owns no gameplay truth.
- `timer_resolution`: Windows scheduler-timer resolution for the frame pacer.

`render` (rank 4):

- `renderer_api`: abstract render API — camera, frame, mesh/material handles, HUD vertex
  types, backend trait.
- `renderer_wgpu`: the real wgpu backend — scene/vehicle/sky/water/fx/rain pipelines,
  shadow cascades, bloom, post, WGSL shader ownership.

`ui` (rank 5):

- `ui_kit`: the shared instrument-direction UI kit — theme tokens, clip-space primitives,
  the baked HUD font atlas, icons — used by both client and editor.

`apps` (rank 6):

- `client`: `winit` desktop application — input, local/remote battle flow through
  `battle_host`, prediction/interpolation, renderer and HUD composition.
- `server`: a headless binary shell over `battle_host` (a ~400-line `main.rs`: args, socket
  loop, session wiring).
- `editor`: the map editor — a blueprint authoring shell on the game's own render path
  (viewport via `scene_build` + `renderer_wgpu`, panels via `ui_kit`); sculpt brushes,
  structural op stamps, object/road/water/gameplay tools, the contract report with
  jump-to-problem, and one-click playtest through `MapId::Scratch`.
- `tools`: Rust CLI tools for asset conversion and map data generation.

`tooling` (rank 7):

- `quality`: test-only architecture gate crate; reads the whole tree by definition.

## Module Boundaries

The workspace is intentionally split by ownership, not by convenience. `game_core` owns durable game concepts. `sim` owns deterministic state transitions. `physics` owns the custom deterministic movement math and terrain contact. `net` owns wire compatibility. `renderer_api` and `renderer_wgpu` keep the render API separate from the backend implementation. Client, server, tools, and editor binaries compose these crates without becoming owners of the underlying rules.

Inside crates, modules stay narrow. For example, `sim` is split into:

- `command`: player and AI commands.
- `clock`: fixed simulation/server defaults and simulation clock.
- `timestep`: fixed tick timing.
- `state`: simulation state and stepping.
- `replay`: replay fixture types and regression runner.

Vehicle running gear has one dynamic ownership path. `game_core::TrackShape` owns wheel stations,
shoe/wheel families, and suspension architecture; `vehicle_geometry` derives unit meshes and live
placements; the client instances them on the hull pose. Track backing is part of each animated link,
never fused into the hull, so terrain travel, drive tension, and thrown-track state cannot reveal a
stale second belt. Forge Studio composes those same rest-pose instances into its review images.

## Direction

The current scaffold intentionally keeps the renderer surface/device setup, real UDP transport, asset binary packing, terrain LOD, and full hit detection behind narrow crate boundaries. That keeps the first project state buildable while leaving clear places to expand within armored vehicle battles.

The project optimizes for terrain, LOD, shadows, spotting, shell physics, and networking. It explicitly does not optimize for full indoor AAA streaming, voxel-style full-world destruction, or skeletal-animation-first gameplay. Battlefield destruction — contact-true impacts, visible vehicle damage, destructible cover as selective gameplay state — is in scope and shipped, server-authoritative.

Terrain and large-world policy is fixed early: maps are heightmap/chunk based,
with collision terrain, render LOD, splat maps, roads, cover, spawn/capture
data, navigation, visibility sectors, and minimap data tracked as first-class
systems. Maps are authored as DATA: a RON blueprint per map compiles
deterministically into the `BattlefieldMap` both ends of the wire agree on
(`map_forge`; the map itself never crosses the network — `MapId` identity and a
content hash do). The authoring rules live in `docs/map-forge-policy.md`.
Normal battle maps use `f32`; maps beyond the configured threshold use
origin rebasing instead of leaking `f64` everywhere. Renderer projections follow
WebGPU depth range `[0, 1]`. Details live in `docs/terrain-large-world-policy.md`.

Renderer decisions are tiered by adapter capability, not by a specific developer GPU. The detailed policy lives in `docs/wgpu-capability-model.md`. Shader buffer layout rules live in `docs/wgsl-layout-policy.md`. Pipeline prewarm and cache rules live in `docs/pipeline-policy.md`. Upload batching rules live in `docs/gpu-upload-policy.md`. Procedural vehicle mesh rules live in `docs/vehicle-geometry-policy.md`. Desktop event-loop rules live in `docs/winit-event-loop-policy.md`. Simulation/render clock separation lives in `docs/simulation-render-separation.md`. Physics ownership rules live in `docs/physics-policy.md`.

Debug tooling is part of the initial architecture. Backend-neutral debug draw
and inspectors are defined before the renderer is feature-complete, and
`renderer_wgpu` labels GPU resources for RenderDoc/log validation. The detailed
rule lives in `docs/debug-tools-policy.md`.

The server path exists from the first local build. Client input is encoded as
commands, `battle_host` owns the authoritative battle loop (the `server` app is
a thin binary shell over it), and the client renders interpolated snapshots.
The detailed rule lives in `docs/server-first-policy.md`.

## Quality Gates

Architecture rules are executable — `cargo test -p quality` holds rules about CODE, not
about prose. The old required-docs path list was removed on purpose
(`crates/tooling/quality/tests/architecture_rules.rs:1-4`: "a policy document earns its keep
by being cited from the tests that enforce it, not by a test asserting the file exists").
What the gate actually locks today: the layer DAG and app isolation (`layer_rules.rs`), the
render surface confined to the crates that own it (`architecture_rules.rs` — default-deny
tables for `wgpu`/`winit`/`renderer_wgpu`/`renderer_api`), the parry3d/no-rapier dependency
pin (`parry_feature_rules.rs`), simulation/render separation, identity-enum append rules,
duplication and naming hygiene, and the other rule files under
`crates/tooling/quality/tests/`. The complete local gate is `./scripts/verify.ps1`.
