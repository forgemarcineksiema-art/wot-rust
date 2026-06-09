# Panzerkampfwagen VI B Tiger II

## Implemented Variant

- `Panzerkampfwagen VI B Tiger II`

This is the late-war German heavy tank usually called Tiger II or King Tiger. The game spec represents the production-turret vehicle with the 8.8 cm KwK 43 L/71 gun and a simplified four-facing armor model.

## Data Sources And Gameplay Translation

The implemented values are practical gameplay specs grounded in public historical data.

Reference points:

- [Panzerworld Tiger II](https://panzerworld.com/pz-kpfw-tiger-ausf-b): 69.8 t weight, 100-150 mm frontal hull armor, 80 mm side/rear hull armor, 180 mm turret front, 35 km/h maximum speed, and the 8.8 cm KwK 43 L/71 main weapon.
- [OnWar Tiger II data](https://www.onwar.com/wwii/tanks/germany/ge068tiger2.html): Panzerkampfwagen VI Ausf. B/Tiger II/Konigstiger designations, 70,000 kg combat weight, Maybach HL230P30 with 700 hp, 35-42 km/h road speed band, 150 mm superstructure front, and 180 mm turret front.
- [Panzerworld 8.8 cm KwK 43 L/71](https://panzerworld.com/8-8-cm-kw-k-43-l-71): 88 mm caliber, 71.4 caliber barrel length, 1,000 m/s muzzle velocity and 202 mm penetration at 100 m for Pzgr 39/43 APCBC-HE.

## Current Gameplay Spec

- Name: `Panzerkampfwagen VI B Tiger II`
- Mass: 69,800 kg
- Engine: 515 kW Maybach HL230P30-class
- Max forward speed: 10.56 m/s
- Gun: 8.8 cm KwK 43 L/71
- Armor model: 150 mm hull front, 80 mm simplified hull side, 80 mm hull rear, 180 mm turret front

The front hull value uses the upper front/superstructure protection because the current armor model has one hull-front slot. The forward speed uses a playable midpoint inside the 35-42 km/h road-speed band rather than the most conservative 35 km/h figure.

## Asset

Generated vehicle asset:

```text
assets/vehicles/tiger_ii_ausf_b.vehicle.json
```

Regenerate it with:

```powershell
cargo run -p tools -- generate-vehicle --vehicle tiger-ii-ausf-b --output assets/vehicles/tiger_ii_ausf_b.vehicle.json
```
