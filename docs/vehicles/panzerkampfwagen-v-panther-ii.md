# Panzerkampfwagen V Panther II

## Implemented Variant

- `Panzerkampfwagen V Panther II`

This is the improved Panther project from 1943: a Panther with heavier armor and Tiger II component commonality goals. The game spec models the historically grounded Panther II prototype direction, not the popular later-game fantasy of a Panther II with the Tiger II 8.8 cm KwK 43.

## Data Sources And Gameplay Translation

The implemented values are practical gameplay specs grounded in public historical data.

Reference points:

- [Panzerworld Panther](https://panzerworld.com/pz-kpfw-panther): Panther II was initiated as an up-armored Panther, with 100 mm glacis, 60 mm hull sides, 120 mm turret front in later armor notes, Tiger II component sharing goals, and a note that the 8.8 cm KwK 43 was not feasible for the Panther II turret ring.
- [DeWiki Panther II](https://dewiki.de/Lexikon/Panzerkampfwagen_V_Panther_II): Jentz/Doyle-derived technical data lists 53 t weight, 100 mm front armor, 60 mm side armor, 40 mm rear armor, Maybach HL 230, 46 km/h speed, five crew, one turretless prototype, and 7.5 cm KwK L/70 armament.
- [Panzerworld 7.5 cm KwK 42 L/70](https://panzerworld.com/7-5-cm-kw-k-42-l-70): 75 mm caliber, 70-caliber barrel, 935 m/s muzzle velocity and 138 mm penetration at 100 m for Pzgr 39/42 APCBC-HE.

## Current Gameplay Spec

- Name: `Panzerkampfwagen V Panther II`
- Mass: 53,000 kg
- Engine: 441 kW Maybach HL 230-class
- Max forward speed: 12.78 m/s
- Gun: 7.5 cm KwK 42 L/70
- Armor model: 100 mm hull front, 60 mm hull side, 40 mm hull rear, 120 mm turret front

The `turret_front_mm` value uses the planned Panther II/late Panther turret-front protection, while the current four-value armor model does not separately represent mantlets or roof armor. The 8.8 cm KwK 43 is intentionally not used here because the Panther II program ended before that later Schmalturm/firepower work and the turret-ring constraint makes it a poor historical fit.

## Asset

Generated vehicle asset:

```text
assets/vehicles/panther_ii.vehicle.json
```

Regenerate it with:

```powershell
cargo run -p tools -- generate-vehicle --vehicle panther-ii --output assets/vehicles/panther_ii.vehicle.json
```
