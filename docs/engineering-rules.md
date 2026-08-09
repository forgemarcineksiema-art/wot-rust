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

The merge gate is exactly what `scripts/verify.ps1` runs (`verify.ps1:33-41`):

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`

Run all gates with:

```powershell
./scripts/verify.ps1
```

`cargo check --workspace --all-targets` is deliberately NOT a separate gate: clippy
`--all-targets` already runs the full compiler front-end over every target, so a second
check would be redundant work (the script says so at `verify.ps1:34-35`). The benchmark
compile (`cargo bench --workspace --no-run`) runs only behind `./scripts/verify.ps1
-Release` — it is a second, optimized build of the whole workspace and roughly doubles wall
time; run it before cutting a release or tag.

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
