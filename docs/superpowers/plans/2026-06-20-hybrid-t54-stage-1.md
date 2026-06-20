# Hybrid T-54 Stage 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Correct the hybrid T-54's moving gun assembly and make revolved caps free of degenerate triangles.

**Architecture:** Keep all vehicle submeshes in shared vehicle-local coordinates. Make `revolve` encode poles explicitly, then use a mounted barrel and a separate gun-submesh mantlet in `vehicle_build`. The existing Forge and client bake paths remain unchanged.

**Tech Stack:** Rust, glam, Cargo tests, `vehicle_geometry::GeometryMesh`.

---

## File structure

- `crates/revolve/src/revolve.rs`: generic origin-aware revolution and non-degenerate cap topology.
- `crates/revolve/src/parts.rs`: mounted barrel API and regression tests.
- `crates/sdf_mesh/src/t54.rs`: fixed T-54 turret socket, without the moving mantlet.
- `crates/vehicle_build/src/t54.rs`: compose socket, oval moving mantlet, and barrel in their correct submeshes; test mount-frame contracts.
- `crates/vehicle_build/src/part.rs`: unchanged routing boundary; use its `Mesh` arm for the moving assembly.

### Task 1: Make revolved poles topologically valid

**Files:**
- Modify: `crates/revolve/src/revolve.rs`
- Test: `crates/revolve/src/revolve.rs`

- [ ] **Step 1: Write a failing mesh-validity test**

Add a test which revolves `[(0.0, 0.0), (0.0, 0.5), (1.0, 0.5), (1.0, 0.0)]`, then asserts every indexed triangle has three distinct indices and nonzero cross-product area.

- [ ] **Step 2: Run the focused test and verify RED**

Run: `cargo test -p revolve capped_revolve_has_no_degenerate_triangles -- --exact`

Expected: failure because the current zero-radius rings weld into repeated indices.

- [ ] **Step 3: Implement row-aware revolve topology**

Replace the fixed `profile.len() * segments` allocation with rows that contain either one pole index or `segments` ring indices. Emit a fan for pole-to-ring transitions and two triangles per wedge for ring-to-ring transitions. Translate every generated vertex by a new `origin: Vec3` parameter; keep the existing public `revolve` as an origin-zero wrapper if other callers need it.

- [ ] **Step 4: Run the focused test and verify GREEN**

Run: `cargo test -p revolve capped_revolve_has_no_degenerate_triangles -- --exact`

Expected: one passing test.

### Task 2: Author the T-54 barrel on its authoritative mount frame

**Files:**
- Modify: `crates/revolve/src/parts.rs`
- Test: `crates/revolve/src/parts.rs`

- [ ] **Step 1: Write failing mounted-barrel tests**

Replace the origin-based barrel assertions with a test calling a mounted-barrel API using `MountFrames::for_vehicle(VehicleKind::T54_1951)`. Assert the barrel's Y bounds straddle `gun_trunnion.translation.y`, its muzzle-side Z reaches `muzzle.translation.z`, and its minimum Z remains behind that muzzle.

- [ ] **Step 2: Run the focused test and verify RED**

Run: `cargo test -p revolve mounted_t54_barrel_uses_authoritative_frames -- --exact`

Expected: compilation failure until the mounted API exists, or an assertion failure showing the old Y=0 barrel.

- [ ] **Step 3: Implement the mounted barrel API**

Add `gun_barrel_between(trunnion: Vec3, muzzle: Vec3) -> GeometryMesh`. Require an approximately +Z bore axis for this current vehicle recipe, revolve at `trunnion`, start the profile just inside the mantlet, and end it exactly at `muzzle.z`. Preserve the current 20 segments, steel material, and barrel smoothing group.

- [ ] **Step 4: Run barrel tests and verify GREEN**

Run: `cargo test -p revolve parts::tests -- --exact`

Expected: all barrel and running-gear tests pass.

