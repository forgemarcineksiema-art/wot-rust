# Server First Policy

Multiplayer architecture starts now, even while the first server runs locally.
The client must not be a singleplayer game that later grows networking.

## Required Flow

The first playable loop is:

1. client input becomes `ClientInputCommand`,
2. server simulation advances authoritative state,
3. server emits scheduled snapshots,
4. client stores snapshots for interpolation,
5. renderer presents interpolated state.

## Ownership

- `game_core` owns durable gameplay data.
- `sim` owns deterministic state transitions.
- `server` owns authoritative simulation state.
- `client` owns input capture, prediction/interpolation state, renderer, and UI.
- `renderer_wgpu` owns GPU resources only.

The client may create a `LocalAuthoritativeServer` for early desktop builds, but
that object is still the server path. Client code must not instantiate
`SimulationState`, call `apply_commands`, or spawn authoritative tanks directly.
Local bot battles follow the same rule: the client submits only the player's
`TankCommand`, while `LocalAuthoritativeServer` appends deterministic bot
commands before advancing the authoritative simulation.

## Why This Exists

Keeping the server path alive from week one makes these systems natural instead
of retrofits:

- replay regression,
- bots,
- spectator mode,
- lag compensation,
- anti-cheat validation,
- dedicated server,
- matchmaker integration.

## The shipped transport (N1–N5, 2026-07-14)

Plain UDP through `net::transport` — no async runtime, no streams. The protocol was designed for
a lossy wire: snapshots are FULL state every 50 ms (a lost datagram is answered by the next one),
inputs ride redundantly (`InputBatch` re-carries the last three commands), and the only two
reliable words are the hello exchange (resend with backoff) and `StartBattle` (repeated until the
first InputBatch acks it). Messages fragment at 1150 B; an incomplete snapshot is abandoned, not
retransmitted. `MemoryHub`/`LossyLoopback` make every network test deterministic.

**Lag compensation is deliberately deferred.** Shells are simulated objects flying for many ticks
on the shared ballistic integrator — the player leads the target anyway, so the artifact reduces
to the fire command's transit (RTT/2, capped by `max_prediction_ticks` ≈ 133 ms), below the
perception threshold for sub-60 km/h hulls on regional hosting. The future step, if the beta
demands it, is rewinding ONLY the shooter's turret/gun angles at the fire command — never a full
world rewind. Anti-wallhack, spotting-per-era, the radio gate, distant-HP quantization and the
dead-viewer rule all run server-side in `filtered_for_viewer_with_observers`, before the wire.
