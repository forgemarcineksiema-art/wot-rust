# WOT Rust — the honest tank game

A native-Rust game of armored vehicle battles on large terrain maps: 7v7 on
1000 m fields, WWII vehicles on nation trees (lines and tiers), and a design creed of **honesty** — no ±25%
damage RNG, dispersion as a hard maximum radius with no shot ever outside the circle, no
premium ammo, armor resolved against real 3D plates, and a world where what
blocks the shell is exactly what blocks the eye. Not a general-purpose engine: everything is
biased toward outdoor terrain, vehicles, spotting, shell physics, destruction and a headless
authoritative server.

**Contributing (human or AI): read `CLAUDE.md` first** — the working contract (locking
tests, the local verify gate, one-look budgets, append-only enums) — then
**`docs/ROADMAP.md`** for the whole picture: systems inventory, the honest gap list toward
release, and where the center of gravity should go next. Individual program docs
(art direction, multiplayer production, contact & tracks…) are execution details under `docs/`.

## What exists today

- **Battle maps** (the count lives in `docs/ROADMAP.md`, pinned to `MapId::SHIPPED` by the
  `quality` gate), authored as RON blueprints and compiled by Map Forge
  (`crates/world/map_forge`): Prokhorovka (steppe, 1943), Bystra Valley (river + town),
  Orliny Pereval (mountain pass, 1942), **Ostrogorsk** (railway city, 1943 — dense
  masonry core, breachable walls, born ruins, a rail berm with three gates) and Mazurski
  Przesmyk (lake defile). A full in-game
  map editor (`cargo run -p editor`): sculpt brushes, stamps, terrain strokes, grab handles,
  object/road/gameplay tools, viewshed instrument, Ctrl+P playtest.
- **A blueprint-born fleet** (T-54, IS-3, Centurion Mk 3, Tiger I/II, Jagdtiger, Panther II,
  T-34-85): one RON source feeds hitbox, armor volumes, mounts and the visual mesh.
- **Honest Steel destruction**: buildings pound to rubble, walls breach, fences crush,
  tracks take staged damage — gameplay on volumes, presentation contact-true, all replicated.
- **A real renderer** (wgpu): cascaded sun shadows, SSAO, HDR + bloom, weather looks with a
  fairness lock, chunked/culled statics, grass, battle FX, **procedural building generators**
  and **trees as data** (route 2, 2026-09-02: grown offline in Blender, leaf clusters rendered
  there, CC0 bark tiles with their licence beside them under `assets/flora/`, everything
  embedded and hash-locked; no imported tree models). A battlefield oak's trunk is a gameplay
  solid, not a painting.
- **Deterministic sim + netcode**: fixed-tick authoritative server, protocol snapshots,
  replay regression fixtures, per-vehicle spotting, bots with route planning.

## Daily commands

```powershell
./scripts/verify-pr.ps1 -Crates client   # the PR gate: fmt + clippy -D warnings (all targets) + the touched crates' tests
./scripts/verify.ps1                     # THE full gate, once a day over what landed: every example, bench and test
cargo run --release -p client         # play (release; a 14-tank battle needs the optimized build)
$env:WOT_MAP = "ostrogorsk"           # pick a map (prokhorovka-hill-252-2 | bystra-valley | orliny-pereval | ostrogorsk | mazurski-przesmyk)
cargo run -p editor                   # the map editor (or pass a blueprint path)
cargo run -p server -- --max-ticks 10 # headless authoritative server
```

Review/QA artifacts:

```powershell
cargo run -p client --release --example probe -- perf_capture      # the one-look FPS numbers
cargo run -p client --example probe -- flora_probe                 # the species lineup down the LOD ladder
cargo run -p client --example probe -- ostrogorsk_views            # city review renders
```

The repo pins a dated nightly (`rust-toolchain.toml`) for reproducible builds and stable
replay fixtures — bump deliberately and re-run `./scripts/verify.ps1`.

## Controls

**WASD** drives the hull, **mouse** aims, **Space** fires, **1/2/3** select ammo, scroll
zooms, **Shift** holds the sniper scope. `--example probe -- screenshot` renders offscreen to PNG.

## Rules and doctrine

- `CLAUDE.md` — the working contract for any contributor or AI tool.
- `docs/engineering-rules.md`, `docs/testing-and-regression.md` — hard project rules.
- `docs/ROADMAP.md` — the live program status; `docs/maps/*.md` — per-map dossiers.
- `docs/map-forge-policy.md`, `docs/shadow-policy.md` — the standing doctrines.
