# Prokhorovka - Hill 252.2 Sector

## Historical Basis

This terrain profile is based on the Prokhorovka battlefield on 12 July 1943, compressed into a playable 1000m x 1000m training map. It uses the Hill 252.2 / Oktyabrskiy State Farm sector as the core historical reference, with the Psel lowland and railway embankment represented as important terrain boundaries.

Sources used for layout decisions:

- [Battle of Prokhorovka](https://en.wikipedia.org/wiki/Battle_of_Prokhorovka)
- [Ribbentrop at Prokhorovka](https://warfarehistorynetwork.com/article/ribbentrop-at-prokhorovka/)
- [Prokhorovka map overview, Liquipedia World of Tanks](https://liquipedia.net/worldoftanks/Prokhorovka)

## Playable Shape

- Size: 1000m x 1000m (matches the standard World of Tanks Prokhorovka footprint).
- Height samples: 201 x 201 at 5m spacing.
- Theme: open steppe, not urban.
- Layout: a central east-west **railway embankment** on the symmetry axis divides the sectors; the **open Psel flank** runs along the western edge (the long-sightline killzone); contested **hill high ground** (the Hill 252.2 massif) sits in the east; the **Oktyabrskiy farm** is the contested centre on the embankment, flanked by **anti-tank ditch** lines.
- Symmetry: the heightmap is mirror-symmetric across the central axis (`height_at(x, z) == height_at(x, 1000 - z)`), so both teams start on equivalent ground. This is locked by the `prokhorovka_heightmap_is_mirror_symmetric_across_central_axis` test.
- Object density: intentionally low. The runtime map carries mirrored static cover (farm barns, crossing cover, treelines, hull wrecks), but terrain remains the primary gameplay surface.

## Design Approach

The map follows a hybrid direction: a **historical skeleton** (Psel corridor, railway embankment, anti-tank ditch, Hill 252.2, Oktyabrskiy farm) arranged for **mirror-balanced** competitive play. Putting the railway embankment on the central symmetry axis lets the historical north/south divide double as the line of fairness, so we get WoT-style balance without discarding the history.

Phased build-out (each phase lands with a locking test):

- **Phase 0 (done):** 1000m scale + mirror-symmetric heightmap skeleton.
- **Phase 1 (done):** the embankment is a steep ~9 m railbed barrier with three crossings (x = 250 / 500 / 750); its face is steeper than tank gradeability, so movement funnels through the gaps.
- **Phase 2 (done):** the western Psel field is flattened into a coverless killzone (local relief ~0.9 m vs ~2.2 m in the rolling steppe), with mirrored tank-destroyer overwatch knolls (x = 235, z = 200 / 800) that hold the high ground and see across it.
- **Phase 3 (done):** each eastern hill has a crest ridge on its west shoulder with a reverse-slope shelf behind it; a tank on the shelf masks its hull behind the crest (~0.6 m) while its turret fires over it (~0.7 m clear), seen from a central attacker.
- **Phase 4a (done):** static cover (barns, treelines, wrecks) now blocks shots — the projectile sweep tests cover boxes, wired through the authoritative server. Spawns are mirrored on flat ground (since Phase 0).
- **Phase 4b (done):** tank-vs-cover driving collision lives in the shared `physics::step_tank_on_world` / `resolve_cover_collision`, used by both the sim and the client predictor, so tanks stop at cover (sliding along faces) and prediction stays in lockstep with the server.
- **Phase 5:** capture zones, minimap data, and visibility sectors (the remaining `map_plan` layers).

## Strategic Points

Points are placed as mirrored north/south pairs (or on the axis) for competitive balance:

- Hill 252.2 crest (south / north): dominant eastern high ground and observation.
- Oktyabrskiy farm rise: contested central objective on the embankment.
- Railway crossings (west / east): the discrete movement gates through the central barrier.
- Psel open flank (south / north): exposed western flanking route under long-range fire.
- Psel field overwatch (south / north): tank-destroyer knolls holding the high ground over the open field.
- Hill 252.2 hull-down shelf (south / north): reverse-slope firing positions tucked behind the crest.

## Physics

The generated heightmap is meant to be physical immediately. `physics::make_terrain_heightfield_collider()` converts the map heightmap into a Rapier heightfield collider, so tank movement can be validated against terrain instead of a flat plane.

The custom controller also has an early terrain-aware stepping hook: `physics::step_tank_on_heightmap()`. It moves the kinematic tank and grounds it to the sampled heightmap. This is not the final suspension model, but it is enough for first playable driving tests on non-flat terrain.

The controller enforces a maximum climbable grade (gradeability), so the steep railway embankment is a true barrier: tanks must funnel through the crossings rather than drive over the railbed. This is locked by the `embankment_blocks_movement_except_at_crossings` test.

## Asset

Generated map asset:

```text
assets/maps/prokhorovka_hill_252_2.terrain.json
```

Regenerate it with:

```powershell
cargo run -p tools -- generate-map --map prokhorovka-hill-252-2 --output assets/maps/prokhorovka_hill_252_2.terrain.json
```