### Task 3: Separate fixed socket from moving mantlet

**Files:**
- Modify: `crates/sdf_mesh/src/t54.rs`
- Modify: `crates/vehicle_build/src/t54.rs`
- Test: `crates/vehicle_build/src/t54.rs`

- [ ] **Step 1: Write failing submesh contract tests**

Add a T-54 test that inspects the gun mesh and asserts: its bounds straddle the trunnion Y; its Z maximum reaches the muzzle; it contains cast-armour vertices forming a mantlet whose X span exceeds its Y span. Assert the turret mesh no longer contains the old forward mantlet sphere's terminal Z extent beyond the fixed socket design.

- [ ] **Step 2: Run the focused test and verify RED**

Run: `cargo test -p vehicle_build t54_moving_mantlet_follows_the_gun_mount -- --exact`

Expected: failure because the gun submesh contains only an origin-space barrel and no cast mantlet.

- [ ] **Step 3: Implement fixed socket plus moving assembly**

In `sdf_mesh::t54_turret`, replace the smooth-unioned forward mantlet sphere with a shallow `smooth_subtract` socket centred on the trunnion. In `vehicle_build::t54`, build an oval cast mantlet mesh around the trunnion and merge it with `revolve::gun_barrel_between`. Place both in `SubmeshKind::Gun`; leave the socket in `SubmeshKind::Turret`.

- [ ] **Step 4: Run the focused test and verify GREEN**

Run: `cargo test -p vehicle_build t54_moving_mantlet_follows_the_gun_mount -- --exact`

Expected: one passing test.

### Task 4: Preserve module and LOD regression coverage

**Files:**
- Modify: `crates/vehicle_build/src/t54.rs`
- Test: `crates/vehicle_build/src/t54.rs`

- [ ] **Step 1: Write a failing module-length test in the new frame model**

Set a non-stock valid gun module and assert its barrel is still centred at the same trunnion Y while its muzzle-side Z changes only according to the explicitly selected mount rule. Remove any assertion that treats `1.0 + SPIKE_GUN_SCALE * barrel_length` as an authoritative muzzle coordinate.

- [ ] **Step 2: Run the focused test and verify RED**

Run: `cargo test -p vehicle_build swapping_the_gun_module_changes_the_mounted_barrel -- --exact`

Expected: failure until module-to-muzzle resolution is explicit.

- [ ] **Step 3: Implement a single module-to-mounted-barrel policy**

Use the stock module to terminate at the authoritative `MountFrames::muzzle`; derive an alternate module's muzzle offset from the stock module's barrel-length delta. Keep the bore on the trunnion axis and eliminate `SPIKE_GUN_SCALE`.

- [ ] **Step 4: Run all focused crates and verify GREEN**

Run: `cargo test -p revolve -p sdf_mesh -p vehicle_build`

Expected: all focused tests pass.

### Task 5: Render and repository verification

**Files:**
- Generated: `target/spike_sdf/t54_contact_sheet.png`

- [ ] **Step 1: Regenerate the renderer-free review images**

Run: `cargo run -p vehicle_build --example assemble`

Expected: the profile image shows the gun at turret height, with a visible moving mantlet.

- [ ] **Step 2: Inspect the contact sheet**

Inspect `target/spike_sdf/t54_contact_sheet.png`; reject the change if the barrel intersects the ground, misses the turret socket, or the mantlet remains a fixed turret protrusion.

- [ ] **Step 3: Run the canonical verification gate**

Run: `./scripts/verify.ps1`

Expected: exit code 0.

- [ ] **Step 4: Commit only Stage 1 source, tests, and documentation**

Run: `git add crates/revolve crates/sdf_mesh crates/vehicle_build docs/superpowers/specs docs/superpowers/plans && git commit -m "feat: mount hybrid t54 gun assembly"`

Expected: one focused commit; do not stage unrelated workspace files.
