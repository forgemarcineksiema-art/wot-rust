# Centurion Mk 3

## Implemented Variant

- `Centurion Mk 3`

The British universal tank of the early Cold War, in its 1948-52 Mk 3 form with the 84 mm
Ordnance QF 20-pounder. The Centurion opens the game's third nation (`Nation::Britain`) and
sits at tier VIII on the British medium line.

## Blueprint-born (2026-07)

The Centurion was born on the blueprint — no legacy phase ever existed for it.
`game_core::vehicle_blueprint::centurion` is the single shape source for the hitbox, mounts,
armor facet slopes, armor volumes, and (via `vehicle_geometry::recipes::centurion`) the
visible mesh, locked by `centurion_benchmark.rs` from day one.

### Anchor dimensions (1:1)

| Anchor | Value | In the blueprint |
| --- | --- | --- |
| Hull length | 7.557 m (was 7.60 until 2026-09-06) | `half_len 3.7785` |
| Width over skirts | 3.38 m (was ~3.34 until 2026-09-06) | `track.outer_x 1.58` + skirt standoff 0.10 + sheet |
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
at tier VIII.

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

## Fire control (Inny Poziom A12, 2026-09-02)

- The Mk 3 carries a **vertical gun stabilizer** (the FVRDE Metadyne stabilizer under the
  20-pounder): in the sim the mount cancels every hull pitch change the same tick, so the gun
  holds its world elevation inside its −10/+18 arc while the hull works a furrow. It is the
  only stabilized vehicle in the roster — the wartime hulls and the T-54 obr. 1951 ride their
  hulls (`TankSpec::vertical_stabilizer`, locked by name in `game_core/tests/modules.rs`).
- The 20-pounder elevates at 0.9 rad/s (52 deg/s), its own number since A12; turret traverse
  36 deg/s since A11.

## Reference anatomy, researched (2026-09-06)

