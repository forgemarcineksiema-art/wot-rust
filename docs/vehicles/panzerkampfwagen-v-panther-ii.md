# Panzerkampfwagen V Panther II

## Implemented Variant

- `Panzerkampfwagen V Panther II`

This is the improved Panther project from 1943: a Panther with heavier armor and Tiger II
component commonality goals. The game spec models the historically grounded Panther II
prototype direction, not the popular later-game fantasy of a Panther II with the Tiger II
8.8 cm KwK 43.

## Reference anatomy (W1 dossier, PR-PII.1 — 2026-07-18)

**Configuration (masterplan decision, re-confirmed here):** we model the **Fort Benning /
Patton Museum specimen** — the only Panther II that exists: the up-armoured Panther II hull
on Tiger II-commonality 800 mm steel wheels, fitted with a **Panther Ausf. G turret** and the
7.5 cm KwK 42 L/70. Happy alignment: the game spec already arms the Panther II with the
KwK 42, exactly what the specimen's G turret carries — no gameplay change needed.

Anchor numbers written BEFORE the turret swap, per protocol. HULL rows are tight (the hull
already measures 1:1); TURRET rows are authored at the **G-turret goals the current narrow
wedge does not meet**, with temporarily widened tolerances — the goal gate is on record
before any RON moves.

| Anchor | Value | Source | Confidence | Gate |
| --- | --- | --- | --- | --- |
| Hull length | 6.87 m | Panther-dimensioned prototype hull | high | `HullLength` ±0.08 |
| Width over tracks | 3.42 m | Spielberger (660 mm commonality tracks) | medium | `HullWidth` ±0.10 |
| Height to cupola | 2.99 m | specimen with G turret | medium | `HeightToTurretRoof` ±0.06 |
| Overall, gun forward | 8.86 m | KwK 42 L/70 on the G turret | high | `OverallLengthWithGun` ±0.20 TEMP → ±0.10 after PII.2 (model today: 9.03) |
| Road wheels | 7 × ⌀0.80 m overlapped steel | Tiger II commonality programme | high | `RoadWheelDiameter` ±0.01 |
| G-turret plan | ~2.4 m over ~2.1 m, rounded, bustled | Benning photo + Panther G drawings | medium | `TurretLengthToWidth` 1.15 ±0.35 TEMP → ±0.08 (model today: 1.448) |
| G-turret beam | ~0.60 of hull width | Benning photo | medium | `TurretWidthToHullWidth` 0.60 ±0.12 TEMP → ±0.05 (model today: 0.508) |

**Measured wrongness of the current wedge (the PII.2 work list, in gate numbers):**
turret beam −15.4% under the G target, plan proportion +25.9% over (too long and narrow —
a Schmalturm read), gun reach +2.0% (9.03 m vs the documented 8.86). PII.2 replaces the
wedge with the G turret: wider rounded plan with the rear bustle, the **curved cast
G-blende mantlet band across the front** (add_oval_mantlet_socket family, its own scales),
muzzle pulled to 8.86 m, cupola staying left. With it come the hull's photo items:
**Kugelblende** in the glacis right, driver periscopes on the glacis top, curved fender
sweeps and the ONE glacis headlight (F3/F4).

## Blueprint Migration (2026-07)

The Panther II is blueprint-born — and the LAST German off the legacy path, which retires the
hand-authored `legacy_tracks` table entirely (every animated vehicle now reads its blueprint's
running gear). `game_core::vehicle_blueprint::panther_ii` is the single shape source for the
hitbox, mounts, armor facet slopes, `WeldedBox` armor volumes, and (via
`vehicle_geometry::recipes::panther_ii`) the visible mesh, locked by
`panther_ii_benchmark.rs`.

### Anchor dimensions (1:1)

| Anchor | Value | In the blueprint |
| --- | --- | --- |
| Hull length | 6.87 m | `half_len 3.435` |
| Width over tracks | 3.42 m | `track.outer_x 1.70` |
| Height | 2.99 m | `deck_y 1.85`, `roof_y 2.72`, cupola to 2.99 |
| Ground clearance | 0.54 m | `belly_y 0.54` |
| Road wheels | 7 × ⌀0.80 m overlapped steel | `wheel_count 7`, `overlap_inner_dx 0.20` |
| Track width | 660 mm | `inner_x 1.00 .. outer_x 1.70` |
| Contact run | ~3.5 m | `wheel_first_z/last_z ±1.75` |
| Overall with gun | ~8.9 m | `muzzle_z 5.60` (7.5 cm KwK 42 L/70) |
| Fire line | ~2.18 m | `trunnion_y 2.18` |

