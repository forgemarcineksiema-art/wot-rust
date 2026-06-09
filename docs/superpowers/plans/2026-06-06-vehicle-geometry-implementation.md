# Vehicle Geometry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build deterministic procedural tank geometry that upgrades all vehicle silhouettes while preserving hitbox, terrain, physics, and renderer boundaries.

**Architecture:** Add a pure `vehicle_geometry` crate that bakes renderer-neutral local-space submeshes and mount frames. Prove the technology on `T55A`, add surface treatment, then move runtime rendering from per-frame dynamic mesh rebuilding to stable mesh handles and instanced render objects before rolling out all vehicles.

**Tech Stack:** Rust 2024, `glam`, `game_core`, `renderer_api` adapters, `renderer_wgpu` mesh handle/instancing path, existing `quality` gates and `./scripts/verify.ps1`.

---

### Task 1: Vehicle Geometry Kernel

**Files:**
- Create: `crates/vehicle_geometry/Cargo.toml`
- Create: `crates/vehicle_geometry/src/lib.rs`
- Create: `crates/vehicle_geometry/src/mesh.rs`
- Create: `crates/vehicle_geometry/src/bounds.rs`
- Create: `crates/vehicle_geometry/src/builder.rs`
- Create: `crates/vehicle_geometry/src/ops.rs`
- Create: `crates/vehicle_geometry/src/vehicle.rs`
- Create: `crates/vehicle_geometry/src/recipes.rs`
- Modify: `Cargo.toml`
- Modify: `docs/architecture.md`
- Modify: `crates/quality/tests/architecture_rules.rs`

- [x] Write failing tests for finite indexed geometry, bounds, normalized normals, `revolve`, `chamfered_prism`, and deterministic `T55A` bake metadata.
- [x] Run `cargo test -p vehicle_geometry` and confirm failures are from missing crate/API.
- [x] Implement the smallest pure geometry kernel that passes the tests.
- [x] Add `vehicle_geometry` to the workspace and required architecture artifacts.
- [x] Run `cargo test -p vehicle_geometry` and `cargo test -p quality --test architecture_rules`.

### Task 2: T-55A Dynamic Render Slice

**Files:**
- Modify: `crates/client/Cargo.toml`
- Modify/Create focused modules under `crates/client/src/vehicle_geometry_*`
- Modify: `crates/client/src/vehicle_mesh.rs`
- Modify: `crates/client/src/lib.rs`

- [x] Write failing client tests proving T-55A uses baked procedural submeshes, body geometry fits/fills `HitboxProfile`, and casemate/turret semantics stay unchanged for non-T55A fallback vehicles.
- [x] Implement adapter conversion from `vehicle_geometry` vertices to `renderer_api::SceneVertex`.
- [x] Route only `VehicleKind::T55A` through the baked path; leave other vehicles on the existing visual fallback.
- [x] Render `cargo run -p client --example vehicle_lineup -- target/vehicle_lineup.png` and inspect the output.

### Task 3: Surface Pass

**Files:**
- Modify/Create focused modules under `crates/vehicle_geometry/src`
- Modify T-55A recipe modules only.

- [x] Write failing tests for smoothing groups, hard-edge preservation, deterministic color bands, and darker lower/occluded geometry.
- [x] Add vertex-color surface roles and simple deterministic contact/dirt shading without changing `SceneVertex`.
- [x] Re-render the lineup and compare T-55A readability against the fallback vehicles.

### Task 4: Renderer Handoff

**Files:**
- Modify: `crates/renderer_api/src/lib.rs`
- Modify/Create focused modules under `crates/renderer_wgpu/src`
- Modify: `crates/client/src/app/render.rs`

- [x] Write failing tests for static mesh registration, per-object transforms, grouped tank instances, and no per-tank `write_buffer` loop.
- [x] Add backend-neutral mesh/material registration plus a renderer-owned batch plan compatible with existing `MeshHandle`, `MaterialHandle`, and `RenderObject`.
- [x] Move rich T-55A rendering off the dynamic mesh path while keeping shells transient.
- [x] Run renderer API/wgpu/client focused tests and a lineup screenshot.

### Task 5: All Vehicle Recipes

**Files:**
- Add focused recipe modules under `crates/vehicle_geometry/src/recipes`
- Modify client routing to use baked recipes for every `VehicleKind`.

- [x] Write failing tests for all known vehicles: finite bake, hitbox fit/fill, silhouette uniqueness, triangle budget, mount frames, and deterministic hashes.
- [x] Add family recipes for Soviet mediums, German vertical heavy, German sloped heavy, Panther II, and Jagdtiger casemate.
- [x] Render and inspect the full lineup.
- [x] Run `./scripts/verify.ps1`.
