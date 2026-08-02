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

The simulation core must be controlled, repeatable, and network-stable — and it is custom
deterministic code end to end: SAT footprints, heightmap stepping, the support envelope.

**Rapier left the workspace 2026-08-02** (audit D6: `RapierWorld`, its collider constructors and
the "ownership policy" beside them were an API surface consumed only by their own tests).
`parry3d` stays, narrowly, for the footprint-intersection query (`physics::parry_query`), pinned
to `default-features = false` + `dim3`/`f32` with no SIMD/parallel features — enforced by
`quality/tests/parry_feature_rules.rs`, which also holds the door shut behind rapier: re-adding
it is a design decision with its own tests, not a dependency drive-by.

## Tank Movement

Do not start with realistic track simulation. Start with a custom kinematic controller that produces predictable acceleration, braking, steering, terrain height sampling, and replayable results. More detailed traction can be layered behind the same custom controller once network and replay behavior stay stable.

The first such layer is the running-gear support envelope (see `docs/vehicle-movement-policy.md`, "Hull Attitude and the Support Envelope"): terrain sampled at the vehicle's road-wheel stations, the hull resting as a rigid beam on the highest supports, and a rate-limited authoritative hull pitch/roll derived from that plane. It stays kinematic and deterministic — no springs, no per-link simulation — and every extension must keep the server/predictor parity and replay regression tests green.

## Water

A map's standing water is one flat table (`terrain::WaterBody { surface_level_m }`); depth
anywhere is `level − heightmap height`, so the heightmap stays the single spatial source of
truth. Wading (`physics::water`) is pure f32 arithmetic on the shared server/predictor path:
below `WADE_DRAG_START_M` water is cosmetic; above it the hull pays a speed-and-depth
proportional bow-wave drag and the riverbed cuts traction. Every formula collapses to the exact
dry value at zero depth — waterless maps are bit-identical (replay-locked, see
`physics/tests/water_wading.rs`). Drowning (engine flood + hull loss past `sim::DROWN_DEPTH_M`)
is a sim game rule, server-authoritative only — the client predicts wading drag but never
drowning damage.
