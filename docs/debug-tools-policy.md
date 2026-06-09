# Debug Tools Policy

Debug tools are first-week systems, not late polish. They are allowed to be
simple, but they must exist behind stable APIs early so simulation, networking,
renderer, and tools can expose state without ad hoc one-off code.

> Status: this document is the project-level contract **before the full renderer
> exists**. Sections written in the present tense (GPU labels, uncaptured-error
> handling) describe the required behavior once real device creation is wired in;
> today `renderer_wgpu` is a capability-probing shell whose `render_frame` is a no-op,
> so those labels/handlers are intentionally not installed yet.

## Required First-Week Tools

The baseline debug plan includes:

- debug draw line/box/sphere,
- raycast visualizer,
- hitbox/armor plate overlay,
- penetration normal overlay,
- server snapshot inspector,
- entity inspector,
- GPU frame timing,
- CPU profiler markers,
- asset reload,
- free camera,
- network stats overlay.

`renderer_api` owns backend-neutral debug draw commands. The client/editor UI can
decide how to expose inspectors, but the data flow must stay explicit.

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

When real device creation is wired in, `renderer_wgpu` must install an
`on_uncaptured_error` handler and use `push_error_scope` around expected failure
boundaries such as shader hot reload, optional feature probing, and asset upload
validation. Backend code must convert scoped errors into `RenderError` instead
of letting them disappear into logs.
