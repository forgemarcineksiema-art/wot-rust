# Render Boundary

`wgpu` is a backend detail. It must not leak into gameplay, simulation, networking, physics, replay tests, protocol code, bots, tools, or the headless server.

## Hard Boundary

- `game_core` never depends on `wgpu`, `renderer_api`, or `renderer_wgpu`.
- `sim` never depends on `wgpu`, `renderer_api`, or `renderer_wgpu`.
- `net` never depends on `wgpu`, `renderer_api`, or `renderer_wgpu`.
- `physics` never depends on `wgpu`, `renderer_api`, or `renderer_wgpu`.
- `renderer_api` owns render-facing abstractions only: camera data, frame data, handles, render objects, backend traits, and render errors.
- `renderer_wgpu` is the only crate allowed to depend directly on `wgpu`.
- `client` composes windowing, simulation, renderer API, and renderer backend.
- `server` remains headless and does not depend on renderer API, renderer backend, `wgpu`, `winit`, or `egui`.

## Data Shape

Gameplay and simulation state use durable IDs, transforms, and handles:

```rust
pub struct TankRenderLink {
    pub transform: [[f32; 4]; 4],
    pub vehicle_id: u32,
    pub hull_model: MeshHandle,
    pub turret_model: MeshHandle,
    pub material: MaterialHandle,
}
```

They do not own GPU resources:

```rust
// Forbidden outside renderer_wgpu.
pub struct TankGpuResources {
    pub mesh: wgpu::Buffer,
    pub texture: wgpu::Texture,
    pub bind_group: wgpu::BindGroup,
}
```

`renderer_wgpu` translates `MeshHandle` to GPU buffers, `MaterialHandle` to bind groups, and render settings to pipelines. The translation cache belongs to the backend, not to gameplay structs.

## Binding Contract

`renderer_api` owns the backend-neutral binding policy: group 0 is frame/global data, group 1 is camera/view data, group 2 is material data, and group 3 is object/instance data. `renderer_wgpu` may translate that policy into concrete layouts and bind groups, but gameplay crates only see handles and render objects.

The default material path uses a bounded material texture set. Bindless-style resource arrays are optional high-end policy selected through `RenderFeaturePlan`, not an assumption baked into tanks, materials, or asset records.

## WGSL Layout

WGSL buffer layout belongs to `renderer_wgpu`. Gameplay crates do not define GPU buffer structs. Backend buffer records use `bytemuck` only for simple POD data and `encase` for uniform/storage data that must match WGSL alignment and padding. Shader parsing and validation are covered by `naga` tests before shaders are wired into runtime paths.

## Pipeline Ownership

`renderer_api` defines the backend-neutral `PipelineKey`; `renderer_wgpu` translates that key to concrete render pipeline state. Pipelines are prepared through startup prewarm, dev hot reload, or release cache loading. Draw calls may only require an existing key and must not call pipeline creation as a side effect.

## Upload Ownership

GPU upload staging belongs to `renderer_wgpu`. The backend batches frame data through upload arenas, dynamic uniform rings, instance allocators, texture upload queues, and readback queues. Gameplay structs still expose durable handles and transforms only; they do not decide when or how bytes are written to GPU memory.

## Why This Exists

This keeps the authoritative server, protocol snapshots, replay regression tests, bots, asset tools, and simulation benchmarks free from GPU requirements. Even if the project never replaces `wgpu`, the boundary keeps deterministic and headless systems cheap to test and safe to run in CI.

## Enforcement

`cargo test -p quality --test architecture_rules` enforces the policy by checking:

- only `renderer_wgpu` has a direct `wgpu` dependency;
- `game_core`, `sim`, `net`, `physics`, `renderer_api`, and `server` do not reference `wgpu`;
- `server` does not depend on renderer crates, `wgpu`, `winit`, or `egui`;
- this document remains present.
