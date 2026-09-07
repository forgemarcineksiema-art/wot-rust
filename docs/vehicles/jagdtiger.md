# Jagdtiger

## Implemented Variant

- `Panzerjager Tiger Ausf. B Jagdtiger`

This is the late-war German heavy tank destroyer on a lengthened Tiger II chassis. The game
spec models the 12.8 cm Pak 80 vehicle, not the late planned 8.8 cm Pak 43 substitute.

## Reference anatomy (W1 dossier, PR-JT.1 — 2026-07-18)

Anchor numbers written BEFORE further shape work, per the masterplan protocol. Every row is
enforced by a `DimensionTarget` or `RatioTarget` in `vehicle_forge/reference/jagdtiger.reference.ron`.

| Anchor | Value | Source | Confidence | Gate |
| --- | --- | --- | --- | --- |
| Hull length | 7.80 m | tank-afv/Panzerworld (lengthened Tiger II chassis) | high | `HullLength` ±0.08 |
| Width over combat tracks | 3.625 m | Panzerworld | high | `HullWidth` ±0.08 |
| Height to casemate roof | 2.945 m | Panzerworld | high | `HeightToTurretRoof` ±0.05 |
| Overall, gun forward | 10.654 m | 12.8 cm PaK 44 L/55 datasheet | high | `OverallLengthWithGun` ±0.10 |
| Road wheels | 9 × ⌀0.80 m overlapped (production Henschel; the 8-wheel Porsche run NOT modelled) | photo + spec | high | `RoadWheelDiameter` ±0.01 |
| Casemate plan | ~3.20 m over ~2.71 m (rear overhang included) | measured on the armor-plane prism | medium | `TurretLengthToWidth` 1.18 ±0.08 |
| Silhouette ratios | 2.17 / 0.26 / 0.75 / 0.53 / 0.365 | verified 1:1 body (Studio) | high | five-ratio gate ±0.02..0.06 |

**Muzzle decision (source conflict resolved).** Our reference specimen (Aberdeen) and the
bulk of service photos show the PaK 44 with a PLAIN muzzle — the brake was proofed but not
generally fitted in service. The fleet brake pass (#234) gave our gun a double-baffle;
the dossier decides AGAINST it: **PR-JT.2 removes the brake** (the recessed bore stays).
The gun keeps its documented 10.654 m reach.

**PR-JT.2 shape list (from the photo comparison, in dossier order):**
massive CAST COLLAR of the 12.8 cm at the casemate face (today's socket ring is too slight);
spare-track hangers with shoe rows ON the casemate sides; large flat bow guards (F3);
the bow MG Kugelblende in the glacis right; plain muzzle per the decision above.

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
- A casemate never traverses: the superstructure is the hull's and the armor volume is a fixed
  prism (no swept sectors, no ring). Since S17 (2026-09-07) the GUN lays in its ball mount —
  `TurretTraverse::Limited`, 10° to either side of the hull line (Panzerworld), 0.20 rad/s (a
  decision: a hand-cranked mount on 75 t) — the sim clamps the lay to the arc, the sight
  reports `Traverse` only past it, the bots pivot the hull only past it.
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

## Library parts (2026-09-06)

The Jagdtiger is the fourth vehicle the part library builds whole and the first CASEMATE (Forge 2.0 K3): `crates/foundation/game_core/blueprints/jagdtiger.visual.ron` authors the construction, the casemate's furniture (`CasemateVisual` — its own class, not a turret with a cupola), the gun, the deck furniture and the fittings; the blueprint keeps the prism and every armour number. The recipe (`vehicle_recipes::jagdtiger`) is split into named pieces that stay out when the library builds their class; none stands on the shipped vehicle, which ships at `Fidelity::Benchmark`.

| Class | Part | Source of the shape |
| --- | --- | --- |
| Hull tub, upper hull | `slab_tub`, `slab_upper_box` — plate solids on the armour planes, the upper sides leaning 25° | The welded-in casemate, honestly |
| Turret shell | `turret_shell` — the rectangular casemate prism, front 15°, sides 25° continuing the hull's plane, rear 5° | The welded-in casemate, honestly |
| Mantlet | `mantlet_socket` — the cast collar, four rings seated on the leaning face, 1.20 m across, squashed 0.617 in Y | PR-JT.2 shape list |
| Gun barrel | `gun_barrel` bore-honest 128 mm, tube 0.135, PLAIN muzzle (no brake, the dossier's decision) | PaK 44 L/55 |
| Periscopes | `periscope_housing` — the commander's low box at the cupola station (7 cm), no cupola | The welded-in casemate, honestly |
| Hatches | `casemate_hatch_0/1` flush on the rear roof; `driver_hatch`/`radio_hatch` ahead of the casemate | JT.3 |
| Spare tracks | `spare_track` ×6 a side on `spare_track_rail`, each shoe's outer face on the 25° wall | PR-JT.2 shape list |
| Fenders | `fender_guard_0/1` — large flat bow guards with a downturned lip | PR-JT.2 (F3) |
| Course MG | `course_mg_port` — the Kugelblende at (−0.58, 1.45) in the 50° glacis (seated on the plate that folds at the sponson; the recipe's nose-line rule had it 0.47 m inside the hull) | PR-JT.2 |
| Stowage (beyond the list) | `tow_cable`, `tool_jack`, `tool_block`, `tool_box` on the leaned sponson plates; `turret_ventilator`; the open stacks with brackets | JT.3 |

Still owed: the stern plate's furniture (`SternPlate`), a reference drawing (none found on Commons) and with it the K0 outlines.