The Mk 3 as fielded 1948–52: the 20-pdr Type A without fume extractor, the Besa coax, no IR gear
(the searchlight is the Mk 5/2, 10 and 11's — an anachronism if drawn). Nearly every number
descends from one root document — the War Office "User Handbook for Tk., Med. Gun, Centurion,
Mk 3, 5 and 6" (1965), quoted by the [en.wikipedia infobox](https://en.wikipedia.org/wiki/Centurion_(tank)) —
so agreement between the sites below is not independence; the independent corroborations are
named where they exist.

| Dimension | Value | Source | Confidence | Encoded as |
| --- | ---: | --- | --- | --- |
| Hull length | 7.557 m (24 ft 9.5 in) | the Handbook via en.wikipedia; [Discovery UK](https://www.discoveryuk.com/military-history/the-centurion-tank-britains-post-wwii-armoured-giant/) 7.55 (de.wikipedia's 7.82 is uncited: rejected) | high | `HullLength` 7.557 ±0.04 (Locked); `half_len 3.7785` (was 3.80) |
| Overall length, gun forward | 9.83 m (32 ft 3 in) | the Handbook; a museum placard's 32 ft 4 in ([williammaloney.com](https://www.williammaloney.com/Aviation/MilitaryMuseumOfSouthernNewEngland/CenturionMainBattleTank/index.htm)) — independent | high | `OverallLengthWithGun` 9.83 ±0.05 (Locked); `muzzle_z 6.0515` |
| Width over the side plates (the bazooka plates) | 3.38 m (11 ft 1 in) | the Handbook; the same placard (11 ft 1 in) — independent | high | `HullWidth` 3.38 ±0.04 (Locked); `skirt.standoff_m 0.10` (was 0.08 — the plates stood 3.336 wide) |
| Width over the fenders, no side plates | 3.28 m (10 ft 9 in) | the Handbook | medium (one root) | — |
| Height to the cupola top | 2.94 m (9 ft 7.75 in) | the Handbook (de.wikipedia's 3.01 uncited) | medium | `HeightToTurretRoof` 2.94 ±0.05 (Locked) |
| Ground clearance | 0.51 m (1 ft 8 in) | the Handbook | high | `GroundClearance` 0.51 ±0.02 (Locked) |
| Track width | 610 mm (24 in) | the Handbook; [Missing-Lynx (Bovington specimens)](https://www.tapatalk.com/groups/missinglynx/a41-centurion-coming-t136976-s10.html); "The Centurion Tank" ([vdoc.pub](https://vdoc.pub/documents/the-centurion-tank-1mj31lde093g)) — independent | high | `TrackWidth` 0.61 ±0.01 (Locked) |
| Turret ring | 1.880 m (74 in flame-hardened race, 164 × 32 mm balls) | en.wikipedia; "The Centurion Tank" (vdoc.pub) — independent | high | `TurretRingDiameter` 1.88 ±0.02 (Locked) |
| Road wheel diameter | NOT documented (a "31.6 in" search claim could not be reproduced: not used); the blueprint's 0.61 is uncited | — | low | camera-match owed against the 0.610 track width |
| Glacis | 76 mm production plate (57 mm on the 20 prototypes) — NO source states a slope angle; the blueprint's `glacis_slope_deg 57` is suspected to be that 57 mm thickness mistaken for degrees | — | low | unchanged until a scaled photo settles it (the K21-style debt of this file) |
| Ground contact, turret plan, mantlet width | not found | — | — | camera-match owed |
| Turret armour | front 127, mantlet 152, sides/rear 76, roof 25 mm | tanks-encyclopedia (single source) | medium | — |
| Running gear | 6 × road wheels in 3 Horstmann bogies (2 wheels on nested coil springs each), sprocket REAR, idler front, return rollers; 108–109 manganese-steel links a side | en.wikipedia; [Missing-Lynx Horstmann thread](https://www.tapatalk.com/groups/missinglynx/centurion-horstmann-suspension-t339602.html) | high (layout) | `wheel_stations` in pairs ✓; `link_count 102` unverified |
| Mass | 49.5–50.8 t | the Handbook; Discovery UK | high | `TankSpec` |

## Part list (the inventory gate reads this table)

| Class | Fact | Source |
| --- | --- | --- |
| HullTub | The lower tub, floor 17 mm throughout production | "The Centurion Tank" (vdoc.pub) |
| UpperHull | One 76 mm wedge glacis inset between the tracks, straight top and bottom edges | vdoc.pub; recognition guides |
| SternPlate | The rear plate with the twin mufflers' outlets | recognition guides |
| TurretShell | The cast body with the roof WELDED in | [historyofwar.org](https://www.historyofwar.org/articles/weapons_centurion.html) |
| TurretRing | The 74 in race, electric traverse | vdoc.pub |
| TurretStowage | The bustle stowage bin | the dossier's anatomy above |
| Cupola | The commander's cupola, manual 360°, split hatches, a periscopic sight and seven periscopes | recognition guides |
| Hatches | The loader's twin covers (front/rear opening) with a periscope; the driver's split hatch front-RIGHT, two covers each with a periscope | recognition guides |
| Mantlet | The cast external mantlet, 152 mm at its thickest | tanks-encyclopedia |
| GunBarrel | The 20-pdr Type A, L/66.7, a 5.75 m clean tube — no brake, no fume extractor | [en.wikipedia 20-pounder](https://en.wikipedia.org/wiki/Ordnance_QF_20-pounder) |
| CoaxMachineGun | The 7.92 mm Besa (the Browning arrives with the Mk 5, 1954/55) | Tank Encyclopedia's Besa article |
| Periscopes | The gunner's periscopic sight; the driver's and loader's periscopes | recognition guides |
| EngineDeck | The Meteor's deck, rear | the dossier's anatomy |
| DeckGrille | The louvred intake and outlet grilles on the deck | recognition guides (photo-confirm) |
| Exhaust | Twin mufflers, one each side of the rear deck | recognition guides |
| Fenders | The long fenders running the hull's length | general references |
| FenderStowage | The fender boxes, side-loading from late 1950 | recognition guides |
| Headlights | The headlights on the fender line | general references |
| Skirts | Full-length side plates hinged for track access, the tops covered by the removable HEAT skirts | recognition guides |
| TowHooks | Tow hooks at the edge of the glacis | recognition guides |
| SuspensionHardware | Three Horstmann bogies a side, nested coil springs; the return rollers | en.wikipedia; Missing-Lynx |
| SpareTracks | Manganese-steel links, 108–109 a side, carried as spares | period spec text (108 vs 109 conflict) |

Documented absences: no searchlight on the Mk 3, no smoke dischargers (later marks), no bow
machine gun. No public-domain drawing exists; the ST 7-193 "Tank Identification Handbook"
(US Army Infantry School, 1982 — a US federal work) plausibly carries an outline and is the
candidate for `output/refs/centurion_mk3/`
([core.ac.uk](https://core.ac.uk/download/pdf/188098891.pdf)); the-blueprints.com is commercial and
excluded.
