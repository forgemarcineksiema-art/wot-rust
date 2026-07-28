# Centurion Mk 3

## Implemented Variant

- `Centurion Mk 3`

The British universal tank of the early Cold War, in its 1948-52 Mk 3 form with the 84 mm
Ordnance QF 20-pounder. The Centurion opens the game's third nation (`Nation::Britain`) and
fights in Era III against the T-54/T-55/IS-3 park.

## Blueprint-born (2026-07)

The Centurion was born on the blueprint — no legacy phase ever existed for it.
`game_core::vehicle_blueprint::centurion` is the single shape source for the hitbox, mounts,
armor facet slopes, armor volumes, and (via `vehicle_geometry::recipes::centurion`) the
visible mesh, locked by `centurion_benchmark.rs` from day one.

### Anchor dimensions (1:1)

| Anchor | Value | In the blueprint |
| --- | --- | --- |
| Hull length | 7.60 m | `half_len 3.80` |
| Width over skirts | ~3.34 m | `track.outer_x 1.58` + skirt standoff/sheet |
| Height | 2.99 m | `deck_y 1.88`, `roof_y 2.78`, cupola to 2.99 |
| Ground clearance | 0.51 m | `belly_y 0.51` |
| Road wheels | 6 × ⌀0.61 m in 3 Horstmann bogies | `wheel_stations` in PAIRS, axle at `axle_y` |
| Track width | 610 mm | `inner_x 0.97 .. outer_x 1.58` |
| Overall with gun | 9.83 m | `muzzle_z 6.03` |
| Fire line | ~2.16 m | `trunnion_y 2.16` |

### The skirt and the bogie, honestly

- **The fleet's first authored skirt**: full-length bazooka plates from the fender line down
  over the wheel tops, hung one 8 cm standoff outside the track. The armor volumes bake them
  as `ArmorZone::Skirt` — a thin spaced SCREEN a HEAT jet detonates against early (the PR 0
  plumbing built exactly for this vehicle), while AP only pays the sheet. A skirt hit never
  degrades a track and never rolls a module. The visible plate stands on the same plane the
  screen resolves on.
- **Horstmann bogies**: six wheels in three PAIRS — tight in-pair pitch, a real gap between
  bogies — the single `wheel_stations` source both the rendered gear and the physics contact
  footprint ride. Three return rollers carry the top run over the bogie gaps.
  Tight is not merged: a bogie's ⌀0.61 wheels run 0.66 apart, leaving 5 cm of daylight between
  the tyres. The layout was born with ⌀0.80 wheels on a 0.60 pitch — a quarter of a wheel inside
  its own pair, and the middle roller 6.5 cm inside both centre wheels — because the wheel had to
  be big enough to bridge a belt the ROLLERS carry. `axle_y` now states the axle line outright,
  so the wheel is sized by the vehicle instead of by the belt, and `fleet_running_gear.rs` holds
  the whole fleet to it.
- The 76 mm glacis leans 57° — out-sloping every German plate (only the Soviet 60° school
  leans harder), standing on its armor plane via the shared `blueprint_prism_hull`.
- The cast Mk 3 dome overhangs its 74-inch race, with the signature bustle stowage bin
  closing the rear of the turret plan.
- The 20-pounder Type A is a CLEAN tube: no muzzle brake, no fume extractor — the only
  unadorned barrel among the big guns, locked by the cage.

## Gameplay shape

The gunnery trade: the Centurion gives up alpha and pace for the best gun handling and optics
in Era III.

- ~49 t, 480 kW Rolls-Royce Meteor (petrol — higher fire chance than the diesels): 34.6 km/h
  flat out. Position with intent; you will not out-run a T-54.
- 84 mm 20-pounder: 240 alpha on an 8.0 s reload, 230 mm AP at 100 m, 1,020 m/s — tighter
  dispersion (2.4 mrad) and faster settle than any D-10. The second slot is **APDS** at
  1,465 m/s: the fastest, flattest shell in the game, whose penetration bleeds hard with
  range like the sub-caliber round it is.
- 65 rounds stowed — the deep rack the honest-ammo economy rewards.
- Armor plays as geometry + screens: the 57° glacis bounces what its 76 mm never could flat,
  the skirts murder HEAT into the sides, and the 152 mm turret face carries the fight
  hull-down — but the flat 51 mm side behind the skirt is honest against AP.

## Modules (stock)

| Slot | Module | Notes |
| --- | --- | --- |
| Gun | 84 mm 20-pounder Type A | AP 230 mm @ 100 m / 240 HP; APDS 300 mm @ 1,465 m/s |
| Gun (alt) | 84 mm 20-pounder Type B | fume extractor: tighter + faster settle, slower load |
| Engine | Rolls-Royce Meteor | 480 kW petrol, fire chance 0.14 |
| Suspension | Horstmann bogies | 3 bogie pairs/side, 52 t load limit |
| Turret | Centurion Mk 3 turret | 152/112/90 mm cast, 0.44 rad/s, view 390 m |
| Radio | WS No. 19 | 680 m |

## Data Sources And Gameplay Translation

- [Tank AFV Centurion](https://tank-afv.com/coldwar/UK/centurion.php): Mk 3 dimensions
  (7.6 m hull, 3.39 m width, ~2.9 m height), 76 mm/57° glacis, 152 mm turret, 20-pounder
  armament, Meteor engine, 34.6 km/h.
- [Wikimedia Commons Centurion gallery](https://commons.wikimedia.org/wiki/Centurion_tank):
  photo reference for the skirt line, bogie pairs, turret casting and bin (Forge pack ratio
  gates).
- 20-pounder: 84 mm, L/66.7, ~1,020 m/s APCBC and APDS at ~1,465 m/s; Type A (plain) vs
  Type B (fume extractor) barrels — both fielded, modeled as the two gun options.

## Asset

Generated vehicle asset:

```text
assets/vehicles/centurion_mk3.vehicle.json
```

Regenerate it with:

```powershell
cargo run -p tools -- generate-vehicle --vehicle centurion-mk3 --output assets/vehicles/centurion_mk3.vehicle.json
```
