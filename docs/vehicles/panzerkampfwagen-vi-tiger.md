# Panzerkampfwagen VI Tiger

## Implemented Variant

- `Panzerkampfwagen VI Tiger Ausf. E`

This is the Tiger I, not the later Tiger II. The game spec represents a late Tiger I Ausf. E with the Maybach HL230-class power output and the 8.8 cm KwK 36 L/56 gun.

## Data Sources And Gameplay Translation

The implemented values are practical gameplay specs grounded in public historical data.

Reference points:

- [Panzerworld Tiger Ausf. E](https://panzerworld.com/pz-kpfw-tiger-ausf-e): Maybach HL230 P45, 700 net hp, 8.8 cm KwK L/56, ammunition and fuel data.
- [Tiger I overview](https://en.wikipedia.org/wiki/Tiger_I): 100 mm frontal hull and turret armor, 8.8 cm KwK 36, HL230 output around 521 kW / 699 hp, heavy flat armor notes.
- [OnWar Tiger I data](https://www.onwar.com/wwii/tanks/germany/ge067tiger1.html): Panzerkampfwagen VI Ausf. E designation, 57,000 kg combat weight, 88 mm KwK 36 L/56, 38 km/h speed, armor table.

## Current Gameplay Spec

- Name: `Panzerkampfwagen VI Tiger Ausf. E`
- Mass: 57,000 kg
- Engine: 515 kW Maybach HL230-class
- Max forward speed: 10.56 m/s
- Gun: 8.8 cm KwK 36 L/56
- Armor model: 100 mm hull front, 80 mm simplified hull side, 80 mm hull rear, 100 mm turret front

The side armor is simplified to 80 mm for the current four-value armor model, representing the heavy side superstructure/turret protection better than the lower side plate alone.

## Asset

Generated vehicle asset:

```text
assets/vehicles/tiger_i_ausf_e.vehicle.json
```

Regenerate it with:

```powershell
cargo run -p tools -- generate-vehicle --vehicle tiger-i-ausf-e --output assets/vehicles/tiger_i_ausf_e.vehicle.json
```
