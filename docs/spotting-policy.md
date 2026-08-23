# Spotting policy

Who can see whom is decided on the server, never the client. `sim::spotting`
recomputes, on a fixed cadence, a per-tank bitmask of the teams that currently
have line of sight to it, and that mask replicates in the snapshot
(`TankSnapshot::spotted_by_teams_mask`, protocol v16).

## The rule

- **View range is per OBSERVER** (v29): `TankSpec::view_range_m()` is frozen on
  the vehicle (400 m for the late-war park, 440 m for the postwar park —
  `crates/foundation/game_core/src/tank.rs`; used at
  `crates/runtime/sim/src/spotting.rs`). The old flat
  `VIEW_RANGE_M = 400 m` survives only as the mid-park reference for fog tuning
  and bot heuristics (`spotting.rs`).
- **Concealment is stillness** (decision 2.6, 2026-08-02, `spotting.rs:49-66`):
  a hull whose planar speed is at or below `STATIONARY_SPEED_MPS = 0.5` is seen
  from only `STATIONARY_SPOT_FACTOR = 0.7` of the observer's range. Binary and
  readable — no per-vehicle camouflage percentage to memorise; creeping to a
  ridge still counts as moving.
- **Firing reveals**: a shot makes the firer fully visible for
  `FIRE_REVEAL_TICKS = 480` (8 s), keyed off the shot the sim records as a fact
  (`last_shot_tick`, fed by the protocol-v41 `ShotFired` event).
- A team spots a tank when **any living member** has an unobstructed sight line
  to it within its own range. Allies share spots **through a working radio
  only** (`spotting.rs:312`): a radio-dead crew neither lights targets for the
  team nor receives the team's — but its own eyes still count, tracked
  separately in `compute_observer_masks` and unioned back in by the per-viewer
  snapshot filter, so no enemy in plain sight ever vanishes off its screen.
- The observer looks from the **top of its hull box** (commander's eye) at the
  target's **hull centre and turret top** (two rays); either clearing counts.
- **Terrain ridges** block the line, sampled at a grid-tied step — two samples
  per heightmap cell, capped at 2 m (`terrain_sight_step_m`,
  `spotting.rs:139-141`), so a densified map cannot make the eye coarser than
  the ground — and so do **static cover boxes**: buildings, rail berms, and
  tree lines all occlude.
- **A spot holds for 2 s** after the fresh line of sight breaks
  (`SPOTTED_HOLD_TICKS = 120`, `spotting.rs:26,99-113`): the 10 Hz boolean LOS
  test would otherwise strobe a target dancing on a ridge line; the hold turns
  that into one clean spot-then-fade cycle.
- A team **always sees its own** vehicles, and a **wreck is public** to everyone
  (destroyed tanks stay on every minimap).
- Recompute runs every `SPOTTING_INTERVAL_TICKS = 6` ticks (10 Hz at the 60 Hz
  simulation), seeded on tick 0.

## Replication filtering

The mask applies to local authoritative server snapshots before they enter
the client presentation path. From the viewer's perspective, the snapshot keeps
allies, public wrecks, and live enemies spotted by the viewer's team; unspotted
live enemies are removed immediately rather than kept as last-known ghosts.

Shells and impacts are WORLD EVENTS, not intel: a tracer in the air and the dirt
a near-miss throws are visible to everyone standing there, so they always
replicate — but their OWNER is intel. Protocol v44 strips that identity from
third-party projectiles: `ShellSnapshot.owner` and `ShellImpact.owner` are
`None` whenever the viewer has not spotted the firing tank, so back-integrating a
tracer can no longer name an unspotted shooter. A `ShotFired` muzzle-flash event
(the sharpest leak, pairing shooter with shell id) is dropped entirely for an
unspotted shooter — it was never drawable anyway, as the flash is rendered only
from the shooter's pose, which the viewer does not have. Presentation keys on
`shell_id`, never on `owner`. (Residual: `shell_id` is a hash of the owner id, so
a determined packet reader could brute-force it over the ~14 tank ids; a
per-viewer shell-id remap is future work — see `docs/multiplayer-production-program.md`.)

Protocol v38 fixes the audience of reliable personal events at their
authoritative emission tick: the source and target receive their damage truth,
and only the owner receives an absorbed-shell terminal. Retransmission never
re-evaluates later spotting, so changing visibility cannot widen that audience.
Those personal events are stripped from the recipient's snapshot to avoid
duplicate presentation.

Player-owned blind-hit confirmation (a damage event whose source is the viewer,
even against an unspotted target) is a deliberate gameplay compromise: it feels
honest to the shot. It replicates the OUTCOME of the viewer's own shot, not the
identity of anyone else's gun.

Both the local authoritative path and `RemoteBattleServer` apply the same
per-client filter before a snapshot reaches the client or wire.
