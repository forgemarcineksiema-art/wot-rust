# Working in this repo (read me first — any AI tool)

Rust tank game ("honest tank": no ±25% RNG, 7v7, nations / lines / tiers). Workspace of crates under
`crates/{foundation,kernels,vehicle,world,runtime,render,ui,apps,tooling}`.

## Non-negotiable rules
- **Every change lands with a locking test.** Gameplay promises live in tests, not comments.
- **Two gates, local, no CI** (billing blocked; `.github/workflows/ci.yml` stays dormant on
  purpose — `architecture_rules.rs` requires the file). `scripts/preflight.ps1` first (30 s:
  fmt + the `quality` ratchet — four gate reruns in one day died on it). PR gate =
  `scripts/verify-pr.ps1 -Crates <touched>` (fmt + clippy `-D warnings` over all targets +
  `quality` + `tools` + the touched crates' tests). Full gate = `scripts/verify.ps1` (+ every example/bench/test): once a day
  over what landed, and before any merge touching examples, benches, wire, replays or physics
  numbers; `-Deep` after a killed build. A cold full run exceeds 25 min — run gates detached
  (log to a file, wait for the exit line) and never kill a build mid-flight.
- **One worktree per session, for the whole session** (`git worktree add ../wot-work -b <branch>
  master`, then branch inside it per PR). Cargo keys workspace artifacts by path: a fresh
  worktree per PR recompiles all 33 crates every time.
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
- `docs/ROADMAP.md` — **the whole picture: systems inventory and the honest gap list, start here**.
- `docs/art-direction-program.md` — the visual DEFECT register; `docs/art-direction-policy.md` —
  the target look, its 7 rules and their locks.
- `docs/vehicles/t-54.md` — the benchmark vehicle dossier (the fleet's bar; siblings alongside).
- `docs/map-forge-policy.md` — map + flora doctrine (procedural-only); `docs/maps/*.md` —
  per-map dossiers. editor: `cargo run -p editor`.
- `docs/shadow-policy.md`, `docs/engineering-rules.md`.
- `crates/tooling/quality` — **the ratchet**: 21 gate tests enforce the layer DAG, append-only
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
