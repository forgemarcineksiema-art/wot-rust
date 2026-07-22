# WOT Rust — the honest tank game

A native-Rust, from-scratch armored battle game: 7v7 on 1000 m maps, WWII-era vehicles in
three eras, and a design creed of **honesty** — no ±25% damage RNG, sub-milliradian
dispersion, no premium ammo, armor resolved against real 3D plates, and a world where what
blocks the shell is exactly what blocks the eye. Not a general-purpose engine: everything is
biased toward outdoor terrain, vehicles, spotting, shell physics, destruction and a headless
authoritative server.

**Contributing (human or AI): read `CLAUDE.md` first** — the working contract (locking
tests, the local verify gate, one-look budgets, append-only enums) — then
**`docs/ROADMAP.md`** for the whole picture: systems inventory, the honest gap list toward
release, and where the center of gravity should go next. Individual program docs (urban
map, destruction, fleet…) are execution details under `docs/`.

## What exists today

- **Four battle maps**, authored as RON blueprints and compiled by Map Forge
  (`crates/world/map_forge`): Prokhorovka (steppe, 1943), Bystra Valley (river + town),
  Orliny Pereval (mountain pass, 1942) and **Ostrogorsk** (railway city, 1943 — dense
  masonry core, breachable walls, born ruins, a rail berm with three gates). A full in-game
  map editor (`cargo run -p editor`): sculpt brushes, stamps, terrain strokes, grab handles,
  object/road/gameplay tools, viewshed instrument, Ctrl+P playtest.
- **A blueprint-born fleet** (T-54, IS-3, Centurion Mk 3, Tiger I/II, Jagdtiger, Panther II,
  T-34-85): one RON source feeds hitbox, armor volumes, mounts and the visual mesh.
- **Honest Steel destruction**: buildings pound to rubble, walls breach, fences crush,
  tracks take staged damage — gameplay on volumes, presentation contact-true, all replicated.
- **A real renderer** (wgpu): cascaded sun shadows, SSAO, HDR + bloom, weather looks with a
  fairness lock, chunked/culled statics, grass, battle FX, procedural building/tree
  generators — plus a **CC0 flora import pipeline** (glTF → validated assets → one runtime
  atlas with alpha-cutout foliage whose shadows are their masks).
- **Deterministic sim + netcode**: fixed-tick authoritative server, protocol snapshots,
  replay regression fixtures, per-era spotting, bots with route planning.

## Daily commands

```powershell
./scripts/verify.ps1                  # THE merge gate: fmt + clippy -D warnings + all tests
cargo run --release -p client         # play (release; a 14-tank battle needs the optimized build)
$env:WOT_MAP = "ostrogorsk"           # pick a map (prokhorovka-hill-252-2 | bystra-valley | orliny-pereval | ostrogorsk)
cargo run -p editor                   # the map editor (or pass a blueprint path)
cargo run -p server -- --max-ticks 10 # headless authoritative server
```

Review/QA artifacts:

```powershell
cargo run -p client --release --example perf_capture      # the one-look FPS numbers
cargo run -p client --example flora_probe                 # imported vs procedural trees
cargo run -p client --example ostrogorsk_views            # city review renders
cargo run -p tools -- import-flora --input model.glb --manifest model.manifest.ron
cargo run -p tools -- generate-map --map ostrogorsk --output assets/maps/ostrogorsk.terrain.json
```

The repo pins a dated nightly (`rust-toolchain.toml`) for reproducible builds and stable
replay fixtures — bump deliberately and re-run `./scripts/verify.ps1`.

## Controls

**WASD** drives the hull, **mouse** aims, **Space** fires, **1/2/3** select ammo, scroll
zooms, **Shift** holds the sniper scope. `--example screenshot` renders offscreen to PNG.

## Rules and doctrine

- `CLAUDE.md` — the working contract for any contributor or AI tool.
- `docs/engineering-rules.md`, `docs/testing-and-regression.md` — hard project rules.
- `docs/urban-map-program.md` — current program status; `docs/maps/*.md` — per-map dossiers.
- `docs/map-forge-policy.md`, `docs/destruction-program.md`, `docs/shadow-policy.md`,
  `docs/vehicle-fidelity-masterplan.md` — the standing doctrines.
