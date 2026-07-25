# Prokhorovka - Hill 252.2 Sector

The oldest map, REBUILT (2026-07-22) on its own historical skeleton — the first shipped map
whose anatomy uses the drawn-stroke vocabulary (Ręce do terenu): the steppe is now cut the
way the real one is, by balkas.

## Historical Basis

The Prokhorovka battlefield of 12 July 1943, compressed into a playable 1000m x 1000m map:
the Hill 252.2 / Oktyabrskiy State Farm sector, with the Psel lowland and railway
embankment as the terrain boundaries. Sources: the Battle of Prokhorovka literature and the
WoT map anatomy as the genre reference.

## Playable Shape

- Size: 1000m x 1000m; 201 x 201 height samples at 5 m; `MirrorZ` (mirror-symmetric across
  the embankment axis, locked by `prokhorovka_heightmap_is_mirror_symmetric_across_central_axis`).
- Theme: open steppe — `horizon: None` is deliberate (the analytic continuation reading out
  to the haze IS the historical statement; Bystra and Orliny carry the enclosed horizons).
- **Three lanes, three characters:**
  1. **West — Psel field (x 60–240):** the naked flank. Fast rotation, punished from the
     overwatch knolls (235, 200/800) and the embankment line. Stays bare by contract
     (`the_western_killzone_stays_nearly_bare`).
  2. **Centre — farm corridor (x 400–600):** the brawl. The Oktyabrskiy farm now sits on a
     REAL bench (drawn Plateau strokes seat the yard at 11 m — the "farm rise" its
     Observation point always claimed), with barns, fences, and the orchard in the central
     gate. One capture zone (`oktyabrskiy_farm`, data-only, M7).
  3. **East — the hill (x 660–950):** the hull-down duel, now with TWO systems per side:
     the west-facing crest shelf (crest x=726 / shelf x=752 — covers pushes from the
     centre) and the NEW axis-facing top ridge (a drawn Ridge stroke at z≈368/632 —
     covers the north–south standoff over the saddle).
- Flora (FL-5): the farm and field broadleaf scatters use the accepted imported textured
  tree; procedural bushes and willows retain the steppe's lowland vocabulary. `FloraBush`
  remains unauthored because its source failed the look gate.
- **The balkas (drawn Valley strokes — what the rebuild adds):**
  - The **anti-tank ditch** is a meandering balka ~110 m before the axis (x 330→700,
    3.5 m deep): the covered east–west rotation of each half. A tank in it is in FULL
    defilade from the embankment line (hull AND turret), but the hill's west-facing crest
    still looks into it — the hill's dominance stays meaningful. Edge grade ≈0.48:
    crossable everywhere (the bots' no-wall promise, locked by
    `the_ditch_balka_is_crossable_everywhere`).
  - The **Storozhevoe draw**: a diagonal balka from each spawn's east shoulder to the hill
    foot — the masked approach into the east lane; its mouth opens drivably onto the
    shelf.
- The embankment itself is unchanged: a ~9 m railbed barrier with three gates
  (x = 250/500/750); its faces exceed gradeability, so movement funnels through the gates
  (locked by physics' `embankment_blocks_movement_except_at_crossings`).

## Contracts

- `prokhorovka_hull_down.rs` — the west-facing shelf line AND the new
  `hill_top_ridge_offers_a_hull_down_line_facing_the_axis`.
- `prokhorovka_balkas.rs` — full defilade from the midline; the hill's firing line still
  sees in; crossable everywhere; the draw masks the hill approach and opens onto the
  shelf.
- `prokhorovka_field.rs` — the Psel field stays flat open killzone; the knoll overwatches
  it.
- `prokhorovka_dressing.rs` — ≥80 bushes east, mirrored dressing, bare west, mirrored
  roads incl. the ballast line, crushable fences.
- `historical_map.rs` — feature names (Hill 252.2, railway embankment, anti-tank ditch,
  Psel lowland), spawns, cover bounds, the mirror lock.
- Shared gates: the map report (playability BFS spawn→every point/zone), the golden in
  `blueprints/goldens.ron` (re-blessed with the rebuild), determinism, weather fairness.
- The three looks stay byte-identical in order [ClearAfternoon, GoldenEvening, Overcast]
  (server `match_info` weather-table lock + scene_build preset/sky table locks).

## Asset

```text
assets/maps/prokhorovka_hill_252_2.terrain.json
```

Regenerate with:

```powershell
cargo run -p tools -- generate-map --map prokhorovka-hill-252-2 --output assets/maps/prokhorovka_hill_252_2.terrain.json
```

Open in the editor / playtest:

```powershell
cargo run -p editor -- crates/world/map_forge/blueprints/prokhorovka-hill-252-2.map.ron
$env:WOT_MAP = "prokhorovka-hill-252-2"; cargo run -p client
```
