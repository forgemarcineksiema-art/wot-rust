# WGSL Layout Policy

WGSL data layout is a renderer backend contract, not a visual detail. Rust structs sent to uniform or storage buffers must be laid out with WGSL memory rules in mind, and every shader added to `renderer_wgpu` must be parsed and validated in tests.

## Rules

- Use `bytemuck` for simple POD data such as vertex buffer records.
- Use `encase` for uniform and storage buffer structs that must match WGSL alignment and padding rules.
- Use `naga` to parse and validate WGSL shaders in automated tests.
- Do not rely on `#[repr(C)]` alone for uniform or storage buffer compatibility.
- Do not hand-pack larger uniform/storage structs with ad hoc byte offsets.
- Keep shader binding groups aligned with the layouts the pipelines are built from: group 0 is
  the camera uniform plus the armor-damage storage buffers (`build_camera_bind_group_layout`),
  group 1 is per-pipeline material data (the foliage atlas for the scene pipeline, the vehicle
  material families for the vehicle pipeline, the ground maps for the terrain pipeline), group 2
  is the shared environment (`build_shadow_bind_group_layout`: both shadow cascades, the AO
  target, the cloud tile, the reflection cube).

## Current Baseline

- `SceneVertex` is POD (`bytemuck`): eight fields, 15 floats, 60 bytes — `position`, `normal`,
  `color`, `tint_weight`, `gloss` (materials v2), `surface` (surface-role lane), `sway` (wind
  lane), `uv` (foliage-atlas lane); `renderer_api/src/scene.rs:41-70` locks the size. Grown ONLY
  by appending: the mesh occupies vertex attribute locations `0..=3` plus the appended `9..=12`
  (`scene_pipeline.rs::VERTEX_ATTRIBUTES`) — a reorder would corrupt every pipeline at once.
  `tint_weight` selects how much of the per-instance team tint multiplies the base color (`0.0`
  absolute, `1.0` fully tinted), so one team-neutral mesh serves every team color.
- `SceneInstance` is POD (`bytemuck`): the per-object `model` matrix plus a `tint` color, bound as a
  second instance-step vertex buffer at locations `4..=8`.
- `CameraUniform` is serialized through `encase::UniformBuffer`.
- Every shipped shader is validated by `naga` in `renderer_wgpu/tests/wgsl_layout.rs`
  (`every_shipped_shader_validates`). The dead-path `basic_tank.wgsl` and its `TankVertex` were
  removed on 2026-09-05; nothing drew with them.
- The camera uniform lives in group 0, binding 0 (`camera_common.wgsl`), in every pass.
- `CameraUniform.time_params.x` is the presentation clock every shader animation reads
  (water ripple, foliage sway, weather). It is **tick-domain by doctrine**: fed from the fixed
  simulation tick plus the sub-tick render phase, never integrated from render-frame deltas — a
  jittery frame clock must not wobble world animation (the `engine::TankMotion` rule). Shaders
  that do not animate may declare only a prefix of the `Camera` struct (`shadow.wgsl`,
  `ssao.wgsl`, `fx.wgsl` already do); trailing fields never shift earlier offsets.

## Growth Path

If shader count or binding complexity grows, add a generated binding layer with `wgsl_to_wgpu` or an internal generator. Until then, every new uniform/storage struct needs an explicit layout test that checks encoded size, alignment assumptions, and shader binding location.
