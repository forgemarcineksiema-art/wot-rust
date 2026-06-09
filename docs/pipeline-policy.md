# Pipeline Policy

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

The project starts with a backend-neutral registry and explicit warmup policy. When real `wgpu::RenderPipeline` creation is added, it should be wired behind this registry instead of being called from the middle of a draw loop.
