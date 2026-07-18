# Architecture

This workspace starts as a modular native foundation for armored vehicle battles
on large terrain maps, not a general-purpose engine.

## Crates

- `game_core`: pure gameplay data for tanks, known vehicle profiles, guns, shells, armor, damage, and battle rewards.
- `sim`: deterministic fixed tick simulation for movement, turret rotation, shell state, replay regression fixtures, and hit-detection integration points.
- `net`: binary protocol messages for input commands, scheduled snapshots, prediction, interpolation configuration, and wire snapshot tests.
- `engine`: thin ECS/world utility layer using `bevy_ecs`; it supports tank-battle systems but does not own broad engine ambitions.
- `renderer_api`: abstract render API with camera, frame, mesh/material handles, and backend trait.
- `renderer_wgpu`: WebGPU/wgpu backend shell, adapter capability probing, and WGSL shader ownership.
- `physics`: Rapier collision primitives, heightfield terrain collider creation, and custom tank controller.
- `terrain`: heightmap sampling, signed chunk ids, terrain chunk data, and historical battlefield map profiles.
- `vehicle_geometry`: deterministic procedural vehicle mesh kernel, recipes, mount frames, and hitbox-fit validation data.
- `client`: `winit` desktop application loop wired to input, local/remote server flow, interpolation state, and renderer.
- `server`: headless authoritative simulation library and binary.
- `tools`: Rust CLI tools for glTF conversion and map data generation.
- `editor`: first internal editor shell, ready to grow into egui-based tooling.
- `quality`: test-only architecture gate crate that enforces project structure rules.

## Module Boundaries

The workspace is intentionally split by ownership, not by convenience. `game_core` owns durable game concepts. `sim` owns deterministic state transitions. `physics` owns Rapier integration and the custom tank controller. `net` owns wire compatibility. `renderer_api` and `renderer_wgpu` keep the render API separate from the backend implementation. Client, server, tools, and editor binaries compose these crates without becoming owners of the underlying rules.

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

The project optimizes for terrain, LOD, shadows, spotting, shell physics, and networking. It explicitly does not optimize for full indoor AAA streaming, voxel-style full-world destruction, or skeletal-animation-first gameplay. Battlefield destruction — contact-true impacts, visible vehicle damage, destructible cover as selective gameplay state — is entering scope deliberately, phased and server-authoritative, via `docs/destruction-program.md`. The detailed domain rule lives in `docs/armored-battle-domain.md`.

Terrain and large-world policy is fixed early: maps are heightmap/chunk based,
with collision terrain, render LOD, splat maps, roads, cover, spawn/capture
data, navigation, visibility sectors, and minimap data tracked as first-class
systems. Normal battle maps use `f32`; maps beyond the configured threshold use
origin rebasing instead of leaking `f64` everywhere. Renderer projections follow
WebGPU depth range `[0, 1]`. Details live in `docs/terrain-large-world-policy.md`.

Renderer decisions are tiered by adapter capability, not by a specific developer GPU. The detailed policy lives in `docs/wgpu-capability-model.md`. Shader buffer layout rules live in `docs/wgsl-layout-policy.md`. Pipeline prewarm and cache rules live in `docs/pipeline-policy.md`. Upload batching rules live in `docs/gpu-upload-policy.md`. Procedural vehicle mesh rules live in `docs/vehicle-geometry-policy.md`. Desktop event-loop rules live in `docs/winit-event-loop-policy.md`. Simulation/render clock separation lives in `docs/simulation-render-separation.md`. Physics ownership rules live in `docs/physics-policy.md`.

Debug tooling is part of the initial architecture. Backend-neutral debug draw
and inspectors are defined before the renderer is feature-complete, and
`renderer_wgpu` labels GPU resources for RenderDoc/log validation. The detailed
rule lives in `docs/debug-tools-policy.md`.

The server path exists from the first local build. Client input is encoded as
commands, `server` owns authoritative simulation state, and the client renders
interpolated snapshots. The detailed rule lives in `docs/server-first-policy.md`.

## Quality Gates

Architecture rules are executable. `cargo test -p quality` checks required docs, benchmark files, protocol snapshots, replay fixtures, CI, and the verification script. The complete local gate is `./scripts/verify.ps1`.
