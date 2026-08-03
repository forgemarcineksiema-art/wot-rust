# Physics Policy

There is no physics engine. The simulation core is custom deterministic code end to end:
the hand-rolled SAT footprint test (`crates/runtime/physics/src/collision.rs`,
`obstacles_overlap` — reached from `tank_resolve.rs`, `contact_impulse.rs` and `cover.rs`),
heightmap stepping, and the running-gear support envelope
(`crates/runtime/physics/src/track_contact.rs`).

rapier3d left the workspace 2026-08-02. `parry3d` remains a dependency, but its only entry
point — `physics::parry_query::tank_footprints_intersect_query`
(`crates/runtime/physics/src/parry_query.rs:7`) — currently has ZERO production callers: the
only references are its own `#[cfg(test)]` module and the re-export. That is an OPEN decision,
not a settled one: either the query earns a production caller, or `parry3d` follows rapier out
of the workspace. `crates/tooling/quality/tests/parry_feature_rules.rs` pins the feature set
meanwhile, so a version bump cannot drag rapier back in through it.

## Custom Code Owns

Everything gameplay touches:

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

## Gravity Is A Scale Decision

`GRAVITY_MPS2 = 12.0` (`crates/foundation/game_core/src/math/mod.rs:16-17`) — deliberately
above the real 9.81; the constant's own words are "mildly exaggerated gravity so shell arcs
read at map scale". It is not a ballistics-only fudge: `forces.rs:31` binds the same value
into the hull force model, where it multiplies every grip cap — the longitudinal thrust cap
(`crates/runtime/physics/src/forces.rs:93`), the lateral friction cap (`forces.rs:148`) —
and the parked static hold (`forces.rs:66`), and `vertical.rs:74` uses it for airborne fall.
For an "honest tank" game this is a real physical exaggeration, recorded here as a
deliberate scale decision: shells drop faster and tracks grip harder than Earth allows, in
the same proportion, so arc feel and slope behavior stay mutually consistent. Changing the
number retunes gradeability, the momentum-climb band, the static hold, landing impacts, and
every firing solution at once.

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
