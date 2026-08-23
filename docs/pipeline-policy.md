# Pipeline Policy

> **STATUS (2026-08-03): built, not wired.** `renderer_wgpu::PipelineRegistry` with `prewarm()` /
> `require_for_draw()` exists and is tested, but no production code calls it — a grep for
> `require_for_draw` hits only `pipeline_registry.rs`, its tests, and the dead-path
> `renderer.rs` (a `prewarm` grep now also hits the unrelated `scene_build::hangar` bake
> warm-up). The shipping `SceneRenderer` creates its pipelines directly
> (`create_render_pipeline` sites across the fx/rain/scene/sky/vehicle/water/bloom/ground/post/
> shadow/SSAO modules). This document is the contract those call sites adopt when they are wired
> behind the registry; it does not describe today's renderer.

Render pipelines are prepared before gameplay rendering. A draw call may request an existing pipeline by key, but it must not create one on demand.

## Pipeline Key

`renderer_api::PipelineKey` records the render state that changes pipeline compatibility:

- shader;
- vertex layout;
- material flags;
- color format;
- depth format;
- MSAA sample count;
- skinning flag;
- alpha mode.

The key is backend-neutral so gameplay, simulation, networking, physics, replay tests, bots, and the server do not import `wgpu`.

## Runtime Rules

- Startup builds a `PipelineWarmupPlan` with predictable material variants.
- `renderer_wgpu::PipelineRegistry::prewarm()` prepares pipeline keys before draw calls.
- `require_for_draw()` only checks for an already prepared key and returns an error if it is missing.
- Dev hot reload may replace cached keys outside draw calls.
- Release builds use prewarm and may persist a backend cache for faster future starts.

## wgpu PipelineCache

`wgpu::PipelineCache` is a backend implementation detail. It can accelerate render or compute pipeline creation on later runs, but the cache is only meaningful for the same or similar adapter, driver, backend, and limits. It must sit inside `renderer_wgpu` and must not change gameplay-visible state.

The backend-neutral registry and explicit warmup policy exist; real `wgpu::RenderPipeline` creation also exists — outside them. The open work is wiring the shipping renderer's existing creation sites behind the registry, not waiting for new ones; until that lands, this policy is aspiration, not description.
