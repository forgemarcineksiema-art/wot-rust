# WOT

A tank combat game written in Rust: WWII armored vehicles, nation lines and tiers, 7v7 and
15v15 battles on 1 km maps, with a native wgpu renderer and a headless authoritative server.

Damage is deterministic — no ±25 % roll — and penetration is resolved against the vehicle's
actual armor plates.

## Features

- **Vehicles** — T-54, T-34-85, IS-3, Centurion Mk 3, Tiger I, Tiger II, Panther II and
  Jagdtiger. Each vehicle is a single RON blueprint that produces its hitbox, armor volumes,
  gun mounts and render mesh.
- **Maps** — Prokhorovka, Bystra Valley, Orliny Pereval, Ostrogorsk and Mazurski Przesmyk.
  Maps are RON blueprints compiled by Map Forge and checked against golden hashes.
- **Game modes** — online PvP (7v7 or 15v15, bots fill empty seats) and an offline 15v15
  battle against AI.
- **Ballistics and armor** — shell flight, ricochet and normalization against 3D plates, module
  and crew damage, fire, staged track damage.
- **Destruction** — buildings collapse into rubble, walls can be breached, fences and light
  cover are crushed; all of it replicated over the network.
- **Renderer** — wgpu with cascaded shadows, SSAO, HDR and bloom, grass, procedural buildings
  and trees authored in Blender (CC0 bark textures, embedded and hash-locked).
- **Networking** — fixed-tick server simulation, snapshot replication, deterministic replays.
- **Map editor** — sculpting, stamps, roads, object and gameplay tools, line-of-sight
  visualization, playtest straight from the editor.

## Requirements

- Windows 10/11
- A GPU with DirectX 12 or Vulkan support. Performance target: 60 FPS on a GeForce MX330.
- The Rust toolchain from `rust-toolchain.toml` (a pinned nightly; `rustup` installs it
  automatically)

The first build compiles the whole workspace from scratch and takes a while.

## Getting started

```powershell
git clone <repo-url> wot
cd wot
cargo run --release -p client
```

Always run the client in release mode — a full battle is not playable in a debug build.

### Options

The client is configured through environment variables:

| Variable | Purpose | Example |
|---|---|---|
| `WOT_MAP` | Map to load | `ostrogorsk`, `bystra-valley`, `orliny-pereval`, `mazurski-przesmyk`, `prokhorovka-hill-252-2` |
| `WOT_VEHICLE` | Vehicle to request from the server | `t54_1951`, `tiger_i_ausf_e`, `is3`, `centurion_mk3` |
| `WOT_CONNECT` | Join a dedicated server instead of hosting locally | `127.0.0.1:40000` |

```powershell
$env:WOT_MAP = "ostrogorsk"
cargo run --release -p client
```

Garage state, key bindings and battle history are saved in `%APPDATA%\wot-prototype\`.

### Dedicated server

```powershell
cargo run --release -p server -- --bind 0.0.0.0:40000 --map bystra-valley
```

| Flag | Default | Description |
|---|---|---|
| `--bind` | `0.0.0.0:40000` | Listen address |
| `--map` | catalog default | Map slug to host |
| `--lobby-wait-s` | `30` | Seconds before the battle starts with bots in empty seats |
| `--max-battles` | `0` (unlimited) | Battles to host before exiting |
| `--seed` | `0` (from clock) | Deterministic battle seed |
| `--config` | — | Load all settings from a RON file instead of flags |

### Map editor

```powershell
cargo run -p editor -- crates/world/map_forge/blueprints/bystra-valley.map.ron
```

See [`crates/apps/editor/README.md`](crates/apps/editor/README.md) for the editor's controls.

## Controls

| Action | Key |
|---|---|
| Drive | `W` `A` `S` `D` / arrow keys |
| Brake | `Ctrl` |
| Cruise speed up / down | `R` / `F` |
| Aim | Mouse |
| Fire | `Space` / Left mouse button |
| Sniper scope | `Shift` |
| Free look | `Alt` |
| Ammunition | `1` `2` `3` |
| Camera mode | `V` |
| Mark target | `T` |
| Command wheel | `Z` |
| Minimap size | `M` |
| Return to garage | `G` |
| Menu | `Esc` |
| Fullscreen | `F11` |

Bindings are stored in `%APPDATA%\wot-prototype\keybinds.json`.

## Project layout

```
crates/
  foundation/   game_core (shared types, vehicle and map identity), terrain
  kernels/      geometry kernels: SDF, sweep, loft, revolve, panels, deformation
  vehicle/      vehicle_forge, vehicle_build, vehicle_recipes — blueprint to mesh and armor
  world/        map_forge (map compiler), world_forge, scene_build
  runtime/      sim, physics, net, battle_host, matchmaker, audio, engine
  render/       renderer_api, renderer_wgpu
  ui/           ui_kit
  apps/         client, server, editor, tools
  tooling/      quality — architecture and consistency checks
assets/         embedded assets (flora, textures) with license files
docs/           design and engineering documentation
scripts/        build and verification scripts
```

## Development

Checks are run locally:

```powershell
./scripts/preflight.ps1                  # fast: formatting + architecture checks
./scripts/verify-pr.ps1 -Crates client   # fmt, clippy -D warnings, tests of the given crates
./scripts/verify.ps1                     # full suite: every crate, example, benchmark and test
```

Useful tools:

```powershell
cargo run --release -p client --example probe -- perf_capture   # frame-time measurement
cargo bench -p sim --bench combat_hot_path                        # simulation benchmark
cargo run --release -p tools -- fit --vehicle <slug>              # fit a vehicle blueprint to reference outlines
```

Every change ships with a test that locks its behavior. Engineering rules are in
[`docs/engineering-rules.md`](docs/engineering-rules.md) and
[`docs/testing-and-regression.md`](docs/testing-and-regression.md).

## Documentation

| Document | Contents |
|---|---|
| [`docs/game-design.md`](docs/game-design.md) | Game design |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Current state and system inventory |
| [`docs/program.md`](docs/program.md) | Work queue |
| [`docs/architecture.md`](docs/architecture.md) | Architecture overview |
| [`docs/game-modes.md`](docs/game-modes.md) | Game modes and matchmaking |
| [`docs/maps/`](docs/maps/) | Per-map documentation |
| [`docs/vehicles/`](docs/vehicles/) | Per-vehicle reference dossiers |

## License

Proprietary. All rights reserved. Third-party assets (textures, fonts) carry their own
license files under `assets/`.
