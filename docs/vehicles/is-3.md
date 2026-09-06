# IS-3

The Soviet heavy of the early Cold War and the archetype of the pike nose. It anchors the
tier-IX heavy role opposite the T-54 mediums: half again their weight, a 122 mm gun that
trades everything horizontal (reload, handling, shell speed) for vertical alpha, and armor that
is GEOMETRY first — the bow and the turret both defeat shells by shape, not by raw millimeters.

## Reference anatomy (blueprint-verified)

- Hull 6.90 m long (the 6.77 this line carried until 2026-09-06 was the IS-2's), 3.15 m over the 650 mm tracks, 2.44 m to the turret roof; 9.85 m overall
  with the gun forward (muzzle at z = 6.46).
- **Pike nose ("shchuchy nos")**: two 110 mm upper bow plates at 56° from vertical, swept ±38°
  in plan, meeting at a central ridge. Head-on, each face presents a compound angle (~64° true);
  yawing the hull toward a shooter FLATTENS the near face toward its bare 56°. The pike inverts
  the angling instinct — this vehicle is strongest square-on.
- **Flattened cast dome**: a wide low "frying pan" casting overhanging its 1.8 m ring — 250 mm
  at the face with 55° of continuous curvature, 45° flanks, sloped even at the rear. The armor
  volumes tessellate it into swept sector planes, so the impact normal follows the casting.
- Running gear: six 550 mm road wheels per side (the IS family's small-wheel look), drive
  sprocket rear, tracks 650 mm wide, and the top run carried on THREE small return rollers per
  side — the heavy's look against the wheel-riding T-54 family.
- **86 OMSh shoes per side**, the real count, which puts the shoe pitch at ~157 mm against the
  historical 162 mm. This is a cost lock as much as an anatomy one: running gear is ~95% of what
  this tank costs to draw and every shoe is its own draw call, so an invented link count is paid
  for in frame time on every IS-3 on the field. It shipped at 104 (130 mm pitch, a fifth too fine)
  and that cost 36 extra draws and 4 800 triangles per tank — 67 200 per 7v7 — buying nothing.
- Low silhouette for a heavy: hull roof at 1.62 m, the stepped-down engine deck and the dome
  carrying the rest — over half a metre SHORTER than every German heavy in the lineup.
- Tail-end signature: two external cylindrical fuel drums lying along the rear fender shelves
  (visual stowage — the armor volumes rightly ignore them).

## Shape locks (the benchmark cage)

The IS-3 was blueprint-born from day one; the finish pass gave it the same per-vehicle test
cage the German fleet carries:

- `is3_pike.rs::the_visible_pike_bow_is_the_armor_pike` — the visible bow plates lie ON the
  armor volume planes (the original honesty lock, the pattern the whole fleet copied).
- `is3_benchmark.rs` — the ±38° plan sweep is symmetric about the ridge; the dome overhangs
  its ring and out-slopes every other dome in the fleet; the D-25T wears its brake and no
  evacuator; three return rollers per side over six 550 mm wheels; the OMSh belt carries its real
  86 shoes at a historical pitch; the rear fuel drums stand proud of the fenders; and the 2.44 m
  heavy stays over 0.4 m lower than every German heavy.
- `is3_hull.rs` — the pike step line runs straight through the tub corner. The fold and both step
  corners are collinear by construction, which is what forces the boundary order of the sponson
  underside; get it wrong and the hull winds against itself (it did, for 2 edges).

## Gameplay shape

- 45.9 t, 382 kW (V-11): 40 km/h on paper, ponderous in the turn (0.58 rad/s) — position early,
  because repositioning is a commitment.
- 122 mm D-25T: 390 alpha on a 12.6 s reload, 175 mm of penetration at 100 m, slow 795 m/s
  shells with real drop. One shell carries a medium's two; every miss costs a medium's whole
  exchange.
- Armor plays by facing the threat squarely and hiding the lower plate: the pike faces and the
  dome bounce what the flat side of a medium never could, while the tub sides behind the tracks
  stay honest 90 mm.

## Modules (stock)

| Slot | Module | Notes |
| --- | --- | --- |
| Gun | 122 mm D-25T | double-baffle muzzle brake, AP 175 mm @ 100 m, 390 HP |
| Engine | V-11 | 382 kW, diesel |
| Suspension | IS-3 running gear | 6 wheels/side, 50 t load limit |
| Turret | IS-3 cast dome | 250/160/110 mm, 0.36 rad/s |
| Radio | 10-RK-26 | 625 m |

## Open items

- **Roof height 2.44 vs 2.39 — 50 mm apart, and no gate can notice.** This dossier states 2.44 m
  to the turret roof (twice, above), but the shipped blueprint bakes `roof_y: 2.39`
  (`game_core/blueprints/is3.blueprint.ron:53`). Nothing measures the discrepancy: the IS-3 pack
  has ZERO `DimensionTarget`s (`vehicle_forge/reference/is3.reference.ron` carries an empty `dimensions` list — the unclosed W2 TODO)
  and the dimension gate skips packs with no dimensions. The benchmark cage measures the 2.49 m
  hitbox apex and the height gap to the German heavies, not the roof plane — its assert message
  even says "2.44 m tank in a 2.49 m box" (`vehicle_recipes/tests/is3_benchmark.rs:143`) while
  the blueprint disagrees. Which number is right is dossier-and-measure work (W2-is3); the
  blueprint stays untouched until it is decided.

- **The running gear's LENGTH is not verified, and the steering mechanism IS.** Opened by P4.6 of
  `docs/contact-and-tracks-program.md`, which needed the IS-3's track-on-ground length before the
  fleet's steering character could rest on geometry. Research (2026-08-06) settled one half and
  refused the other:

  | | finding | state |
  |---|---|---|
  | track width | 650 mm | settled, multiple sources, matches the blueprint |
  | road wheels | six per side, 550 mm diameter | settled, matches `wheel_radius: 0.275` |
  | shoes | 86 per side, ~160 mm pitch | settled, already locked in the benchmark cage |
  | return rollers | three per side, **385 mm diameter** | count matches; the blueprint bakes `roller_radius: 0.11` against a documented 0.19 |
  | overall width | **3.07 / 3.09 / 3.15 / 3.39 m** across sources | **NOT settled — a 32 cm spread** |
  | ground contact length | one figure found, 3.65 m | **NOT settled** — from a table whose own "ширина колеи 3,37 m" is impossible against its own 3,07 m width |
  | **steering mechanism** | **two-stage planetary side mechanisms (ПМП)**, one per track at the ends of the main shaft, multi-disc dry locking clutches and band brakes | **settled**, three independent sources |

  The blueprint bakes a 4.60 m span between the first and last wheel centres — a 920 mm pitch on
  550 mm wheels, leaving a quarter-metre of daylight between neighbours. The IS family's
  recognition feature is wheels that nearly touch, and the one published contact length would put
  the pitch at ~620 mm. That is a reason to doubt the blueprint, not a reason to edit it: the
  number that would replace it comes from a table that contradicts itself.

  **What the steering finding is worth on its own.** A two-stage ПМП has two states per side —
  full speed and a reduced ratio — plus a band brake. It cannot drive a track BACKWARDS. So the
  IS-3's tightest turn is a pivot about a stopped track, never about its own centre: it has no
  neutral steer. That is a documented mechanical fact about the vehicle, and it is the actual
  cause of the ponderous handling, rather than the length-over-gauge ratio that was standing in
  for it.

  **Owed:** a 1:1 running-gear session with drawings, the way the T-54 got one — not another web
  table. Until then no gameplay trait may be derived from this vehicle's contact length.

## Reference anatomy, researched (2026-09-06)

The 1945 production IS-3 (Object 703, ChKZ), NOT the IS-3M modernization (TPK-1 sight, two 200 L
drums, the B-54K-IS engine). Every row carries its sources; the pack (`is3.reference.ron`) now locks
the dimension anchors — the "no `DimensionTarget`s" debt above is closed.

| Dimension | Value | Source | Confidence | Encoded as |
| --- | ---: | --- | --- | --- |
| Hull length | 6.90 m | [ru.wikipedia ИС-3](https://ru.wikipedia.org/wiki/ИС-3) (6900 mm); [victorymuseum.ru](https://victorymuseum.ru/encyclopedia/technic/bronetankovaya-tekhnika/tyazhelyy-tank-is-3-obraztsa-1945-goda-sssr/) — one Soviet-table lineage. **The 6.77 m this dossier carried is the IS-2's 6770 mm** ([army.lv IS-2](http://army.lv/ru/is-2/harakteristiki/631/546)): sibling contamination | medium-high | `HullLength` 6.90 ±0.05 (Locked); `half_len 3.45` (was 3.385) |
| Overall length, gun forward | 9.85 m | ru.wikipedia (9850); [military.wikireading.ru](https://military.wikireading.ru/8302) | high (en.wikipedia's 9.725 rejected: the IS-2's neighbourhood) | `OverallLengthWithGun` 9.85 ±0.05 (Locked); `muzzle_z 6.40` |
| Width over the tracks | 3.15 m | consistent with `outer_x 1.575`; ru.wikipedia's single width field is the 3.39 m gabarit over the fenders (en.wikipedia's 3.07 is the IS-2's) | medium | `HullWidth` 3.15 ±0.05 (Locked) |
| Height to the turret roof | 2.44–2.45 m — the dome's own roof: the IS-3 carries NO cupola, only flush MK-4 periscopes, so roof and silhouette apex coincide | [en.wikipedia IS-3](https://en.wikipedia.org/wiki/IS-3) (2.44); ru.wikipedia (2450); wikireading (2440) | high | `HeightToTurretRoof` 2.45 ±0.07 (Locked; the instrument reads the armour skin's apex, and the commander's cast MK-4 hood stands 6 cm over the roof — the band holds the roof and the hood); `roof_y 2.44` (was 2.39 — the open item above is decided); `hitbox_half_height 1.30`, `hitbox_half_length 3.50` grow with the hull and the hood |
| Ground clearance | 0.46 m | en.wikipedia (460; ru 450; wikireading 435) | medium | `GroundClearance` 0.46 ±0.02 (Locked) |
| Track width | 650 mm | ru.wikipedia; wikireading; victorymuseum; [modelist-konstruktor](https://modelist-konstruktor.com/bronekollekcziya/is-3-poslednij-tank-proryva) | high | `TrackWidth` 0.65 ±0.01 (Locked) |
| Road wheels | 6 × ⌀550 mm a side, unrubberized | the same four sources | high | `RoadWheelDiameter` 0.55 ±0.01 (Locked) |
| Return rollers | 3 × ⌀385 mm a side, rubberized | the same four sources | high | `roller_radius 0.1925` (was 0.11 — the open item above) |
| Turret ring | 1.80 m (the IS family's, Kotin's widened ring) | [parkpatriot.ru](https://parkpatriot.ru/o-parke/tekhnika-parka/tank-is-3/) | high | `TurretRingDiameter` 1.80 ±0.02 (Locked) |
| Pike plates | 110 mm at 56° from vertical (55° in the Zaloga line); the plan "подворот" 43° (one lineage) against the blueprint's ±38° — a convention question (half-angle from the centreline or the angle turned between the plates?), unresolved | ru.wikipedia; en.wikipedia | medium | `glacis_slope_deg 56` ✓; `pike_sweep_deg 38` unchanged until the drawing settles it |
| Lower bow plate 63°; driver's roof plate 73°; rear TWO facets — lower 41°, upper 48° (the blueprint's one 18° facet is a simplification); tub sides 90 mm | ru.wikipedia | medium | not modelled as their own facets yet |
| Turret casting | 220 → 110 mm sides/rear, 255 at the face, slopes −8…35°; plan not found in metres (the blueprint's 2.60 × 2.30 stays unverified) | ru.wikipedia | medium | — |
| Tracks | 86 OMSh shoes a side (79 minimum), pitch 160–162 mm | modelist-konstruktor; wikireading | high | `link_count 86` ✓ |

## Part list (the inventory gate reads this table)

| Class | Fact | Source |
| --- | --- | --- |
| HullTub | The welded tub, sides 90 mm behind the tracks | ru.wikipedia |
| UpperHull | Two 110 mm pike plates at 56° swept 43° meeting at the ridge; the lower plate 63°; the driver's roof plate 73° | ru.wikipedia |
| SternPlate | 60 mm, lower 41°, upper 48° | ru.wikipedia |
| EngineDeck | The stepped-down rear deck over the V-11 | [tankarchives.com](https://www.tankarchives.com/2016/06/is-3-tank-with-piked-nose.html) |
| DeckGrille | The radiator grilles on the stepped deck | tankarchives.com |
| Fenders | Full-length fender shelves both sides | Commons gallery |
| FenderStowage | Four external 90 L drums on the rear shelves (the IS-3M's two 200 L are not this vehicle) | tankarchives.com |
| TurretShell | The flattened cast dome, teardrop in plan, 220 → 110 mm, 255 at the face | ru.wikipedia |
| TurretRing | 1.80 m | parkpatriot.ru |
| Hatches | Two flush roof hatches; the driver's SLIDING hatch in the roof behind the pike apex | ru.wikipedia (the driver's hatch); Commons |
| Periscopes | Three MK-4 periscopes on the roof (the TPK-1 is the IS-3M's); the driver's periscope | ru.wikipedia |
| Mantlet | The cast mantlet at the 255 mm face | ru.wikipedia |
| GunBarrel | 122 mm D-25T, 9.85 m overall | ru.wikipedia |
| MuzzleFurniture | The D-25T's double-baffle brake, no evacuator | ru.wikipedia |
| CoaxMachineGun | 7.62 mm DTM coaxial | ru.wikipedia; en.wikipedia |
| AaMachineGun | 12.7 mm DShK on a ring mount on the roof, −4°…+84° | ru.wikipedia; en.wikipedia |
| Aerial | A 1–4 m rod aerial; the 10-RK-26 in the turret left of the gun | ru.wikipedia |
| Exhaust | The rear exhaust ports off the engine deck | tankarchives.com |
| Headlights | A single headlight on the pike nose | Commons gallery |
| SuspensionHardware | Individual torsion bars | ru.wikipedia |

What the IS-3 does NOT carry (documented absences): a cupola (flush periscopes instead), a bow
machine gun (deleted by design), smoke canisters, side skirts.

The open item above ("2.44 vs 2.39") is decided: 2.44 is the dome's roof (no cupola), and the
blueprint follows it. No public-domain drawing exists (the Commons armour profiles are CC-BY-SA;
the 1955/62 Soviet manual is not PD); PD photographs for camera-matching: the Kubinka pair by
Alf van Beem ([pic1](https://commons.wikimedia.org/wiki/File:IS-3_in_the_Kubinka_Tank_Museum_pic1.JPG)).
