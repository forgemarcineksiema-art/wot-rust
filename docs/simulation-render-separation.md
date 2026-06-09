# Simulation/Render Separation

This is a multiplayer tank game. Rendering is presentation, while simulation and
the authoritative server own gameplay truth.

## Clock Model

- Render frame rate is variable: 60/120/144 FPS and platform dependent.
- Client simulation tick is fixed. The default is 60 Hz.
- Server tick is fixed. The default is 60 Hz.
- Network snapshots use a separate fixed schedule. The default is 20 Hz.
- Render frames may request interpolation of already-produced simulation states.
- Render frame delta time must not drive gameplay state.

## Gameplay Rules

These systems must advance from fixed simulation ticks only:

- reload time,
- turret rotation speed,
- gun dispersion,
- shell movement,
- penetration checks,
- cooldowns,
- spotting,
- camouflage,
- collisions.

The client may predict and interpolate, but server/simulation ticks are the
source of truth. Replays must store input per simulation tick so they remain
useful as regression tests.

## Code Boundaries

- `sim` exposes `FixedTimestep` and `SimulationClock`.
- `client` converts winit event-loop elapsed time into fixed tick counts.
- `RedrawRequested` renders the current frame only.
- `server` uses `ServerTickConfig` for authoritative ticks.
- `net` uses `SnapshotSchedule` for snapshot cadence.
- `renderer_api` and `renderer_wgpu` must not become gameplay clocks.
- Local desktop play still goes through the headless server path described in
  `docs/server-first-policy.md`.
