# Panzerkampfwagen VI B Tiger II

## Implemented Variant

- `Panzerkampfwagen VI B Tiger II`

This is the late-war German heavy tank usually called Tiger II or King Tiger. The game spec
represents the production (Henschel) turret vehicle with the 8.8 cm KwK 43 L/71 gun.

## Blueprint Migration (2026-07)

The Tiger II is blueprint-born: `game_core::vehicle_blueprint::tiger_ii` is the single shape
source for the hitbox, the mount frames, the armor facet slopes, the convex armor volumes, and
(via `vehicle_geometry::recipes::tiger_ii`) the visible mesh. The legacy hand-authored body —
an 8.0 m box with vertical sides — is gone; the migrated body is the researched tank, a
conscious gameplay correction documented here and locked by `tiger_ii_benchmark.rs`.

### Anchor dimensions (1:1)

| Anchor | Value | In the blueprint |
| --- | --- | --- |
| Hull length | 7.38 m | `half_len 3.69` |
| Width over combat tracks | 3.755 m | `track.outer_x 1.87`, sponsons at `half_width 1.85` |
| Height | 3.09 m | `deck_y 1.86`, `roof_y 2.75`, cupola to 3.09 |
| Ground clearance | 0.50 m | `belly_y 0.50` |
| Road wheels | 9 × ⌀0.80 m overlapped | `wheel_count 9`, `overlap_inner_dx 0.20` |
| Track width | 800 mm | `inner_x 1.07 .. outer_x 1.87` |
| Contact run | ~4.1 m | `wheel_first_z/last_z ±2.06` |
| Overall with gun | 10.29 m | `muzzle_z 6.60` |
| Fire line | ~2.21 m | `trunnion_y 2.21` |

### The slope, honestly

Where the Tiger I is the vertical slab, the Tiger II is the slope the Germans learned from the
T-34 — and the armor model now says exactly that:

- The fleet's longest glacis: 150 mm at 50° (`hull_front (50.0, 1.0)`), one plate from the
  fold to the deck, over a metre of run in plan.
- Upper hull sides LEANED 25° over the tracks (`hull_side (25.0, 1.0)` — the legacy model
  called them vertical), rear at 30°.
- The Henschel turret's plates carry their real angles: front 10° (mantlet weakspot 0.9),
  long converging sides 21°, bustle rear 20° — a `WeldedBox` plate prism, so the flat normals
  you see are the normals a shell meets.

The visible hull is lofted directly from the armor volumes' plane equations
(`blueprint_prism_hull`): the glacis, the leaned sides, and the rear pair are the SAME planes
the penetration model resolves, locked by the benchmark cage.

### Recognition features carried by the recipe

- The longest glacis in the fleet, running from the nose fold to the deck in one plate.
- Henschel turret: narrow leaned front plate, long 21° converging walls, and the bustle whose
  rear wall closes the REAR armor plane at the ring seat.
- Low commander's cupola on the left rear roof.
- Nine overlapped (not interleaved) road wheels per side in two staggered rows.
- Five metres of KwK 43 barrel past the trunnion, tipped with its double-baffle brake.
- Twin exhausts hugging the 30° rear plate; driver's periscope housing on the glacis.

### What deliberately changed for gameplay (re-recorded consciously)

- Hitbox: 8.00 × 3.90 × 3.08 m → 7.48 × 3.78 × 3.09 m (the real proportions; the old box was
  0.6 m too long).
- Armor geometry: hull sides 0° → 25° leaned (a genuine protection buff the real vehicle had),
  rear 0° → 30°, turret sides/rear 0° → 21°/20°, turret front 12° → 10°.
- Armor resolution moved from facet bands to blueprint volumes: track boxes act as spaced
  armor, the mantlet is a true patch on the front plate, roof shots resolve on a real roof.
- Fire line raised 2.06 → 2.21 m; muzzle reach corrected 6.70 → 6.60 (the documented 10.29 m
  overall).
- Contact footprint: nine real wheel stations (was a five-station hitbox estimate).

## Data Sources And Gameplay Translation

Reference points:

- [Panzerworld Tiger II](https://panzerworld.com/pz-kpfw-tiger-ausf-b): 69.8 t weight,
  100-150 mm frontal hull armor, 80 mm side/rear hull armor, 180 mm turret front, 35 km/h
  maximum speed, and the 8.8 cm KwK 43 L/71 main weapon.
- [OnWar Tiger II data](https://www.onwar.com/wwii/tanks/germany/ge068tiger2.html):
  Panzerkampfwagen VI Ausf. B/Tiger II/Konigstiger designations, 70,000 kg combat weight,
  Maybach HL230P30 with 700 hp, 35-42 km/h road speed band, 150 mm superstructure front, and
  180 mm turret front.
- [Panzerworld 8.8 cm KwK 43 L/71](https://panzerworld.com/8-8-cm-kw-k-43-l-71): 88 mm
  caliber, 71.4 caliber barrel length, 1,000 m/s muzzle velocity and 202 mm penetration at
  100 m for Pzgr 39/43 APCBC-HE.
- Wikimedia Commons photo galleries for the Henschel turret facets, bustle, cupola position,
  and wheel stagger used by the Forge reference pack ratio gates.

## Current Gameplay Spec

- Name: `Panzerkampfwagen VI B Tiger II`
- Mass: 69,800 kg
- Engine: 515 kW Maybach HL230P30-class
- Max forward speed: 10.56 m/s
- Gun: 8.8 cm KwK 43 L/71 (84 rounds)
- Armor model: 150 mm hull front @50°, 80 mm hull side @25°, 80 mm hull rear @30°, 180 mm
  turret front @10° (mantlet weakspot 0.9), 80 mm turret sides @21°, rear @20° — thicknesses
  from the installed modules, geometry from the blueprint.

The forward speed uses a playable midpoint inside the 35-42 km/h road-speed band rather than
the most conservative 35 km/h figure.

## Asset

Generated vehicle asset:

```text
assets/vehicles/tiger_ii_ausf_b.vehicle.json
```

Regenerate it with:

```powershell
cargo run -p tools -- generate-vehicle --vehicle tiger-ii-ausf-b --output assets/vehicles/tiger_ii_ausf_b.vehicle.json
```
