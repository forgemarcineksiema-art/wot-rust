# Engineering Rules

These are hard rules for this prototype. If a rule hurts, change the design before weakening the rule.

## Crates And Files

- Keep crates small and single-purpose. A crate owns one architectural reason to change.
- Keep Rust files under 220 lines. The `quality` crate enforces this with `cargo test -p quality`.
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

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo check --workspace --all-targets`
- `cargo bench --workspace --no-run`

Run all gates with:

```powershell
./scripts/verify.ps1
```

## Testing Policy

- New behavior starts with a failing test.
- Protocol changes require snapshot tests in `crates/net/tests/snapshots`.
- Simulation bugs require replay fixtures in `crates/sim/tests/replays`.
- Clock, tick-rate, and snapshot-cadence changes require policy tests in `sim`, `net`, `server`, or `client`.
- Client/server flow changes require tests proving input commands enter server code before snapshots reach render state.
- Domain-direction changes require tests or docs that preserve terrain, LOD, shadows, spotting, shell physics, and networking as priorities.
- Terrain or camera projection changes require tests for map layers, coordinate precision, and depth convention.
- Debug tooling changes require tests for debug draw primitives, overlays, GPU labels, and error policy.
- Every contact-shape approximation (collision footprint, contact predicate, hit volume, blast radius) requires a negative test: a concrete scenario that must produce **no** contact, no damage, and no event. A passing near-miss is as load-bearing as a passing hit — the 2026-06-10 review found every shipped contact bug (phantom ramming, cover interpenetration) lived exactly where only the positive case was tested.
- Performance-sensitive systems require a benchmark before they are tuned.

## Documentation Policy

- Architecture decisions live in `docs/architecture.md`.
- Domain narrowing lives in `docs/armored-battle-domain.md`.
- Terrain and coordinate policy lives in `docs/terrain-large-world-policy.md`.
- Debug tooling policy lives in `docs/debug-tools-policy.md`.
- Quality rules live here.
- Testing, snapshot, replay, and benchmark workflows live in `docs/testing-and-regression.md`.
