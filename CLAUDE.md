# Working in this repo (read me first — any AI tool)

Rust tank game ("honest tank": no ±25% RNG, 7v7, three eras). Workspace of crates under
`crates/{foundation,runtime,world,render,vehicle,apps,kernels,tooling}`.

## Non-negotiable rules
- **Every change lands with a locking test.** Gameplay promises live in tests, not comments.
- **Merge gate = local `scripts/verify.ps1`** (fmt + clippy `-D warnings` + full workspace
  tests). CI billing is blocked; there is no other gate. Long runs: stage fmt/clippy/test
  separately (a cold full run exceeds 10 min).
- **One look policy**: min spec MX330 @ 60 FPS, no quality options. Frame drops are a game
  bug. Budgets are raised per-item with a measurement, never fleet-wide.
- **Honesty doctrine**: what blocks the shell blocks the eye; collision boxes ARE the visual
  footprint; a leaf's shadow is its alpha mask; scenery NEVER blocks gameplay.
- **Append-only enums** everywhere (wire/asset identity): `MapId`, `StaticCoverKind`,
  `SceneryKind`, `RoadSurface`, vertex layouts. Never reorder.
- **Maps are data**: RON blueprints in `crates/world/map_forge/blueprints/`, compiled +
  report-gated + golden-hashed (`blueprints/goldens.ron` — bless deliberately).
- **No clones** in content; **CC0 only** for imported assets (license manifest required).
- 1 branch = 1 PR from master; commits end with the Co-Authored-By line of the tool used.

## Where things are decided
- `docs/art-direction-program.md` — **current program STATUS + defect register, start here.**
- `docs/art-direction-policy.md` — the target look, its 7 rules and their locks.
- `docs/urban-map-program.md` — previous program (complete); still doctrine for maps + flora.
- `docs/map-forge-policy.md`, `docs/maps/*.md` — map authoring; editor: `cargo run -p editor`.
- `docs/destruction-program.md`, `docs/shadow-policy.md`, `docs/vehicle-fidelity-masterplan.md`.
- Review renders: `cargo run -p client --example <tenement|factory|flora|ostrogorsk_views>_probe`.
- Perf: `cargo run -p client --release --example perf_capture`; sim bench `combat_hot_path`.

## Environment pitfalls (Windows / PowerShell 5.1)
- `Get-Content -Raw` without encoding mangles UTF-8 (Polish comments!) — edit files with
  proper tooling or `[IO.File]::ReadAllText/WriteAllText` with UTF-8.
- Here-strings with embedded `"` break native arg parsing — use `git commit -F <file>`.
- Killed cargo builds can corrupt incremental state (LNK2019 anon symbols) —
  `cargo clean -p <crate>` fixes it.
- clippy requires `#[cfg(test)]` modules LAST in a file (`items_after_test_module`).
