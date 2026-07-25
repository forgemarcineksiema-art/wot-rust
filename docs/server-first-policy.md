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

Plain UDP through `net::transport` — no async runtime, no streams. The protocol is designed for
a lossy wire: snapshots are FULL state every 50 ms (a lost datagram is answered by the next one),
while `InputBatch` repeatedly carries up to twelve oldest unacknowledged commands. A small
`InputAck` repeats every server tick independently of fragmented snapshots; the ACK embedded in
`SnapshotDelivery` remains aligned with that exact authoritative state for prediction replay.
Protocol v38 also moves personal transient combat truth off the snapshot cadence. Damage involving
the player and terminal impacts of the player's own shells enter a per-session ordered queue,
repeat in a single-datagram `CombatEventBatch` until `CombatEventAck`, and are deduplicated before
recording, audio, FX, or HUD presentation. The queue is bounded at 256 events; sequence gaps,
an event that cannot fit one datagram, or overflow end the session loudly instead of silently
dropping a hit or kill.
Every remote message carries a v38 session id, so a fast reconnect on the same UDP four-tuple
starts sequence zero cleanly and delayed traffic from the retired session is ignored before
liveness or gameplay side effects. Hello resends with backoff and `StartBattle` repeats until the
first valid batch acknowledges the seat. Messages fragment at 1150 B; an incomplete snapshot is
abandoned, not retransmitted. `MemoryHub`/`LossyLoopback` make every network test deterministic.

Remote control is valid only while the session is connected, snapshots remain fresh, and the
battle has no outcome. A timeout, transport error, ten seconds without world snapshots, exhausted
input/event backlog, combat-event sequence failure, or `BattleEnded` retires pending
command/fire/ammo edges and freezes the local prediction anchors and motion. The client keeps
pumping final lifecycle/state traffic and rendering, but it cannot drive a private "zombie" tank
over a frozen authoritative world. The HUD gives the authoritative result priority; otherwise it
says `CONNECTION LOST` or `BATTLE OVER` and keeps the garage exit available.

**Lag compensation is deliberately deferred.** Shells are simulated objects flying for many ticks
on the shared ballistic integrator — the player leads the target anyway, so the artifact reduces
to the fire command's transit (RTT/2, capped by `max_prediction_ticks` ≈ 133 ms), below the
perception threshold for sub-60 km/h hulls on regional hosting. The future step, if the beta
demands it, is rewinding ONLY the shooter's turret/gun angles at the fire command — never a full
world rewind. Anti-wallhack, spotting-per-era, the radio gate, distant-HP quantization and the
dead-viewer rule all run server-side in `filtered_for_viewer_with_observers`, before the wire.
