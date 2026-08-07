# Working in this repo (read me first — any AI tool)

Rust tank game ("honest tank": no ±25% RNG, 7v7, three eras). Workspace of crates under
`crates/{foundation,kernels,vehicle,world,runtime,render,ui,apps,tooling}`.

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
- **No clones** in content; **CC0 only** for any future imported assets (license manifest
  required). Flora is procedural-only — no imported trees.
- 1 branch = 1 PR from master; commits end with the Co-Authored-By line of the tool used.

## Where things are decided
- `docs/battle-first/program.md` — **the live plan (waves + STATUS), start here**; defects:
  `docs/battle-first/audit-register.md`.
- `docs/art-direction-program.md` — the visual DEFECT register; `docs/art-direction-policy.md` —
  the target look, its 7 rules and their locks.
- `docs/vehicles/t-54.md` — the benchmark vehicle dossier (the fleet's bar; siblings alongside).
- `docs/map-forge-policy.md` — map + flora doctrine (procedural-only); `docs/maps/*.md` —
  per-map dossiers; `docs/world-2.0-program.md` — the live world program.
  editor: `cargo run -p editor`.
- `docs/honest-steel-policy.md`, `docs/shadow-policy.md`.
- `crates/tooling/quality` — **the ratchet**: 18 gate tests enforce the layer DAG, append-only
  identity enums and the W0 rules. Burn allowlist entries down; never widen one to get green.
- Review renders: `cargo run -p client --example probe -- <tenement_probe|factory_probe|flora_probe|ostrogorsk_views>`.
- Perf: `cargo run -p client --release --example probe -- perf_capture`; sim bench `combat_hot_path`.

## Environment pitfalls (Windows / PowerShell 5.1)
- `Get-Content -Raw` without encoding mangles UTF-8 (Polish comments!) — edit files with
  proper tooling or `[IO.File]::ReadAllText/WriteAllText` with UTF-8.
- Here-strings with embedded `"` break native arg parsing — use `git commit -F <file>`.
- Killed cargo builds can corrupt incremental state (LNK2019 anon symbols) —
  `cargo clean -p <crate>` fixes it.
- clippy requires `#[cfg(test)]` modules LAST in a file (`items_after_test_module`).
