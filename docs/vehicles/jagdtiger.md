# Jagdtiger

## Implemented Variant

- `Panzerjager Tiger Ausf. B Jagdtiger`

This is the late-war German heavy tank destroyer on a lengthened Tiger II chassis. The game
spec models the 12.8 cm Pak 80 vehicle, not the late planned 8.8 cm Pak 43 substitute.

## Blueprint Migration (2026-07)

The Jagdtiger is blueprint-born — and the first CASEMATE on the blueprint path:
`game_core::vehicle_blueprint::jagdtiger` is the single shape source for the hitbox, mounts,
armor facet slopes, convex armor volumes (`TurretForm::Casemate` → a fixed plate prism), and
(via `vehicle_geometry::recipes::jagdtiger`) the visible mesh. The legacy hand-authored body
is gone; the migrated body is the researched vehicle, locked by `jagdtiger_benchmark.rs`.

### Anchor dimensions (1:1)

| Anchor | Value | In the blueprint |
| --- | --- | --- |
| Hull length | 7.80 m | `half_len 3.90` |
| Width over combat tracks | 3.625 m | `track.outer_x 1.80` |
| Height to casemate roof | 2.95 m | `deck_y 1.86`, `roof_y 2.88` + periscope housing |
| Ground clearance | 0.50 m | `belly_y 0.50` |
| Road wheels | 9 × ⌀0.80 m overlapped | `wheel_count 9`, `overlap_inner_dx 0.20` |
| Track width | 800 mm | `inner_x 1.00 .. outer_x 1.80` |
| Contact run | ~4.2 m | `wheel_first_z/last_z ±2.10` |
| Overall with gun | 10.65 m | `muzzle_z 6.75` — the lineup's longest reach |
| Fire line | ~2.25 m | `trunnion_y 2.25` |

### The welded-in casemate, honestly

The Jagdtiger's signature is that the fighting compartment is not a box ON the hull — it is
welded INTO it. The blueprint encodes that literally: the casemate's `plan_half_width` is
chosen so its 25° side wall lies ON the hull's own leaned-side armor plane. One unbroken slope
runs from the sponson to the roof, in the armor volumes AND in the visible mesh (both lofted
from the same plane equations), locked by
`the_casemate_flank_is_the_hull_side_plane_continued`.

- Casemate face: 250 mm at only 15° — thickness over slope — with the mantlet patch riding it.
- Hull glacis: the Tiger II school's 150 mm at 50°; rear at 30°.
- A casemate never traverses: the spec's `has_fixed_casemate` clamp holds yaw at zero and the
  armor volume is a fixed prism (no swept sectors, no ring).
- No cupola: the roof carries only the commander's low periscope housing.

### What deliberately changed for gameplay (re-recorded consciously)

- Hitbox: 8.20 × 4.00 × 2.94 m → 7.90 × 3.64 × 2.95 m (the real proportions).
- Armor geometry: hull front 45° → 50°, hull sides 0° → 25°, rear 0° → 30°, casemate sides
  5° → 25° (the unbroken flank), casemate rear 0° → 5°.
- Armor resolution moved from facet bands to blueprint volumes (spaced track boxes, true
  mantlet patch, real roof plane).
- Fire line raised 2.05 → 2.25 m; muzzle reach corrected 7.85 → 6.75 (the documented 10.65 m
  overall — the old barrel was over a metre too long).
- Contact footprint: nine real wheel stations.

## Data Sources And Gameplay Translation

Reference points:

- [Panzerworld Jagdtiger](https://panzerworld.com/jagdtiger): 75,200 kg weight, 12.8 cm Pak 80
  gun, 10 degree traverse to each side, 100-250 mm frontal armor, 80 mm side/rear armor,
  34.6 km/h maximum speed, and Maybach HL 230 P30 engine data.
- [Panzerworld 12.8 cm Pak 80](https://panzerworld.com/12-8-cm-pak-80): 128 mm caliber,
  55-caliber barrel, 920 m/s muzzle velocity and 223 mm penetration at 100 m for Pzgr 43
  APCBC-HE.
- [Tank Encyclopedia Jagdtiger](https://tanks-encyclopedia.com/ww2-germany-sd-kfz-186-jagdtiger/):
  Tiger II chassis basis, 250 mm casemate front, 150 mm glacis, 80 mm side and rear hull
  plates, and Pak 44/Pak 80 designation context.
- Wikimedia Commons photo galleries for the unbroken flank line, the low roof, and the barrel
  overhang used by the Forge reference pack ratio gates.

## Current Gameplay Spec

- Name: `Panzerjager Tiger Ausf. B Jagdtiger`
- Mass: 75,200 kg
- Engine: 441 kW Maybach HL 230 P30-class
- Max forward speed: 9.61 m/s
- Gun: 12.8 cm Pak 80 L/55
- Armor model: 150 mm hull front @50°, 80 mm hull side @25°, 80 mm hull rear @30°, 250 mm
  casemate front @15° (weakspot 0.95), 80 mm casemate sides @25°, rear @5° — thicknesses from
  the installed modules, geometry from the blueprint.

The current `ArmorProfile` has a `turret_front_mm` slot but no dedicated casemate slot. For
Jagdtiger, `turret_front_mm` intentionally stores the fixed casemate front plate. The tank
destroyer has no turret, so its turret rotation is `0.0`.

## Asset

Generated vehicle asset:

```text
assets/vehicles/jagdtiger.vehicle.json
```

Regenerate it with:

```powershell
cargo run -p tools -- generate-vehicle --vehicle jagdtiger --output assets/vehicles/jagdtiger.vehicle.json
```
