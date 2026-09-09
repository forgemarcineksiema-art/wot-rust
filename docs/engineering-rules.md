# Engineering Rules

These are hard rules for this prototype. If a rule hurts, change the design before weakening the rule.

## Crates And Files

- Keep crates small and single-purpose. A crate owns one architectural reason to change.
- Keep files focused on one concern and split when a module accumulates unrelated behavior.
  There is no hard line limit — reviewability is judged by cohesion, not a line counter.
- Do not create "god object" modules that own simulation, rendering, networking, and tooling together.
- Prefer adding a new focused module or crate over expanding a central file indefinitely.
- A binary crate should compose systems; it should not become the owner of simulation, protocol, render, or physics rules.
- Shared behavior belongs in a library crate with tests before a client/server/editor binary depends on it.
- Gameplay state must advance from fixed simulation/server ticks, never from render-frame delta time.
- The desktop client must not own authoritative `SimulationState`; local play still goes through the server API.
- Do not build a general-purpose engine. Bias every abstraction toward armored vehicle battles on large terrain maps.
- Treat terrain as a gameplay system, not as a single imported scene mesh.
- Camera projection and shaders use WebGPU/wgpu depth range `[0, 1]`.
- Debug tools and GPU labels are first-week systems, not late polish.

## Required Gates

### Sustained performance and one look

[One-look policy](one-look-policy.md) (owner, 2026-09-09; GDD row 36) requires the same
intended picture and combat readability across supported PCs, with sustained 60 FPS on
the warmed MX330 floor and higher FPS on stronger hardware. These are open targets.
[The performance plan](sustained-performance-plan.md) defines acceptance, including actual
viewport, full battles, late-state aiming, frame-time tails, thermal conditions and input
response. Preserve the fixed simulation step; increasing presentation rate must not change
gameplay results. Budgets move per measured item, never by disabling gameplay-relevant world
content. Cold A/B attribution and warmed acceptance are complementary requirements.

Performance claims use [the capture template](performance-capture-template.md); instrumentation
and deterministic tests do not certify frame rate. Missing hardware evidence stays explicit.

### Code gates

There are two gates and one truth: the full gate is what the day owes, the PR gate is what a
PR owes. Both are scripts, the second runs a subset of the first, and neither is a CI job
(CI billing is blocked; `.github/workflows/ci.yml` stays dormant on purpose).

The **full gate**, `scripts/verify.ps1`:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`

The **PR gate**, `scripts/verify-pr.ps1 [-Crates a,b,...]`:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings` — the same whole-workspace,
  all-targets front-end pass, so a probe or a bench that stops compiling fails here without
  paying for its codegen
- `cargo test -p quality` — the ratchet, always
- `cargo test -p <crate>...` for the crates the PR touched; with no crates named,
  `cargo test --workspace --lib --bins --tests` (no example or bench codegen)

```powershell
./scripts/preflight.ps1                               # thirty seconds: rustfmt + the quality ratchet
./scripts/verify-pr.ps1 -Crates client,scene_build   # before a PR (quality and tools ride along)
./scripts/verify.ps1                                  # once a day over what landed, and before
                                                      # any merge that touches examples, benches,
                                                      # the wire, replay fixtures or physics numbers
./scripts/verify.ps1 -Deep                            # the same, compiled without incremental state
```

Why two (Inny Poziom Q6, 2026-09-02): a full run on the reference laptop took 25–35 minutes,
most of it compiling forty probe modules and every client test target with codegen for a
change that touched neither. The PR gate keeps what catches a broken PR and drops what only
catches a broken day; the full gate still runs, over a batch instead of a row. The other half
of the cost was a fresh worktree per PR — cargo keys a workspace crate's artifacts by its path,
so a new path rebuilt all thirty-three crates every time. A session keeps ONE worktree for its
whole run and branches inside it.

The third half of the cost (2026-09-06): feature ping-pong. The resolver unifies a third-party
crate's features only across the packages named on the command line, so `cargo test -p sim`,
`cargo clippy --workspace --all-targets` and `cargo run -p client` each resolved a different graph
(`log` without `std`, `smallvec` without `union`, `windows-sys` without 37 features, `glam` with
and without `nostd-libm`) and each rebuilt what the previous one had built: measured, a workspace
test build straight after a per-crate one rebuilt 696 units in 3 min 01 s, and the per-crate
build after it rebuilt again, with no source changed. `crates/foundation/workspace_hack` is the
cure: a crate with no code that declares the union of every feature any member enables, and
that every member depends on (`workspace_hack.workspace = true`, first line of every
`[dependencies]`). Its managed section is written by `cargo hakari generate`
(`cargo install cargo-hakari`, config in `.config/hakari.toml`); what hakari's model leaves out
(empty `default` features, host-only crates) goes by hand under
`[target.'cfg(all())'.dependencies]`. The lock is `quality/tests/feature_unification.rs`: it runs
`cargo tree` for the eight invocations the gates and the player use and requires one graph. A new
dependency or feature fails that test in twenty seconds, not as a mysterious rebuild.

