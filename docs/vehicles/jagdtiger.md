# Jagdtiger

## Implemented Variant

- `Panzerjager Tiger Ausf. B Jagdtiger`

This is the late-war German heavy tank destroyer on a lengthened Tiger II chassis. The game spec models the 12.8 cm Pak 80 vehicle, not the late planned 8.8 cm Pak 43 substitute.

## Data Sources And Gameplay Translation

The implemented values are practical gameplay specs grounded in public historical data.

Reference points:

- [Panzerworld Jagdtiger](https://panzerworld.com/jagdtiger): 75,200 kg weight, 12.8 cm Pak 80 gun, 10 degree traverse to each side, 100-250 mm frontal armor, 80 mm side/rear armor, 34.6 km/h maximum speed, and Maybach HL 230 P30 engine data.
- [Panzerworld 12.8 cm Pak 80](https://panzerworld.com/12-8-cm-pak-80): 128 mm caliber, 55-caliber barrel, 920 m/s muzzle velocity and 223 mm penetration at 100 m for Pzgr 43 APCBC-HE.
- [Tank Encyclopedia Jagdtiger](https://tanks-encyclopedia.com/ww2-germany-sd-kfz-186-jagdtiger/): Tiger II chassis basis, 250 mm casemate front, 150 mm glacis, 80 mm side and rear hull plates, and Pak 44/Pak 80 designation context.

## Current Gameplay Spec

- Name: `Panzerjager Tiger Ausf. B Jagdtiger`
- Mass: 75,200 kg
- Engine: 441 kW Maybach HL 230 P30-class
- Max forward speed: 9.61 m/s
- Gun: 12.8 cm Pak 80 L/55
- Armor model: 150 mm hull front, 80 mm simplified hull side, 80 mm hull rear, 250 mm casemate front

The current `ArmorProfile` has a `turret_front_mm` slot but no dedicated casemate slot. For Jagdtiger, `turret_front_mm` intentionally stores the fixed casemate front plate. The tank destroyer has no turret, so its turret rotation is `0.0`.

## Asset

Generated vehicle asset:

```text
assets/vehicles/jagdtiger.vehicle.json
```

Regenerate it with:

```powershell
cargo run -p tools -- generate-vehicle --vehicle jagdtiger --output assets/vehicles/jagdtiger.vehicle.json
```
