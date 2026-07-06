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
