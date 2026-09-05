# Debug Tools Policy

Debug tools are first-week systems, not late polish. They are allowed to be
simple, but they must exist behind stable APIs early so simulation, networking,
renderer, and tools can expose state without ad hoc one-off code.

> Status (2026-09-05): the renderer is real. The shipping path is `SceneRenderer` /
> `WindowRenderer` with full pipelines (`renderer_wgpu/src/{scene_pipeline,vehicle_pipeline,
> sky_pipeline,water_pipeline,fx_pipeline,rain_pipeline}.rs` plus
> `scene_renderer/{ground,shadow,bloom,post,ssao}.rs`). There is no second renderer and no
> `RenderBackend` trait any more; `renderer_api` carries the data contract (vertices, frames,
> handles, lighting, culling, capability report), not a catalogue of tools it does not have.

## What exists today

- **GPU labels** on persistent wgpu resources (see the rule below).
- **Env knobs** for the lighting/AO stack: `WOT_SHADOW_*`, `WOT_SSAO`,
  `WOT_CLOUD_SHADOWS` (read in `renderer_wgpu/src/scene_renderer/{shadow,ssao,quality}.rs`
  and `client/src/app/session.rs`).
- **The probe binary** (`crates/apps/client/examples/probe/`): staged review renders
  (`tenement_probe`, `factory_probe`, `flora_probe`, `ostrogorsk_views`, `battle_hud`, …)
  and the perf capture (`perf_capture`).
- **The terrain atlas** (`cargo run --release -p tools -- map-atlas`): top-down instrument
  renders per shipped map (form / ground / drive / tactical / two exposure sweeps) plus
  `atlas.md` stats — drivability by the game's own grade and wading constants, exposure and
  hull-down bands through `sim::line_of_sight` with the T-54 geometry, the sim-vs-mesh
  ground-parity residual, and the engagement-distance profile. Writes `target/map_atlas/`.
- **`WOT_RECORD`** capture.
- **Per-pass GPU timing** (`renderer_wgpu::FrameProfiler`, armed by probes only) and per-pass
  draw/triangle/instance counts on every frame (`PassRecorder`), keyed by `PassId`.

## What does not exist (known gaps)

- CPU profiler markers.
- Asset hot-reload.
- A network stats overlay.

The rest of the original first-week list (debug draw line/box/sphere, raycast visualizer,
hitbox/armor plate overlay beyond the garage's armor inspector, penetration normal overlay,
snapshot/entity inspectors, free camera) remains a target, not an inventory. The
`DebugToolPlan` / `DebugDrawBatch` types that used to enumerate it in `renderer_api` were
removed on 2026-09-05: thirteen "first-week" tools with zero implementations behind them,
locked by a test that compared the list against itself. When a tool lands it brings its own
type; a list of names is not a tool.

## GPU Labels

Every persistent wgpu resource gets a stable label before it is useful in
RenderDoc, logs, or validation output. The labels a capture must show are listed in
`renderer_wgpu::WgpuLabelPolicy::required_startup_labels` (`scene_pipeline`,
`vehicle_pipeline`, `terrain_pipeline`, `shadow_pipeline_scene`, `sun_shadow_map`,
`scene_camera`, `hdr_resolve`, `scene_depth`, `scene_fx_v`), and `tests/gpu_diagnostics.rs`
fails if the source stops giving any of them — the list is checked against the code, not
against itself.

Use lowercase snake case labels that describe the resource role, not an opaque
allocation id. Labels must exist for pipelines, buffers, textures, bind groups,
render passes, and staging/readback resources as they are implemented.

## GPU Errors

wgpu validation errors may arrive synchronously or asynchronously. `GpuContext` installs a
device-lost callback and an uncaptured-error handler at device creation; both LOG rather
than abort, because a shipped game must not crash a player on a transient driver quirk.
No `push_error_scope` is used anywhere in `renderer_wgpu`. `GpuErrorPolicy` describes exactly
this and `tests/gpu_diagnostics.rs` holds it to the source.

A persistently lost surface is a lost device: `SurfaceLossPolicy` reports it as
`RenderError::device_lost` after half a second of `Lost` acquires, the client rebuilds the
renderer on a fresh device once, and a second loss stops the game with a reason
(`client/src/app/render_failure.rs`). A windowed context refuses a CPU (software) adapter.

Open gap: error scopes around expected failure boundaries (optional feature probing, asset
upload validation) would turn a logged validation error into a `RenderError` the caller can
act on. Nothing uses them yet.
