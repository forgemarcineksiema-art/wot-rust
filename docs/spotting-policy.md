# Spotting policy (LOS v1)

Who can see whom is decided on the server, never the client. `sim::spotting`
recomputes, on a fixed cadence, a per-tank bitmask of the teams that currently
have line of sight to it, and that mask replicates in the snapshot
(`TankSnapshot::spotted_by_teams_mask`, protocol v16).

## The rule (v1)

- **View range** is a flat `VIEW_RANGE_M = 400 m` (per-era tuning is a later pass).
- A team spots a tank when **any living member** has an unobstructed sight line
  to it within range; allies share spots.
- The observer looks from the **top of its hull box** (commander's eye) at the
  target's **hull centre and turret top** (two rays); either clearing counts.
- **Terrain ridges** block the line (the ground sampled along it may not rise
  above the sight line), and so do **static cover boxes**: buildings, rail
  berms, and tree lines all occlude in v1.
- A team **always sees its own** vehicles, and a **wreck is public** to everyone
  (destroyed tanks stay on every minimap).
- Recompute runs every `SPOTTING_INTERVAL_TICKS = 6` ticks (10 Hz at the 60 Hz
  simulation), seeded on tick 0.

## Replication filtering

v1 applies the mask to local authoritative server snapshots before they enter
the client presentation path. From the viewer's perspective, the snapshot keeps
allies, public wrecks, and live enemies spotted by the viewer's team; unspotted
live enemies are removed immediately rather than kept as last-known ghosts.

The same filter removes shells and absorbed-shell impact feedback from hidden
owners. Protocol v38 fixes the audience of reliable personal events at their
authoritative emission tick: the source and target receive their damage truth,
and only the owner receives an absorbed-shell terminal. Retransmission never
re-evaluates later spotting, so changing visibility cannot widen that audience.
Those personal events are stripped from the recipient's snapshot to avoid
duplicate presentation.

Visible third-party combat events and projectiles remain best-effort snapshot
feedback. Player-owned blind-hit confirmation is a deliberate gameplay
compromise: it feels honest to the shot but is weaker than a fully redacted
anti-wallhack model. Removing owner identity from third-party shell/impact
replication remains a production-networking task.

Both the local authoritative path and `RemoteBattleServer` apply the same
per-client filter before a snapshot reaches the client or wire.
