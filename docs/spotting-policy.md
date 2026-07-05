# Spotting policy (LOS v1)

Who can see whom is decided on the server, never the client. `sim::spotting`
recomputes, on a fixed cadence, a per-tank bitmask of the teams that currently
have line of sight to it, and that mask replicates in the snapshot
(`TankSnapshot::spotted_by_teams_mask`, protocol v16).

## The rule (v1)

- **View range** is a flat `VIEW_RANGE_M = 400 m` (per-era tuning is a later pass).
- A team spots a tank when **any living member** has an unobstructed sight line
  to it within range — allies share spots.
- The observer looks from the **top of its hull box** (commander's eye) at the
  target's **hull centre and turret top** (two rays); either clearing counts.
- **Terrain ridges** block the line (the ground sampled along it may not rise
  above the sight line), and so do **static cover boxes** — buildings, rail
  berms, and tree lines all occlude in v1.
- A team **always sees its own** vehicles, and a **wreck is public** to everyone
  (destroyed tanks stay on every minimap).
- Recompute runs every `SPOTTING_INTERVAL_TICKS = 6` ticks (10 Hz at the 60 Hz
  simulation), seeded on tick 0.

## Honesty caveat — this gates UI, not replication

v1 produces the mask and nothing more. **The full snapshot still carries every
tank's position to every client**, so a determined client could read unseen
enemies off the wire. The client is expected to honour the mask — hiding unseen
enemies' minimap blips and floating HP bars — but that is cooperation, not
enforcement.

Real anti-wallhack is **per-client snapshot filtering** (drop or fog tanks a
client's team cannot see before serialising), a separate milestone. This mask is
its foundation: the visibility decision already lives on the server, so the
filter is a matter of applying it at send time rather than recomputing it.
