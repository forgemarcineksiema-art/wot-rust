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
owners. Combat events are kept when both parties are visible, and player-owned
feedback remains visible: shots fired by the player and damage taken by the
player are preserved even when the other tank is currently hidden. That is a
deliberate first-slice gameplay compromise: blind-hit confirmation feels better,
but it is weaker than a fully redacted anti-wallhack model.

Real network transport must apply this same per-client filter at send time
before serialising a snapshot for each connected client. Until that transport
exists, the in-process local server is the enforced path.
