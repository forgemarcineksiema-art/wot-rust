# Testing And Regression

## Test Layers

- Unit and integration tests cover gameplay math, simulation stepping, networking, physics, terrain, and architecture rules.
- Protocol snapshot tests lock binary wire compatibility for message fixtures.
- Replay regression tests lock simulation outcomes from recorded inputs.
- Benchmarks track hot paths before optimization work starts.

## Protocol Snapshots

Protocol snapshot fixtures live in `crates/runtime/net/tests/snapshots`. They cover input
commands, baseline tank snapshots, and a non-empty combat snapshot with shells
and damage events.

When changing protocol encoding intentionally:

1. Run `cargo test -p net --test protocol_snapshots`.
2. Confirm the diff is a deliberate protocol version change.
3. Bump `PROTOCOL_VERSION` in `crates/runtime/net/src/lib.rs`.
4. Update the snapshot fixture.
5. Document the compatibility impact before merging.

Current compatibility note: protocol v12 adds `team` to `TankSnapshot` (the
client splits live enemies from teammates/wrecks with the same rule as the
server) and the `shell_impacts: Vec<ShellImpact>` list to `Snapshot` (absorbed
shells report where and on what surface they died). Every snapshot message of
v11 and earlier is binary incompatible with v12. Input command and vehicle
selection bytes are unchanged but belong to v12 once the shared
`PROTOCOL_VERSION` is bumped; the v12 fixtures were regenerated accordingly.

## Replays

Replay fixtures live in `crates/runtime/sim/tests/replays`.

A replay is a compact record of spawn setup, fixed tick rate, command frames, and final expectations. Any fixed simulation bug should get a replay fixture that fails before the fix and passes after it.

Replay tests are ordinary Rust tests, so they run in the full workspace gate. Keep replay fixtures small and deterministic; if a regression needs many frames later, store the replay fixture separately and document why the larger fixture exists.

Run replay regression tests with:

```powershell
cargo test -p sim --test combat_replay_regression
```

## Event Loop Tests

The desktop client uses a testable event-loop policy instead of a manual `poll_events()` loop. `client` tests cover fixed tick accumulation, redraw actions, and the separation between simulation ticks and render-on-redraw.

## Tick Policy Tests

Simulation tick policy lives in `sim`, server cadence lives in `server`, and snapshot cadence lives in `net`. Tests must prove that render frame cadence only feeds a fixed tick accumulator and does not become gameplay time.

## Server First Tests

`server` tests cover the local authoritative path used by early desktop builds:
client commands enter `LocalAuthoritativeServer`, fixed server ticks advance
simulation, and snapshots are emitted on the configured network cadence.
`quality` tests also reject client code that directly owns `SimulationState` or
steps authoritative simulation.

## Physics Policy Tests

Physics tests lock the Rapier/custom split: Rapier is used for world queries and collision helpers, while tank movement and gameplay-critical physics stay in custom deterministic code. Quality tests also lock the workspace Rapier feature set.

## Benchmarks

Benchmarks live beside the crate they measure:

- `crates/runtime/sim/benches/fixed_tick.rs`
- `crates/runtime/net/benches/protocol_codec.rs`

Compile all benchmark targets with:

```powershell
cargo bench --workspace --no-run
```

Run a specific benchmark with:

```powershell
cargo bench -p sim --bench fixed_tick
cargo bench -p net --bench protocol_codec
```
