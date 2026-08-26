# Testing And Regression

## Test Layers

- Unit and integration tests cover gameplay math, simulation stepping, networking, physics, terrain, and architecture rules.
- Protocol snapshot tests lock raw binary payload compatibility for message fixtures.
- Protocol frame tests lock the transport header, handshake messages, and on-wire protocol version.
- Replay regression tests lock simulation outcomes from recorded inputs.
- Benchmarks track hot paths before optimization work starts.

## Protocol Snapshots

Protocol snapshot fixtures live in `crates/runtime/net/tests/snapshots`. They cover input
commands, vehicle selection, baseline tank snapshots, and a non-empty combat snapshot with shells
and damage events. Protocol v38 fixtures additionally lock `SnapshotDelivery`, `InputAck`,
`CombatEventBatch`, `CombatEventAck`, and the append-only combat-event truth fields. These fixtures
exercise `encode_message` / `decode_message`, the raw bincode payload used inside transport frames.

Transport framing is covered separately by `crates/runtime/net/tests/protocol_frame.rs`.
`encode_frame` prefixes every payload with the `WOT1` magic and the current
`PROTOCOL_VERSION`; `decode_frame` rejects short headers, bad magic, and mismatched versions before
decoding the payload. `ClientHello` and `ServerHello` messages advertise the same protocol version
at connection setup.

When changing protocol encoding intentionally:

1. Run `cargo test -p net --test protocol_snapshots`.
2. Confirm the diff is a deliberate protocol version change.
3. Bump `PROTOCOL_VERSION` in `crates/runtime/net/src/lib.rs`.
4. Set `REGEN_WIRE_FIXTURES=1` for one deliberate test run to rewrite the named fixtures.
5. Remove that variable and rerun the same test clean.
6. Document the compatibility impact before merging.

Current compatibility note: the transport frame carries `PROTOCOL_VERSION = 49`
(`crates/runtime/net/src/lib.rs`; this number is gate-locked against that constant by
`crates/tooling/quality/tests/roadmap_claims.rs`); a peer with a different version fails the
frame before payload decode. Recent versions: v44 withholds a third-party projectile's owner
from a viewer who has not spotted the shooter (`owner: Option<TankId>` — a type break); v45
puts the battle clock on the wire (`StartBattle.time_limit_tick`); v46 adds team-private crew
battle wounds (crew masks + first-aid countdowns); v47 carries concrete-round identity
(`round: Option<RoundId>`) and the tungsten `shattered` flag; v48 deletes the test-only
`PrototypeMedium`, shifting every `VehicleKind` wire discriminant (the same class of break as
v33). Earlier: v43 replicates a lit rack's fuze countdown (concealed from enemies by the
snapshot filter); v42 adds the ammo-rack cook-off damage cause; v41 replicates guns fired
this tick as `ShotFired` events; v40 appends `ArmorZone::HullDeck`; v39 moves persistent
armor perforations onto the reliable event lane; v38 added the per-session reliable
personal-combat lane and authoritative event identity/tick/shell/lethal truth; v37
introduced session ids, lightweight input ACKs and snapshot-aligned prediction replay; v16
introduced LOS spotting masks. Older notable payload breaks include v12 adding `team` to
`TankSnapshot` and `shell_impacts: Vec<ShellImpact>` to `Snapshot`, v14 adding hull
pitch/roll, and v15 adding ammo state.

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

There is no engine to split against: rapier3d left the workspace 2026-08-02, and the tests
say so. `crates/runtime/physics/tests/physics_policy.rs` locks the one promise worth
locking — the custom tank controller replays the same inputs bit for bit (its header notes
the old "ownership policy" tests died with it: they described a rapier integration that
never existed). `crates/tooling/quality/tests/parry_feature_rules.rs` pins the surviving
`parry3d` dependency: the workspace manifest must contain no "rapier", and parry stays at
`default-features = false` + `dim3`/`f32` with no SIMD or parallel features, so a geometry
library on the authoritative path cannot quietly grow nondeterminism.

## Benchmarks

Benchmarks live beside the crate they measure:

- `crates/runtime/sim/benches/fixed_tick.rs`
- `crates/runtime/sim/benches/combat_hot_path.rs` — the 14-tank battle hot path (the
  destruction/combat budget gate)
- `crates/runtime/sim/benches/contact_pileup.rs` — fourteen hulls crushed into one point,
  the contact-solver worst case
- `crates/runtime/net/benches/protocol_codec.rs`
- `crates/runtime/battle_host/benches/battle_tick.rs` — the authoritative host tick
- `crates/kernels/sdf_mesh/benches/meshing.rs` — SDF surface-nets meshing

Compile all benchmark targets with:

```powershell
cargo bench --workspace --no-run
```

Run a specific benchmark with:

```powershell
cargo bench -p sim --bench fixed_tick
cargo bench -p net --bench protocol_codec
```
