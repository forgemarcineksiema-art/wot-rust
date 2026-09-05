# Pipeline Policy

Render pipelines are prepared before gameplay rendering. A draw call may only use a pipeline
that already exists; it must not create one as a side effect.

## What exists (2026-09-05)

Every `wgpu::RenderPipeline` the game draws with is created in `SceneRenderer::new` — the scene,
vehicle (skin + interior), terrain, sky, water (analytic + refraction), FX, rain, HUD, post,
FXAA, bloom, SSAO (prepass, evaluate, blur) and the three shadow occluder pipelines — before the
first frame, and never during one. `no_gpu_resource_is_created_during_encode` (renderer_wgpu
tests) keeps resource creation out of the encode half of a frame; pipelines are not even
created in the prepare half.

There is no backend-neutral pipeline key, no registry and no warmup plan. `renderer_api::
PipelineKey` / `PipelineWarmupPlan` and the earlier `renderer_wgpu::PipelineRegistry` described
a design nothing adopted; they were removed (the registry in #a96ab7fd, the key types on
2026-09-05) rather than kept as a contract the shipping renderer did not honour.

## Open

- **A pipeline cache.** Every `create_render_pipeline` passes `cache: None`, so every start
  compiles every shader. `wgpu::PipelineCache` is a backend implementation detail, valid only for
  the same or a similar adapter/driver/backend; when it lands it lives inside `renderer_wgpu`,
  persists under a key derived from the adapter, and changes no gameplay-visible state.
- **Startup compile time is unmeasured.** Before a cache is built, measure what the ~20
  pipelines cost on the min-spec at first start.