### The wedge, honestly

- The 100 mm glacis leans 55° — the steepest German plate in the fleet, one ramp from the
  fold to the deck, standing ON the armor volume plane (`blueprint_prism_hull`).
- Upper sides lean their 29° (the Panther family's sponson rake; the legacy model called them
  vertical); rear at 30°.
- The Schmalturm: the narrowest turret in the German line — a small 20° face barely wider
  than the cone Saukopf mantlet, 25° cheeks converging hard to a slim roof, cupola pulled to
  the centreline. Its plates ARE the armor prism planes.
- Seven overlapped steel-rimmed wheels of the Tiger II school (the Panther II traded the
  interleaved rubber dish for the simpler stagger), no return rollers.

### What deliberately changed for gameplay (re-recorded consciously)

- Hitbox: 7.40 × 3.70 × 2.94 m → 6.98 × 3.46 × 2.99 m (the real proportions).
- Armor geometry: hull sides 0° → 29°, rear 0° → 30°, turret sides/rear 0° → 25°/20°; bands →
  volumes.
- Fire line raised 2.04 → 2.18 m; muzzle reach corrected 6.20 → 5.60 (an L/70, not an L/71 —
  the old barrel borrowed half a metre from the King Tiger).
- Contact footprint: seven real wheel stations.
- With this vehicle the `legacy_tracks` table and the last legacy recipe (`panther.rs`) are
  deleted: the German fleet is 100% blueprint-born.

## Data Sources And Gameplay Translation

Reference points:

- [Panzerworld Panther](https://panzerworld.com/pz-kpfw-panther): Panther II was initiated as
  an up-armored Panther, with 100 mm glacis, 60 mm hull sides, 120 mm turret front in later
  armor notes, Tiger II component sharing goals, and a note that the 8.8 cm KwK 43 was not
  feasible for the Panther II turret ring.
- [DeWiki Panther II](https://dewiki.de/Lexikon/Panzerkampfwagen_V_Panther_II): Jentz/Doyle-
  derived technical data lists 53 t weight, 100 mm front armor, 60 mm side armor, 40 mm rear
  armor, Maybach HL 230, 46 km/h speed, five crew, one turretless prototype, and 7.5 cm KwK
  L/70 armament.
- [Panzerworld 7.5 cm KwK 42 L/70](https://panzerworld.com/7-5-cm-kw-k-42-l-70): 75 mm
  caliber, 70-caliber barrel, 935 m/s muzzle velocity and 138 mm penetration at 100 m for
  Pzgr 39/42 APCBC-HE.
- Wikimedia Commons photo galleries for the Panther proportions, ramp line, and Schmalturm
  references used by the Forge reference pack ratio gates.

## Current Gameplay Spec

- Name: `Panzerkampfwagen V Panther II`
- Mass: 53,000 kg
- Engine: 441 kW Maybach HL 230-class
- Max forward speed: 12.78 m/s
- Gun: 7.5 cm KwK 42 L/70
- Armor model: 100 mm hull front @55°, 60 mm hull side @29°, 40 mm hull rear @30°, 120 mm
  turret front @20° (mantlet weakspot 0.9), turret sides @25°, rear @20° — thicknesses from
  the installed modules, geometry from the blueprint.

The `turret_front_mm` value uses the planned Panther II/late Panther turret-front protection.
The 8.8 cm KwK 43 is intentionally not used here because the Panther II program ended before
that later Schmalturm/firepower work and the turret-ring constraint makes it a poor
historical fit.

## Asset

Generated vehicle asset:

```text
assets/vehicles/panther_ii.vehicle.json
```

Regenerate it with:

```powershell
cargo run -p tools -- generate-vehicle --vehicle panther-ii --output assets/vehicles/panther_ii.vehicle.json
```
