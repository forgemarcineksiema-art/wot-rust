# WOT Rust Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a compileable Rust workspace foundation for a native 3D tank game with client, server, tools, rendering, physics, simulation, networking, terrain, and editor crates.

**Architecture:** Keep gameplay data and deterministic simulation separate from rendering and platform code. Use `bevy_ecs` only behind the local `engine` layer, Rapier for broad collision primitives, and custom code for tank movement and armor/shell logic.

**Tech Stack:** Rust, Cargo workspace, `wgpu`, `winit`, `bevy_ecs`, `rapier3d`, WGSL, `egui`, `serde`, `bincode`, `clap`, glTF.

---

### Task 1: Workspace And Toolchain

**Files:**
- Create: `rust-toolchain.toml`
- Create: `Cargo.toml`
- Create: `.gitignore`

- [x] Pin the local workspace to `nightly-x86_64-pc-windows-msvc` because the machine's stable cargo component is broken.
- [x] Create workspace members for all requested crates.
- [x] Centralize dependency versions in `[workspace.dependencies]`.

### Task 2: Contract Tests

**Files:**
- Create: `crates/game_core/tests/armor_model.rs`
- Create: `crates/sim/tests/fixed_tick.rs`
- Create: `crates/net/tests/protocol_roundtrip.rs`
- Create: `crates/physics/tests/tank_controller.rs`
- Create: `crates/terrain/tests/heightmap.rs`

- [x] Write tests for armor effective thickness and penetration resolution.
- [x] Write tests for fixed tick movement and turret rotation.
- [x] Write tests for protocol encode/decode roundtrip.
- [x] Write tests for custom tank controller and Rapier collider creation.
- [x] Write tests for terrain bilinear sampling and signed chunk coordinates.
- [x] Run `cargo test --workspace --all-targets` and observe a red state before implementation.

### Task 3: Minimal Implementation

**Files:**
- Modify: `crates/*/src/*.rs`
- Create: `crates/renderer_wgpu/src/shaders/basic_tank.wgsl`
- Create: `README.md`
- Create: `docs/architecture.md`

- [x] Implement pure gameplay types in `game_core`.
- [x] Implement deterministic simulation stepping in `sim`.
- [x] Implement bincode protocol messages in `net`.
- [x] Implement ECS world wrapper in `engine`.
- [x] Implement abstract `renderer_api` crate and `wgpu` backend shell.
- [x] Implement Rapier collider helpers and custom tank controller.
- [x] Implement terrain heightmap and chunk primitives.
- [x] Wire client, server, tools, and editor binaries.

### Task 4: Verification

**Files:**
- All workspace crates.

- [x] Run `cargo fmt --all`.
- [x] Run `cargo test --workspace --all-targets`.
- [x] Run `cargo check --workspace --all-targets`.
- [x] Run a server smoke command with `cargo run -p server -- --max-ticks 3`.
- [x] Run a tooling smoke command with `cargo run -p tools -- make-flat-heightmap --output assets/generated/flat.heightmap.json --width 4 --height 4 --cell-size 2 --height-m 1.5`.
- [x] Run an editor smoke command with `cargo run -p editor`.
