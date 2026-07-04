# Physics Policy

Rapier is a world-query and collision tool. It is not the owner of tank feel, weapon behavior, armor, or network-stable gameplay physics.

## Rapier Owns

- broadphase;
- raycasts;
- world collision shapes;
- simple rigid bodies;
- trigger volumes.

## Custom Code Owns

- tank controller;
- traction;
- hull rotation;
- turret rotation;
- gun depression;
- shell ballistics;
- armor penetration;
- damage modules.

## Determinism

The simulation core must be controlled, repeatable, and network-stable. Rapier can be locally deterministic on the same machine under the same conditions, but it is not treated as the cross-platform authoritative gameplay core.

The workspace pins `rapier3d` with `default-features = false` and only `dim3`/`f32`. Do not enable `enhanced-determinism`, SIMD, or parallel features casually. If cross-platform Rapier determinism is ever needed, it must be a separate design decision with tests and a compatibility note.

## Tank Movement

Do not start with realistic track simulation. Start with a custom kinematic controller that produces predictable acceleration, braking, steering, terrain height sampling, and replayable results. More detailed traction can be layered behind the same custom controller once network and replay behavior stay stable.

The first such layer is the running-gear support envelope (see `docs/vehicle-movement-policy.md`, "Hull Attitude and the Support Envelope"): terrain sampled at the vehicle's road-wheel stations, the hull resting as a rigid beam on the highest supports, and a rate-limited authoritative hull pitch/roll derived from that plane. It stays kinematic and deterministic — no springs, no per-link simulation — and every extension must keep the server/predictor parity and replay regression tests green.
