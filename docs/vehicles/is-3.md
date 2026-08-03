# IS-3

The Soviet heavy of the early Cold War era and the archetype of the pike nose. It anchors the
era's heavy role opposite the T-54/T-55 mediums: half again their weight, a 122 mm gun that
trades everything horizontal (reload, handling, shell speed) for vertical alpha, and armor that
is GEOMETRY first — the bow and the turret both defeat shells by shape, not by raw millimeters.

## Reference anatomy (blueprint-verified)

- Hull 6.77 m long, 3.15 m over the 650 mm tracks, 2.44 m to the turret roof; 9.85 m overall
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
  has ZERO `DimensionTarget`s (`vehicle_forge/src/packs_is3.rs:38` carries the unclosed W2 TODO)
  and the dimension gate skips packs with no dimensions. The benchmark cage measures the 2.49 m
  hitbox apex and the height gap to the German heavies, not the roof plane — its assert message
  even says "2.44 m tank in a 2.49 m box" (`vehicle_recipes/tests/is3_benchmark.rs:143`) while
  the blueprint disagrees. Which number is right is dossier-and-measure work (W2-is3); the
  blueprint stays untouched until it is decided.