One test binary per crate (2026-09-06). Every `tests/*.rs` file is its own program linked against
the whole dependency tree; the workspace had 314 of them — per heavy crate ~180 CPU-seconds of
linking on every gate, and clippy `--all-targets` fronting 372 targets. Each crate's integration
tests now live in `tests/suite/main.rs` as one module per former file (`tests/common` stays where
it was, reached by `#[path]`); sim's test build after a one-file touch fell from 84 s to 6 s. A new
integration test is a file under `tests/suite/` plus one `mod` line in `main.rs`, never a loose
`tests/foo.rs`. The goldens (`look_goldens`, `studio_goldens`, `protocol_snapshots`) keep their
own binaries because a re-record must not share a process with a value test of the same file. The
lock is `quality/tests/suite/test_binaries.rs`: at most one auto-discovered target per crate
besides that allowlist.

The linker (2026-09-06, step 2 of the same program): `.cargo/config.toml` at the workspace root
switches the msvc target to `rust-lld.exe` (ships with the toolchain; `linker-flavor=lld-link`).
Measured with the touch protocol (one file touched, then the build), link.exe → lld:
the client test build 25 s → 12–13 s, the client dev build 11 s → 10 s, the release client build 56 s → 49–50 s, sim tests 5 s → 5 s (run-to-run spread on this laptop is ~10 %; the first lld series, taken right after a 31-minute cold rebuild, read 63 s on release — heat, not the linker). The config applies to every cargo invocation on the machine — gates,
`cargo run`, probes — so the linker never differs between the gate and the game.
`quality/tests/suite/linker_config.rs` locks it. A changed linker changes no computed number:
the replay pins and the bake-hash goldens passed unchanged through the switch.

`cargo check --workspace --all-targets` is deliberately NOT a separate gate: clippy
`--all-targets` already runs the full compiler front-end over every target, so a second
check would be redundant work (the script says so). The benchmark compile
(`cargo bench --workspace --no-run`) runs only behind `./scripts/verify.ps1 -Release` — it is
a second, optimized build of the whole workspace and roughly doubles wall time; run it before
cutting a release or tag. `-Deep` compiles without incremental state: after a killed build, or
when a number looks wrong for no reason in the diff. A stale cache once produced a client test
binary whose physics disagreed with the same source compiled fresh (Q5); the gate's answer may
not depend on what a previous build left behind, so a build is never killed mid-flight, a
killed one is followed by `cargo clean -p <crate>`, and the daily full run is a `-Deep` run.

## Testing Policy

- New behavior starts with a failing test.
- Protocol changes require snapshot tests in `crates/runtime/net/tests/snapshots`.
- Simulation bugs require replay fixtures in `crates/runtime/sim/tests/replays`.
- Clock, tick-rate, and snapshot-cadence changes require policy tests in `sim`, `net`, `server`, or `client`.
- Client/server flow changes require tests proving input commands enter server code before snapshots reach render state.
- Domain-direction changes require tests or docs that preserve terrain, LOD, shadows, spotting, shell physics, and networking as priorities.
- Terrain or camera projection changes require tests for map layers, coordinate precision, and depth convention.
- Debug tooling changes require tests for debug draw primitives, overlays, GPU labels, and error policy.
- Every contact-shape approximation (collision footprint, contact predicate, hit volume, blast radius) requires a negative test: a concrete scenario that must produce **no** contact, no damage, and no event. A passing near-miss is as load-bearing as a passing hit — the 2026-06-10 review found every shipped contact bug (phantom ramming, cover interpenetration) lived exactly where only the positive case was tested.
- Performance-sensitive systems require a benchmark before they are tuned.

## Documentation Policy

- Architecture decisions live in `docs/architecture.md`, which also carries the domain narrowing.
- Terrain and coordinate policy lives in `docs/terrain-large-world-policy.md`.
- Debug tooling policy lives in `docs/debug-tools-policy.md`.
- Quality rules live here.
- Testing, snapshot, replay, and benchmark workflows live in `docs/testing-and-regression.md`.
