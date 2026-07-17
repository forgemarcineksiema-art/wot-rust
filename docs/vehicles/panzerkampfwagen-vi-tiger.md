# Panzerkampfwagen VI Tiger

## Implemented Variant

- `Panzerkampfwagen VI Tiger Ausf. E`

This is the Tiger I, not the later Tiger II. The game spec represents a late Tiger I Ausf. E
with the Maybach HL230-class power output and the 8.8 cm KwK 36 L/56 gun.

## Reference dossier (Genialna Flota W1, 2026-07-17)

Per the data-first protocol (`docs/vehicles/_template.md`): anchors verified against external
sources before any shape work. **Source discrepancy, resolved:** Wikipedia's infobox width
"3.56 m" is the hull over sponsons; German records (Jentz/Doyle via Panzerworld) give
**3.705 m over the 725 mm combat tracks** — the game models the combat configuration, so
3.705 m is the anchor. Height splits the same way: **2.885 m to the turret roof** (German
records), **3.00 m to the cupola top** (Wikipedia) — our `HeightToTurretRoof` gate measures
the full silhouette apex, so 3.00 m is the anchor and 2.885 m is a cage-level check.

| Dimension | Value | Source | Confidence | Encoded as |
| --- | ---: | --- | --- | --- |
| Hull length | 6.316 m | Wikipedia + Panzerworld agree | high | `DimensionKind::HullLength` |
| Width (combat tracks) | 3.705 m | Panzerworld (German records); Wikipedia's 3.56 = sponsons | high | `DimensionKind::HullWidth` |
| Height (cupola apex) | 3.00 m | Wikipedia; 2.885 m to turret roof per German records | high | `DimensionKind::HeightToTurretRoof` |
| Overall with gun | 8.450 m | Wikipedia + Panzerworld agree | high | `DimensionKind::OverallLengthWithGun` |
| Road wheel | ⌀0.800 m | Wikipedia (Schachtellaufwerk section) | high | `DimensionKind::RoadWheelDiameter` |
| Firing height | 2.195 m | Panzerworld; blueprint trunnion 2.17 | medium | cage (`trunnion_y`) |
| Ground clearance | 0.47 m | Wikipedia | high | cage (`belly_y`) |

## Blueprint Migration (2026-07)

The Tiger I is blueprint-born: `game_core::vehicle_blueprint::tiger_i` is the single shape
source for the hitbox, the mount frames, the armor facet slopes, the convex armor volumes, and
(via `vehicle_geometry::recipes::tiger_i`) the visible mesh. The legacy hand-authored body —
hitbox fractions plus a magic-number turret box — is gone, and with it a hitbox that was 7.2 m
long and only 2.92 m tall. The migrated body is the researched tank, a conscious gameplay
correction documented here and locked by `tiger_i_benchmark.rs`.

### Anchor dimensions (1:1)

| Anchor | Value | In the blueprint |
| --- | --- | --- |
| Hull length | 6.316 m | `half_len 3.16` |
| Width over combat tracks | 3.705 m | `track.outer_x 1.84`, sponsons at `half_width 1.85` |
| Height to cupola top | 3.00 m | `deck_y 1.90`, `roof_y 2.72`, drum cupola to 3.01 |
| Ground clearance | 0.47 m | `belly_y 0.47` |
| Road wheels | 8 × ⌀0.80 m interleaved | `wheel_count 8`, `overlap_inner_dx 0.22` |
| Track width | 725 mm | `inner_x 1.13 .. outer_x 1.84` |
| Contact run | ~3.6 m | `wheel_first_z/last_z ±1.80` |
| Overall with gun | 8.45 m | `muzzle_z 5.29` |
| Fire line | ~2.17 m | `trunnion_y 2.17` |

### The slab, honestly

The Tiger's character is the ABSENCE of slope, and the armor model now says exactly that:

- Driver's plate ~9° from vertical (`hull_front (9.0, 0.9)` — was a rounded 10° before).
- Sides genuinely vertical (`hull_side (0.0, 1.0)`), rear at its real 8°.
- The turret walls stand straight up; only the front plate leans its 8°
  (`turret_front (8.0, 0.92)` keeps the mantlet weakspot).

Because the vehicle carries armor VOLUMES (a `WeldedBox` plate prism for the turret instead of
the cast-dome sectors), the plate normals a shell meets are the flat plates you see: angling
the hull is what changes the presented angle, exactly like the real crews were taught. The
visible hull front/rear plates are authored on the same plane equations the volumes bake
(`tiger_slab_hull`), locked by `the_visible_drivers_plate_is_the_armor_glacis_plane`.

### Recognition features carried by the recipe

- Horseshoe turret: flat front plate, vertical bent side wall, faceted rear.
- Rommelkiste stowage bin closing the turret's REAR armor plane — the plate a shell into the
  bustle actually meets.
- Drum (not domed) commander's cupola on the left rear roof, topping out the 3.0 m silhouette.
- Interleaved Schachtellaufwerk: two genuinely separate wheel rows per side, odd wheels
  0.22 m inboard, no return rollers — the top run rests on the wheels.
- 8.8 cm KwK 36 with its double-baffle muzzle brake and no bore evacuator.
- Twin exhaust stacks standing proud on the rear plate; driver's visor and bow-MG ball on the
  near-vertical front.

### What deliberately changed for gameplay (re-recorded consciously)

- Hitbox: 7.20 × 3.90 × 2.92 m → 6.44 × 3.74 × 3.01 m (shorter, narrower, taller — the real
  proportions; the Tiger is now honestly harder to hide hull-down and easier to hit tall).
- Fire line raised 2.02 → 2.17 m; muzzle reach 5.85 → 5.29 m (the L/56 is not an L/71).
- Armor resolution moved from facet BANDS to blueprint volumes: track boxes act as spaced
  armor, the mantlet is a true patch on the front plate, roof shots resolve on a real roof
  plane.
- Contact footprint: eight real wheel stations (was a five-station hitbox estimate), so trench
  bridging and crest behavior follow the actual running gear.

## Data Sources And Gameplay Translation

The implemented values are practical gameplay specs grounded in public historical data.

Reference points:

- [Panzerworld Tiger Ausf. E](https://panzerworld.com/pz-kpfw-tiger-ausf-e): Maybach HL230 P45,
  700 net hp, 8.8 cm KwK L/56, ammunition and fuel data.
- [Tiger I overview](https://en.wikipedia.org/wiki/Tiger_I): 100 mm frontal hull and turret
  armor, dimensions (6.316 m hull, 3.705 m width, 3.00 m height), interleaved running gear.
- [OnWar Tiger I data](https://www.onwar.com/wwii/tanks/germany/ge067tiger1.html):
  Panzerkampfwagen VI Ausf. E designation, 57,000 kg combat weight, 88 mm KwK 36 L/56,
  38 km/h speed, armor table.
- Wikimedia Commons photo galleries for the horseshoe turret, Rommelkiste, cupola position,
  exhaust stacks, and wheel interleave used by the Forge reference pack ratio gates.

## Current Gameplay Spec

- Name: `Panzerkampfwagen VI Tiger Ausf. E`
- Mass: 57,000 kg
- Engine: 515 kW Maybach HL230-class
- Max forward speed: 10.56 m/s
- Gun: 8.8 cm KwK 36 L/56 (92 rounds)
- Armor model: 100 mm hull front @9°, 80 mm vertical hull side, 80 mm hull rear @8°,
  100 mm turret front @8° (mantlet weakspot 0.92), vertical turret sides/rear — thicknesses
  from the installed modules, geometry from the blueprint.

## Asset

Generated vehicle asset:

```text
assets/vehicles/tiger_i_ausf_e.vehicle.json
```

Regenerate it with:

```powershell
cargo run -p tools -- generate-vehicle --vehicle tiger-i-ausf-e --output assets/vehicles/tiger_i_ausf_e.vehicle.json
```
