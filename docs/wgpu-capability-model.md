# wgpu Capability Model

`wgpu` is the Rust implementation of the WebGPU programming model. It can run on native backends such as Vulkan, Metal, D3D12 and OpenGL, and on browser WebGPU/WebGL paths, but the renderer policy treats WebGPU as the API model.

The renderer starts from `wgpu::Instance`, probes an adapter, and records a backend-neutral report in `renderer_api::RenderAdapterReport`. Later surface and device ownership must stay inside `renderer_wgpu`; gameplay and simulation only see renderer API handles and capability tiers.

## Startup Report

At renderer startup, the client logs:

- adapter name;
- backend: Vulkan, DX12, Metal, GL, browser WebGPU, noop, or other;
- device type: discrete GPU, integrated GPU, virtual GPU, CPU, or other;
- selected capability tier;
- supported feature names used by the renderer policy;
- relevant limits summary;
- BC, ETC2, and ASTC texture compression support;
- timestamp query support.

The report is backend-neutral so the client can log it without importing `wgpu`.

## Capability Tiers

- `Tier0`: compatibility path for weak desktop GPUs, CPU/fallback adapters, and low limits. Keep terrain and materials conservative.
- `Tier1`: default PC gaming path. Use normal materials and draw organization with WebGPU-default style limits.
- `Tier2`: stronger GPU path. Requires stronger limits, texture compression support, and timestamp queries; use this for heavier diagnostics and richer render features.
- `Experimental`: opt-in future/native path. This is for feature names marked with the `experimental:` prefix and must never become a silent default.

The renderer chooses behavior from `RenderCapabilityTier`. It must not branch on a specific vendor, adapter name, or one local RTX-class machine.

## Lighting Quality Table

Per-adapter-class lighting knobs live in one backend-neutral table, `renderer_api::LightingQuality`
(`for_device_type`): near shadow-cascade resolution (the far cascade derives half), cascade count,
SSAO render scale (half resolution on integrated/software adapters — including the depth prepass,
the real cost), and whether terrain runs cloud shadows. `renderer_wgpu` maps its adapter type onto
the table and applies the `WOT_SHADOW_RES` / `WOT_SHADOW_CASCADES` / `WOT_SSAO=off|half|full` env
overrides in exactly one place (`scene_renderer::quality`), replacing per-feature resolver
functions scattered through the passes. The tier values and a per-tier lighting memory budget at
1080p are locked by tests; a future settings menu drives the same struct.

## Feature Fallback Policy

The baseline render plan is intentionally boring and cross-platform:

- forward+ rendering;
- shadow maps;
- terrain chunks;
- instancing;
- LOD;
- frustum and occlusion culling;
- particles;
- postprocess;
- UI;
- debug draw.

High-end features are optional requests, not foundations:

- ray tracing or ray queries fall back to shadow maps;
- mesh shaders fall back to instancing;
- bindless-style resource arrays fall back to the normal forward+ material path;
- GPU-driven rendering falls back to CPU-side frustum culling and instancing.

`renderer_api::select_render_feature_plan()` owns this policy. `renderer_wgpu` maps native `wgpu` feature flags into backend-neutral feature names, including `experimental:ray-query` and `experimental:mesh-shader`, then stores the selected `RenderFeaturePlan`. Experimental paths only activate from explicit experimental feature names; otherwise they are recorded as fallbacks.

## Bind Group Policy

The baseline renderer uses four stable bind group slots:

- group 0: frame/global data;
- group 1: camera/view data;
- group 2: material data;
- group 3: object/instance data.

This is the default `renderer_api::baseline_bind_group_layout()`. The renderer must not create a separate bind group for every small mesh part, tank, bolt, or texture. Materials should batch through the material slot and object transforms should flow through object/instance data.

The baseline texture strategy is `BoundedMaterialSet`, not full bindless. Bindless-style arrays and large texture arrays require `RenderFeature::BindlessResources` in the selected feature plan. That feature only enables when the backend reports the required texture/storage binding array support and `partially-bound-binding-array` on an experimental tier; otherwise the plan records a fallback to the normal forward+ material path.

## Pipeline Preparation

Pipeline creation is not part of gameplay rendering. The renderer prepares predictable `PipelineKey` variants before draw calls, including shader, vertex layout, material flags, color/depth formats, MSAA, skinning, and alpha mode. Dev builds may hot reload these variants outside the draw loop. Release builds use explicit prewarm and can later persist a `wgpu::PipelineCache` for the same or similar device.

## Upload Strategy

GPU upload is batched by renderer-owned systems, not by individual gameplay objects. Tanks, foliage, props, particles, track marks, and effects are collected into instance batches and staged through arenas or upload queues. Uniform data uses dynamic ring-buffer offsets, texture data goes through a texture upload queue, and readbacks are queued separately from rendering.

## Requested Device Limits

Do not use `adapter.limits()` as the default device request. Adapter limits describe what the GPU can do, not what the game needs. Requesting more than the adapter supports fails, and requesting more than the renderer needs can hurt performance.

The backend exposes explicit request profiles:

- `renderer_low_spec_limits()`: minimal startup profile, based on WebGL2/downlevel-style limits and used by default.
- `renderer_recommended_limits()`: normal desktop profile for the current renderer.
- `renderer_high_limits()`: opt-in stronger profile for richer rendering paths.

`RenderSettings::default()` starts with `RenderLimitProfile::LowSpec`. Tests in `renderer_api` and `renderer_wgpu` assert that the renderer can start with that profile and that low-spec limits are not copied from a strong adapter.

## Resource Ownership

`wgpu` handles are reference-counted and cloneable, and dependent resources keep their parents alive as needed. That does not make them gameplay state. GPU resources still belong in `renderer_wgpu` caches:

- `MeshHandle` maps to GPU buffers;
- `MaterialHandle` maps to bind groups and texture views;
- render settings and tiers map to pipelines and optional features.

Gameplay, simulation, networking, physics, replay tests, tools, and the server stay GPU-free.
