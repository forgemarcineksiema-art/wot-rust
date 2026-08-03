# Debug Tools Policy

Debug tools are first-week systems, not late polish. They are allowed to be
simple, but they must exist behind stable APIs early so simulation, networking,
renderer, and tools can expose state without ad hoc one-off code.

> Status: the renderer is real. The shipping path is `SceneRenderer`/`WindowRenderer`
> with full pipelines (`renderer_wgpu/src/{scene_pipeline,vehicle_pipeline,sky_pipeline,
> water_pipeline,fx_pipeline,rain_pipeline}.rs` plus
> `scene_renderer/{ground,shadow,bloom,post}.rs` and friends). The one no-op left is the
> dead `impl RenderBackend for WgpuRenderer` (`renderer_wgpu/src/renderer.rs:97`) — an
> unused compatibility layer, registered as defect D4, not the shipping path.

## What exists today

- **GPU labels** on persistent wgpu resources (see the rule below).
- **Env knobs** for the lighting/AO stack: `WOT_SHADOW_*`, `WOT_SSAO`,
  `WOT_CLOUD_SHADOWS` (read in `renderer_wgpu/src/scene_renderer/{shadow,ssao,quality}.rs`
  and `client/src/app/session.rs`).
- **The probe binary** (`crates/apps/client/examples/probe/`): staged review renders
  (`tenement_probe`, `factory_probe`, `flora_probe`, `ostrogorsk_views`, `battle_hud`, …)
  and the perf capture (`perf_capture`).
- **`WOT_RECORD`** capture.

## What does not exist (known gaps)

- CPU profiler markers.
- Asset hot-reload.
- A network stats overlay.

The rest of the original first-week list (debug draw line/box/sphere, raycast visualizer,
hitbox/armor plate overlay, penetration normal overlay, snapshot/entity inspectors, GPU
frame timing, free camera) remains the target inventory; `renderer_api` owns backend-neutral
debug draw commands, and the client/editor UI decides how to expose inspectors while keeping
the data flow explicit.

## GPU Labels

Every persistent wgpu resource gets a stable label before it is useful in
RenderDoc, logs, or validation output. Required early labels include:

- `terrain_depth_prepass_pipeline`,
- `tank_pbr_pipeline`,
- `shell_tracer_vertex_buffer`,
- `shadow_map_2048`.

Use lowercase snake case labels that describe the resource role, not an opaque
allocation id. Labels must exist for pipelines, buffers, textures, bind groups,
render passes, and staging/readback resources as they are implemented.

## GPU Errors

wgpu validation errors may arrive synchronously or asynchronously. The renderer
backend treats unexpected uncaptured GPU errors as fatal and wraps expected risky
operations in explicit error scopes.

Open gap: device creation is real, but `renderer_wgpu` installs no
`on_uncaptured_error` handler and uses no `push_error_scope` today (neither call
appears in `renderer_wgpu/src`). The requirement stands: install the handler,
scope expected failure boundaries (optional feature probing, asset upload
validation), and convert scoped errors into `RenderError` instead of letting
them disappear into logs.
