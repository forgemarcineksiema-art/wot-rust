# WOT Rust Prototype

Native Rust workspace for armored vehicle battles on large terrain maps.

This is not a general-purpose engine. The foundation is intentionally biased
toward outdoor terrain, vehicles, effects, spotting, shell physics, LOD,
shadows, networking, and a headless authoritative server.

## Stack

- Rust workspace with separate gameplay, simulation, renderer API, renderer backend, physics, networking, terrain, client, server, tools, and editor crates.
- `winit` for the client window/input loop.
- `wgpu` backend crate with WGSL shader entrypoint.
- `bevy_ecs` inside the local engine layer.
- Rapier integration types plus a custom tank controller.
- glTF import tooling that emits a first custom `.wotasset` JSON manifest.
- Headless authoritative server binary from day one.

## Commands

```powershell
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo bench --workspace --no-run
./scripts/verify.ps1
cargo run -p server -- --max-ticks 10
cargo run -p client
cargo run -p tools -- make-flat-heightmap --output assets/generated/flat.heightmap.json
cargo run -p tools -- generate-map --map prokhorovka-hill-252-2 --output assets/maps/prokhorovka_hill_252_2.terrain.json
cargo run -p tools -- generate-vehicle --vehicle t54-1951 --output assets/vehicles/t54_1951.vehicle.json
cargo run -p tools -- generate-vehicle --vehicle t55a --output assets/vehicles/t55a.vehicle.json
cargo run -p tools -- generate-vehicle --vehicle tiger-i-ausf-e --output assets/vehicles/tiger_i_ausf_e.vehicle.json
cargo run -p tools -- generate-vehicle --vehicle tiger-ii-ausf-b --output assets/vehicles/tiger_ii_ausf_b.vehicle.json
cargo run -p tools -- generate-vehicle --vehicle jagdtiger --output assets/vehicles/jagdtiger.vehicle.json
cargo run -p tools -- generate-vehicle --vehicle panther-ii --output assets/vehicles/panther_ii.vehicle.json
cargo run -p tools -- convert-gltf --input path/to/your-model.gltf --output assets/generated/tank.wotasset.json
cargo run -p editor
cargo run -p client --example screenshot -- target/scene.png
```

The repo pins a dated nightly (`nightly-2026-02-12`) in `rust-toolchain.toml` for reproducible builds and stable f32/replay-regression fixtures. Bump the date deliberately and re-run `./scripts/verify.ps1` so any fixture drift is a reviewed change, not a surprise from a toolchain auto-update.

## Controls

`cargo run -p client` opens the battle window: **WASD** drives the hull, the **mouse** aims the
turret, **Space** fires, the **scroll wheel** zooms, and **1**/**2** switch between the
third-person and sniper cameras. For a headless preview, `--example screenshot` renders the
battle offscreen to a PNG.

## Rules

Hard project rules are documented in `docs/engineering-rules.md`. Test, protocol snapshot, replay regression, and benchmark workflows are documented in `docs/testing-and-regression.md`.
