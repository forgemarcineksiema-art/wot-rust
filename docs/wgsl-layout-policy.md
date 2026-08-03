# WGSL Layout Policy

WGSL data layout is a renderer backend contract, not a visual detail. Rust structs sent to uniform or storage buffers must be laid out with WGSL memory rules in mind, and every shader added to `renderer_wgpu` must be parsed and validated in tests.

## Rules

- Use `bytemuck` for simple POD data such as vertex buffer records.
- Use `encase` for uniform and storage buffer structs that must match WGSL alignment and padding rules.
- Use `naga` to parse and validate WGSL shaders in automated tests.
- Do not rely on `#[repr(C)]` alone for uniform or storage buffer compatibility.
- Do not hand-pack larger uniform/storage structs with ad hoc byte offsets.
- Keep shader binding groups aligned with `renderer_api::baseline_bind_group_layout()`.

## Current Baseline

- `TankVertex` is POD and can be copied to vertex buffers through `bytemuck`.
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
- `basic_tank.wgsl` and `scene.wgsl` are validated by `naga` in `renderer_wgpu` tests. Note that
  `basic_tank.wgsl` is a dead-path shader: only the unused `RenderBackend` (`renderer.rs`) and the
  validation tests reference it — the shipping picture is drawn by the scene/vehicle/sky/water/FX
  pipelines.
- The camera uniform lives in group 1, binding 0, matching the camera/view slot.
- `CameraUniform.time_params.x` is the presentation clock every shader animation reads
  (water ripple, foliage sway, weather). It is **tick-domain by doctrine**: fed from the fixed
  simulation tick plus the sub-tick render phase, never integrated from render-frame deltas — a
  jittery frame clock must not wobble world animation (the `engine::TankMotion` rule). Shaders
  that do not animate may declare only a prefix of the `Camera` struct (`shadow.wgsl`,
  `ssao.wgsl`, `fx.wgsl` already do); trailing fields never shift earlier offsets.

## Growth Path

If shader count or binding complexity grows, add a generated binding layer with `wgsl_to_wgpu` or an internal generator. Until then, every new uniform/storage struct needs an explicit layout test that checks encoded size, alignment assumptions, and shader binding location.
