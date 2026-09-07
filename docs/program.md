# The Program — one queue for the whole game (2026-09-07)

This is the one document. It merges `docs/inny-poziom-program.md` (the second pass: its
diagnosis, its registers, its 91-row queue) with the audits of 2026-09-06 and 2026-09-07 —
collision, driving, the running gear, destruction, ballistics, terrain, and the owner's
graphics review (`output/design-review`, VR-01…VR-09) — and with every decision the owner
took in those two days. From here on there is one queue, one register per lane, and one rule
for closing a row: **a row closes with a number or a frame, never with a sentence.**

Authority: the owner delegated the open decisions on 2026-09-07 („wszystko, co jest do
podjęcia moją decyzją — podejmuj Ty”). Every decision below is dated, names its owner's words
where they exist, and either cites a chapter of `docs/game-design.md` or adds a dated row to
that document's reconciliation table (rows 25–35 were added with this document). Where the
table and the text disagree, the table wins.

Where the old texts live: `docs/inny-poziom-program.md` (the diagnosis, the closed rows and
the full text of every open row quoted here by id), `docs/art-direction-program.md` (the D
register, closed rows and D1–D33 in full), `docs/contact-and-tracks-program.md` (the tracks
program, its measurements and the rollover arithmetic), `docs/interface-program.md` (closed;
its policy is `docs/interface-policy.md`), `docs/game-modes.md` (the M lane, its own document
by the owner's decision). Those files are history and reference; the queue is here.

## 1. Decisions of 2026-09-06 and 2026-09-07

Each entry: the decision, the owner's words where they were the decision, the reconciliation
row it adds (`GDD row N`), and the lane that carries it.

1. **Collision is honest 2.5D, not full 3D and not Rapier** (the owner, 2026-09-06: „Idziemy w
   2,5D”). Hull and cover become upright boxes with a yaw and a height band; two upright boxes
   overlap iff their XZ footprints overlap (today's SAT) AND their y-intervals overlap — exact
   algebra for upright boxes. A solid no taller than the vehicle's step height enters the
   support envelope like rubble does today; a taller one blocks in plan where the bands
   overlap. Cover becomes an immovable body in the roster solver (the veto path goes).
   Outside the model, deliberately: hull-on-hull stacking, rollover from contact, passing
   under solids, the gun as a body, windows as holes in the collision proxy. GDD row 25. Lane X.
2. **No rollover, full stop** (the owner, 2026-09-07: „bez przewrotek czołgów”). The gate
   `rollover_unreachable.rs` stays; the landing roll excursion is a sway, not a flip. GDD §4
   stands (suspension and sway), reconciled in row 25.
3. **Climbing is skill and positions are never taken away** (the owner, 2026-09-07, on WoT
   removing the Himmelsdorf and El Halluf climbs: „zabierali graczom rozrywkę i wiedzę/skilla”).
   §3.3 „Jeśli fizyka pozwala wjechać na skałę, wjeżdżasz” stands literally; the map report
   must KNOW the climbs (the passable graph with the step height), never remove them. GDD row 26.
4. **P4.7 (terrain throws a track) is rejected** („odpada”). Not proposed again.
5. **Driving comes first** (the owner, 2026-09-07: „ciężko go przewidzieć i trudno jest szybko
   wychamować, stanąć i wycelować w wroga; w World of Tanks jest inaczej”). S brakes; the brake
   is traction-limited (7.2 m/s² on grass, worse in mud and water); releasing W engine-brakes;
   the steer spool is halved; **the dive share of the hull pitch is stabilised out of every
   gun** (the owner's choice, 2026-09-07: „stabilizować udział nurka dla każdego działa jak
   w WoT”) while the Centurion keeps its historical terrain stabiliser; **a gearbox as data**
   (3–4 gears, a flat torque curve, shift points — §4's „krzywa momentu × przełożenie” adopted
   as data, replacing the 2026-08-06 „no gearbox” decision). GDD rows 27 and 28. Lane J.
6. **The running gear is honest before it is dense.** The top run rides its carriers' live
   height, the wheels fit the physics support line with heave subtracted and one symmetric
   clamp, the horn and plate seat derive from the wheel's radii. Zero triangles. Lane G (G1, K10, J7).
7. **Buildings before their collision** (the owner, 2026-09-07). The building kit (B1–B6)
   defines the box data (yaw, height, segments); the sim consumes it in lane X afterwards.
8. **Destruction falls, it does not swap** (the owner, 2026-09-07: „bez sensu … drzewo nie
   upada tylko jakby znika i pojawiają się jakieś boxy. Tak samo jest z budynkami”). Trees
   topple with a direction byte, wall segments fall as rigid pieces inside the dust, a ruin
   keeps two walls and a floor. **Destruction classes are a data axis separate from the kind**
   (the owner's taxonomy: fences, boxes, signs, trees, walls, houses, terrain, mountains, big
   buildings): Immovable / Stateful-per-segment / Breach / Topple / Crush / Prop / Terrain.
   Havok is the lesson, not the plan (Wargaming announced full building physics in 2014 and
   shipped 9.14's driving physics in 2016; buildings stayed authored states under dust). GDD row 29.
   Lane Z.
9. **Destroyed terrain is „beznadziejny”** (the owner, 2026-09-07). Craters are the one honest
   mechanic (ledger, 0.625 m patch geometry); everything else is a decal that fades. Lane T
   (T7–T9): scorch as a splat with material, spoil and clods, ruts with memory in soft ground,
   furrows as displacement in the patch, and the per-HE whole-mesh re-upload (Q1) gone.
10. **No camouflage stat; bushes hide** (the owner, 2026-09-07: „kamuflażów ja nie chcę,
    dokument projektowy kłamie. Oczywiście czołgi muszą mieć swój zasięg widzenia, wykrywanie
    itd, także zwiadowca ma swoją rolę”; and: „krzaki mają chować”). V2 and the concealment
    stat of R1 are out; V1 (optical density of foliage on the sight line) is in; view range per
    vehicle from the dossier stays; scenery still never blocks a shell. GDD row 30. Lane V.
11. **Ballistics** (the audit of 2026-09-07, 7/10): air drag STAYS and the design text is
    reconciled (GDD row 31); HE direct-hit damage becomes a function of plate thickness at the
    point (§3.1 implemented); the dispersion seed gets a per-battle salt before the first public
    match; the casemate gets a limited traverse arc; the aim time is documented as three
    e-folds and the bloom as additive (GDD row 32); `r·u²` stays (row 5); the glance-band loss
    is shown in the reticle; splash is occluded by wrecks; the non-penetrating module lump goes
    through the wound scale. Lane S.
12. **Ramming is billed from the peak closing speed per contact, and both hulls pay, weighted
    by mass** (GDD row 33 amends §4's „boli tylko średniego”: a free weapon for heavies is a
    bot-spam vector at 15v15). Lane X.
13. **Wrecks can be pushed** (one flag and a parking friction); **cover is a body** (no
    momentum is deleted at a wall). Lane X.
14. **Windows are not apertures for sight** (GDD row 34 amends §13.4): a shell pays the wall's
    thickness (a window is thin wood in the same table), the sight line is blocked. Shooting
    through windows at 20 Hz snapshots is „trafił mnie przez ścianę” for the victim.
15. **The terrain grid goes to 2.5 m map-wide, with chunk LOD and geomorphing, gated by the
    map-swap and frame measurements T1 asks for.** Tank-scale relief stays analytic through
    `sample_height` (craters today; ruts and authored micro-strokes next), never a denser
    raster. 1 m map-wide and the GDD's 0.5 m are refused for the same bake reason. Lane T (T1).
16. **Maps are gameplay topology first** (the owner, 2026-09-07: „lanes, crossfire, cover,
    hull-down, flanking, sniper positions, rotation paths, fallback positions”). The roles
    become data (append-only) and the map report computes them from geometry. GDD row 35. Lane W.
17. **The picture**, from the graphics review verified in code (2026-09-07): the low sun owns
    the field (profile data), shadow strength per look, cloud shade with depth, a second
    canonical view INTO the sun; leaf sprites re-baked into the world's albedo window; the
    vehicle material chain becomes a real response per role with mips, a macro octave, edge
    wear and a dirt band (K6 — K24's colour is done, the materials are not); the ground gets
    form (dirt darker than grass, plots with edges, clumps with occupancy, the hedge body as
    foliage); the garage hero pivots 1.0 m forward. **Every one of these PRs rewrites the lying
    lock in the same PR** (O2). Lane D (D34–D45).
18. **Interface rows route to the interface session** (`docs/interface-policy.md` ownership):
    labels leave the stencil for Plex SemiBold 18 u (a dated policy row replaces
    `interface-policy.md:92`); texture never competes with text (enamel under any plate that
    carries text ≤ 12 u); the reticle numbers get their 3:1 (already the policy's promise);
    the hit log gets a compact default; the range prints once; the zoom label is burned.
    No HUD or garage overlay file is touched from any other lane. Lane U (U12–U17).
19. **The representative fragment** (the review's own next step): Kamienna on Bystra, the
    T-54 (the benchmark is unchanged — the owner withdrew the Tiger I proposal), ~10 s: out
    from behind the parapet, hull-down on the bridgehead bluff, aim into the market lane at a
    Tiger, shot, wall breach, drive through the breach. The one sequence that passes through
    every lane above. It is the acceptance frame of blocks 1–4, not a block of its own.
20. **The one thing only the owner can do:** N9 needs `wsl --install -d Ubuntu` on the owner's
    machine. Everything else in this document is delegated.
21. Reaffirmed: no physics engine (rapier's `enhanced-determinism` is same-binary only);
    `parry3d` and `legacy_boxes.rs` are deleted (zero callers, a dead kernel with a real bug);
    weather is presentation only (row 9); the scope is rigid (row 6); zero aim assist.

## 2. The queue

Throughput assumption unchanged from the second pass: the gate on one laptop is the ceiling
(10–15 min per PR gate, 25–30 min the full one), not the writing of code. Estimates are
continuous sessions. Blocks are ordered by what the player sees in the first minute and by
dependency; a block is done when every row in it is closed.

| Order | Block | Rows | Estimate |
|---|---|---|---|
| 0 | **Housekeeping, any time, between gates** | X9 (`parry3d` out), S22 (`legacy_boxes` out), D17 close, S21 (bible tint = code), S19/S20 (reticle Clear-on-ground, detached turret in `trace_sets`), Z4, S6, A2 | 0.5 day |
| 1 | **Jazda** | J1 → J2 → J3 → J4 → J5 → J6, then G1 → J7 (the running gear), K10; each PR with the stop-and-aim lock and the owner at the wheel | 2–3 days |
| 2 | **Światło i materiały, zero triangles** | D34 → D35 → D36 → D37 (light); D38 (leaf sprites); D39 → D40 → D41 → D42 (vehicle material = K6's first half); D43 → D44 (ground form); D45 (garage pivot, turntable, crew strip); O2 rewritten with each | 2–3 days |
| 3 | **Budynki i destrukcja (Destruction 2.0)** | B3 → B1 → B2 → B4 → B5 → B6 → B7; Z8 → Z9 → Z10 → Z11 → Z12 → Z13; T7 → T8 → T9; V0; Z7; Q1 | 7–10 days |
| 4 | **Kolizje 2,5D** | X1 → X2 (one district of Kamienna at an angle) → X3 → X4 → X5 → X6 → X7 → X8; X10, X11 | 3–4 days |
| 5 | **Balistyka** | S14 → S15 → S16 → S17 → S18; S23 | 2 days |
| 6 | **Widoczność** | V1 (bushes hide) → V3 → V4; R1 as amended | 2 days |
| 7 | **Teren** | T1 (2.5 m + LOD + geomorph, measured) → T2 → T4 → T5; C4 after T1 | 4–6 days |
| 8 | **Determinizm i klatka** | N9 (after the owner's WSL), Q7, Q2, Q4 | 1.5–2 days |
| 9 | **Kamera** | C1 → C2 → C3 (after G7b) → C4 | 2 days |
| 10 | **Mapy jako topologia** | W1 → W2 → W3 → W4 | 3 days |
| 11 | **Forge 2.0 (W3 of the second pass)** | K0 block → K3 → K9, K11–K23 → K4/K5 → K6's second half → P1–P3 | 10–15 days |
| 12 | **Interface session** | U12 → U13 → U14 → U15 → U16 → U17, then U1–U11 as that program orders them | its own program |
| 13 | **The rest** | G2–G6, G7b, R2, R3, H1–H6, N1–N8, N11, F8–F11, O1, L1–L3, M5b/M7b/M8 (`docs/game-modes.md`) | as the second pass estimated |

Dependencies that decide the order: G1 before any support-from-solids (X4) and before J7 —
otherwise a fourth ground sampler; G7b makes the step-over (X4) readable; B-rows before X1
(the kit defines the box data); V0 before V1; T1 before C4 and before T2/T4 (cliffs make the
height band X3 matter); N9 before the first public match (with S16); the representative
fragment (decision 19) is rendered after block 4 and judged by the owner playing it.

Machine facts that no agent changes: the cold measurement needs the box idle and rested to
≤ 60 °C; two worktrees on one target dir hand each other's binaries around; HUD and garage
goldens belong to the interface session; a blueprint/armour change gates with `-Crates
...,client,sim`; a wire enum append bumps `PROTOCOL_VERSION`; every replay a sim number moves
is re-pinned in the same PR and says why.

## 3. Registers

New rows carry their full text. Rows inherited from the second pass keep their id and one
line; their full text, evidence and "closes when" stay in `docs/inny-poziom-program.md`
(or the register named). Each new row: defect · evidence · closes when.

### X — collision (2.5D)

| ID | Defect | Evidence | Closes when |
|---|---|---|---|
| X1 | **Every solid is an axis-aligned box; towns are grids.** `StaticCoverObject` has no yaw; the SAT gets yaw 0; `TownGrid` is orthogonal; the only yawed things in the blueprints are trees and lampposts | `terrain/src/battlefield.rs:270-276`, `physics/src/cover.rs:105-114`; Ostrogorsk 48-block grid, Kamienna 4×3 | `yaw_rad` on the box with `serde(default)`; ONE `CoverBox` type with `corners()/contains()/segment()` consumed by every reader (SAT, shell slab, sight slab, bake, camera obstacles, minimap, predict, bots, editor, atlas — fifteen consumers today); lock: the yaw = 0 path is bit-identical on every replay |
| X2 | **One district of Kamienna at an angle** — content that proves X1 | `bystra-valley.map.ron` town grid | a yawed block in the market lane, its goldens blessed, the district on the fragment's route |
| X3 | **Height does not exist in contact.** The SAT never reads `center.y`; a hull on a rubble mound collides with one below as if level; a 1.1 m parapet blocks like a tenement | `physics/src/collision.rs:188-189`; `docs/vehicle-movement-policy.md` "blocks in plan at any height" | XZ SAT ∧ y-interval for hull–hull and hull–cover, the band from the support height + `HitboxProfile` height; lock: a hull 3 m above a 1 m wall passes; a hull on a mound does not shove the hull below |
| X4 | **Nothing is climbable but terrain and rubble.** P2.2 measured out because no content under 0.8 m exists; the shortest cover is the 1.10 m bridge parapet, 12 objects under 2 m on five maps | `docs/contact-and-tracks-program.md` P2.2; the blueprint census | a low tier of `StaticCoverKind` (append-only, wire) in the SAME PR as the physics: a solid whose top is within the step height of the current support enters the support envelope; the step height = `TrackShape.top_y` minus clearance, the dossier's vertical obstacle as the acceptance number (T-54 ~0.8 m); lock: the parapet is crossed with a tilt and a speed loss, the churchyard wall (1.6 m) is not |
| X5 | **Rocks are walls or ghosts.** 8 `Crag` boxes (Mazurski 2, Orliny 6) block to the sky; 9 scenery `Rock`s up to 2.1 m have no collision; Ostrogorsk and Prokhorovka have no rock at all | `map_forge/src/compile.rs`, `clutter.rs:273-275` | `Crag` in the X4 model (its top a support when within the step, else a wall); scenery `Rock` above the belly line gets a low-tier box or is cut to 0.40 m; lock: every shipped Rock's top − ground ≤ `BELLY_LINE_M` or boxed |
| X6 | **Cover is a veto, not a body.** A wall deletes momentum (the sim's own invariant says nothing may), gives no dive (the accel is read before the wall resolves), no torque, no damage; a hull pinned between a pusher and a wall loses its velocity every tick | `physics/src/cover.rs:15-34`, `world.rs:197-227`, `collision.rs:93-113` | an immovable `ContactBody` per nearby box in the roster solve; the veto path deleted; lock: a 14 m/s wall hit dives the nose and bills through `ContactPair`; the pinned hull holds |
| X7 | **The ram bill is a hidden die.** Billed from the solver's per-tick impulse while the speculative contact closes the remaining gap in the touch tick: ~268 HP or ~55 HP for the same charge by 8 cm of spawn distance | `sim/src/ramming.rs:46`, `physics/src/contact_impulse.rs:427-433` | bill from the peak closing speed per `ContactFeature`, once at separation; both hulls pay weighted by mass; lock: a 1 cm sweep of spawn distance 30.00…30.23 m gives ≤ 5 % spread |
| X8 | **H2: a hull sinks 0.44 m into a parked neighbour.** One contact point per pair; the two-point manifold was built (0.43 → 0.039 m) and shelved because it destabilised queues; the contact point is the midpoint of the centres so a charger slews as much as its victim | `sim/tests/steering_into_a_neighbour.rs:35-56`, `contact_impulse.rs:406-412` | the SAT's `ContactFeature` (face + corner) as the contact point; then the two-point manifold with the correction shared across a hull's contacts; the H2 tolerance lock retired |
| ~~X9~~ | ~~**`parry3d` compiles for a function with zero callers**~~ **CLOSED (2026-09-07, `chore/x9-parry-out`).** `parry_query.rs` and the dependency deleted; `no_physics_engine_rules.rs` keeps rapier AND parry out of the manifest; `math_backend_parity` no longer lists a crate that is not there | `physics/src/lib.rs`, root `Cargo.toml` | done |
| X10 | **Trees are ghosts but the oak.** Only `SceneryKind::Oak` earns a `TreeTrunk` box; poplars on Bystra, Mazurski, Orliny and fruit trees have 20 m boles a hull drives through; the oak's box is two literals from a generator that no longer ships | `map_forge/src/compile.rs:309-341` | trunk boxes for every species from the authored asset's bole metrics; lock: chest-height bole vertices inside the box ± 5 cm, first limb above 0.9 of the box |
| X11 | **Rubble has three shapes.** Shell/sight: a lowered box with vertical faces; hull: a 38° pyramid; picture: a slab at 0.55 of the crest with chunks | `sim/src/cover_damage.rs:128-137`, `terrain/src/rubble.rs:81-89`, `scene_build/src/battlefield.rs:451-510` | the pyramid for shell and sight too; the talus drawn; lock: for a grid of points on the talus, shell blocked ⇔ `RubbleMound::height_at` |
| X12 | **Wrecks are infinite mass**; a dead ally in a chokepoint is a wall forever | `sim/src/state.rs:709` (`movable: hit_points > 0`) | wrecks movable with parking friction; lock: a 36 t hull pushes a 36 t wreck at ≤ 1 m/s |

### J — driving (jazda)

| ID | Defect | Evidence | Closes when |
|---|---|---|---|
| ~~J1~~ | ~~**S is not the brake.**~~ **CLOSED (2026-09-07, `feat/j1-j2-s-brakes-traction`).** An opposing throttle on a rolling hull goes through the brake channel; the speed cap follows the direction of motion while momentum is opposed (no more overspeed governor toward 4.2 m/s); locks: S from 50 km/h stands in < 1.9 s inside 13 m and never snaps across zero; S and Ctrl brake identically | `physics/src/forces.rs`, `tests/suite/movement_model.rs` | done |
| ~~J2~~ | ~~**The brake is a constant, not traction.**~~ **CLOSED (2026-09-07, with J1).** `brake_deceleration_mps2` 4.8 → 7.2 = the grass grip cap, and the demand is `min(brake, mu·g·traction·cosθ)`; the mobility table's `brake_m` column re-recorded (T-54 grass 16.9 → 11.8 m, Tiger II 9.9 → 6.9 m); lock: quarter traction brakes at ≤ 1.8 m/s²; the bot planner's `braking_deceleration_mps2()` is truthful by construction | `controller_settings.rs:209`, `mobility_baseline.rs` | done |
| ~~J3~~ | ~~**Releasing W coasts 45 m.**~~ **CLOSED (2026-09-07, `feat/j3-j5-engine-brake-steer-release`).** `idle_drag_mps2` 1.3 → 2.6 and engine braking applies only when the driver commands nothing (a steer with no throttle is a driven state, so a braked-belt pivot keeps its walk); coast lock band 25–90 → 15–35 m (T-54 ~22 m) | `controller_settings.rs`, `forces.rs`, `movement_model.rs` | done |
| J4 | **The gun rides the dive.** Weight transfer pitches the authoritative hull 3.9° on a brake; `vertical_stabilizer` is 1.0 on the Centurion and 0.0 elsewhere; the client re-lays the gun one tick late → the gun marker and the impact X leave the ring for ~0.8 s on every stop and launch; a second presentation spring nods the drawn barrel a further ~1.1 s | `physics/src/hull_attitude.rs:28`, `sim/src/aiming.rs:55-58`, `engine/src/attitude.rs:12-14` | the hull spring split into two superposed states (terrain share + transfer share); every gun compensates the transfer share (factor 1.0), the Centurion additionally the terrain share; the presentation spring becomes a pass-through so the drawn barrel is the shooting barrel; `brake_dip.rs` rewritten (the dive moves the hull, the aim point holds); lock: after a full-speed stop the X never leaves the 2.9 mrad ring |
| ~~J5~~ | ~~**The heading overshoots 6.7° on every steer release.**~~ **CLOSED (2026-09-07, with J3).** Spool clamp (0.25..0.7) → (0.12..0.35) s and the release ramp runs at twice the spool-up rate (both belts brake against the ground); lock: full lock at 8 m/s released → overshoot ≤ 2° and the rotation stops; the pivot table and `mobility_baseline` unchanged within tolerance | `controller_settings.rs:198`, `movement.rs` | done |
| J6 | **The engine is one number and the launch is a jolt.** P/v with a 2.2 m/s floor: 7.2 m/s² for 0.35 s then a 1/v grind (0→25 km/h 1.8 s, 25→40 3.8 s, 40→47.5 6.4 s); GDD §4 wants a torque curve × gear ratio with an automatic box | `forces.rs:97-103`, `controller_settings.rs:163, 189-190`; `docs/contact-and-tracks-program.md` "no gearbox" (superseded) | a gearbox as DATA per vehicle (3–4 ratios, a flat torque curve, shift points), deterministic; the engine voice reads the shifts; `mobility_baseline.rs` re-recorded as a set; lock: 0→80 % in the same time as today ± 10 %, no single-tick jolt above 4 m/s² |
| J7 | **The tracks go into the wheels.** The top run is rigid (`dy = 0` for `sample.y ≥ end_cy`) while wheels travel to +0.20 m → the tyre leaves the belt at 5 cm of travel on 6 of 8 vehicles; the wheels are fitted to `hull_y + tan(presented pitch)·z` (tick-snapped height, spring-lagged pitch), heave ignored, rubble ignored; the OMSh horn sits 24 mm in the steel at rest, the Kgs horn 56 mm; the plate 10 mm inside the tyre by design; `running_gear_dynamics.rs:110` locks the defect | `vehicle_geometry/src/running_gear_place.rs:109, 272`, `running_gear_belt.rs:21, 153-157`, `client/src/vehicle/render_frame.rs:283-305` | the top run on its carriers' live height; wheels fitted to `sample_support` with heave subtracted and ONE symmetric clamp shared with physics; horn and plate seat from the wheel's radii; the wheel at the arm's rotated tip; the `− 0.075` test replaced by a clearance lock over a driven crest and ditch (every link ≥ R − 3 mm from every wheel centre, the top run ≤ SEAT + 10 mm above the crest, the wheel bottom within 10 mm of the ground unless the clamp saturates). Zero triangles (K10 closes with it) |

### G — tracks and the ground (inherited)

G1 (three ground samplers — ONE function owns contact; before X4 and J7), G2 (belt speed on the wire), G3 (the FX gauge), G4 (one `TrackCondition`), G5 (per-belt ground; P2.2 closes as X4), G6 (fifteen track models; `parry_query` deleted = X9), G7b (heave on the spring). Full text: `docs/inny-poziom-program.md`.

### Z — destruction (Honest Steel)

Inherited: Z4 (the register replaces the roadmap's DONE), Z7 (shells ricochet off the ground).

| ID | Defect | Evidence | Closes when |
|---|---|---|---|
| Z8 | **A tree vanishes and a log-box appears.** The instance is filtered out by phase; the bake puts a stump and a lying log prism in its place; the collapse dust is the tenement's | `scene_build/src/battlefield.rs:340-372`, `tree_lod.rs:465`, `client/src/fx/collapse.rs` (kind-blind) | a Topple class: a fall-direction byte (from the crusher's heading or the shell), a client choreography that lays the authored tree down over ~1.5 s, then the stump-and-log state; foliage-family dust, not masonry; lock: the felled tree's crown ends within 1 m of the heading, the state after the fall equals the bake |
| Z9 | **A wall breach is a state swap under dust.** Four states per building exist as one phase byte for the whole box (0/1/2); §13.4 wants destruction PER WALL SEGMENT with four states and the material deciding | `sim/src/cover_damage.rs`, `terrain/src/battlefield.rs` phases | segments as data on the building kit (B3), a phase byte per segment (append-only on the wire), the wall's material as thickness in the armour table; lock: a 57 mm and a 152 mm fell the same barn segment in different counts, a stone church segment only to a large HE |
| Z10 | **A collapse is a swap: the walls vanish inside the dust and a slab with 4–6 chunks stands there.** The theatre (Z1) sized the dust; the masonry never moves | `client/src/fx/collapse.rs`, `battlefield.rs:451-510` | wall pieces as animated rigid bodies for ~1 s inside the dust (client choreography, deterministic from the seed), ending in B5's ruin (≥ 2 wall planes and a floor) and the talus (X11); lock: the choreography's final state equals the ruin bake byte for byte |
| Z11 | **Destruction is one axis.** `StaticCoverKind` decides look, HP, rubble fraction and crushability at once; scenery never dies; props with mass do not exist (§4 „bryły z masą”) | `terrain/src/battlefield.rs:66-100`, `scenery.rs` | a `DestructionClass` per kind as data: Immovable (Crag, RailCover, mountains), Stateful (FarmBuilding, CityBuilding, StoneTower: per segment), Breach (StoneWall), Topple (trees, lampposts, signs), Crush (WoodenFence, Bush), Prop (crates, carts: a knock-over state with a direction, no dynamics on the wire), Terrain (craters); lock: every kind names a class; the class decides the choreography and the phases |
| Z12 | **Ramming a wall does nothing to the wall.** §13.4 „drewno pada od taranu, cegła od kilku HE, kamień tylko od dużego HE”; today only fences and hedges crush at ≥ 2.5 m/s | `sim/src/state.rs:388-410` | crush thresholds per class and mass (a heavy at speed fells a stone garden wall with self-damage); lock: the T-54 at 8 m/s breaches a `StoneWall` and pays; at 2 m/s it stops |
| Z13 | **The detached turret is a picture.** §12 „wieża ląduje jako prop”; the wire carries `detached_turrets`; nothing blocks on it | `net/src/lib.rs:501-506`; no `physics`/`sim` reader | the landed turret as a low solid (X4's model): blocks a shell, is crossed with a tilt; lock: a shell into a landed turret stops |

### T — terrain

Inherited: T1 (the grid — DECIDED: 2.5 m map-wide with chunk LOD and geomorphing, gated by the map-swap and frame measurements; the far chunks morph to 5 m/10 m, the morph is zero inside ~150–200 m so eye = ground where shots land), T2 (erosion in the editor), T4 (cliffs triplanar), T5 (road edges and kerbs — extended to the city kerb/pavement and a setts tile: the paved street routes to the rock lane and wears the cliff's crack tile).

| ID | Defect | Evidence | Closes when |
|---|---|---|---|
| T7 | **Destroyed terrain is „beznadziejny”** (the owner, 2026-09-07). The crater is real (ledger, a 0.625 m patch cut into the 5 m mesh) but it reads as a smooth bowl with a scorch STAMP; no spoil, no clods, no torn turf, no rim material | `scene_build/src/battlefield.rs:1540-1760`, `client/src/fx/terrain_scars.rs`, `terrain/src/craters.rs` | the scorch as a splat with its own material (Z5's owed half), the rim as spoil (a dirt splat ring + instanced clods from the clutter path), torn turf at the lip; a `destruction_showcase` frame looked at before merge; lock: the crater's splat ring ≥ 0.5 of its radius, clods ≥ 6 per crater above 1.5 m radius |
| T8 | **Ruts and furrows are stickers.** `rut_depth_m` exists per material and only the mark's opacity reads it; an AP furrow is a decal; a column of tanks leaves a road that fades | `terrain/src/ground.rs:107-114`, `client/src/app/motion_fx.rs` | ruts with memory in soft ground: client-side persistent displacement in the crater patch path for Dirt/wet Mud (no gameplay, no wire), capped per battle; furrows as displacement in the same patch; lock: a T-54 column of five leaves a rut ≥ 5 cm in dirt that outlives the fade |
| T9 | **Every HE re-uploads the whole ground mesh** (Q1's terrain half): a hitch per hit, a fresh GPU allocation never scaled to the dirty span | `renderer_wgpu/src/scene_renderer/ground.rs:436-455` | the crater patch as its own chunk buffer (append, never re-upload the base); lock: an HE on terrain allocates ≤ the patch, frame delta recorded |
| T10 | **Steep faces exist only where a diagonal run-up reaches them.** The Prokhorovka embankment (grade 0.78) and Ostrogorsk's (0.71) are the only places to fall from; nothing measures or locks the climb envelope on the shipped maps; bots never climb | `blueprints/*.map.ron`, `physics/tests/suite/climb_envelope.rs` (synthetic face only) | the map report lists every face between 0.68 and 1.36 grade as a CLIMB (position, height, approach angle) so the author sees them (decision 3: never removed); lock: the count per map is a golden the author blesses |

### S — the shot (ballistics and hit)

Inherited: S2 (sniper feel), S6 (roadmap wording).

| ID | Defect | Evidence | Closes when |
|---|---|---|---|
| S14 | **Design and code disagree on air drag with no row.** GDD §4/§6 „opór powietrza nie”, „spadek penetracji tabelą”; code: linear drag `c = 0.0130/SD` (0.07–0.12/s AP, 0.17–0.24 tungsten, 0.05 HEAT/HE), pen = `pen100·(v(d)/v(100))^1.5`. DECIDED: the code stands (GDD row 31) | `game_core/src/weapon.rs:255-302`, `docs/combat-policy.md:41-52` | a numeric flight lock: D-10T BR-412 at 1000 m drop 6.64 ± 0.1 m, TOF 1.173 ± 0.01 s, `speed_mps_at_distance(1000)` within 2 m/s of the integrated speed |
| S15 | **HE direct non-penetration is a flat 18 % of alpha whatever the plate.** A T-34-85 side (45 mm) and a Tiger II glacis (150 mm) both take 77 HP from an OF-412; §3.1 „obrażenia w funkcji grubości w punkcie” | `game_core/src/armor/resolve.rs:300-308`; the bystander law already in `sim/src/shell_splash.rs:78-80` | direct target: `max(0, 0.5·alpha − 1.3·LOS at the point)`; lock: OF-412 on a 45 mm side > on a 150 mm glacis, 0 from 166 mm |
| S16 | **The dispersion seed is predictable** (tick ^ id ^ shot → splitmix64): a modified client clicks on a lucky tick | `sim/src/aim_dispersion.rs:94-101` | a per-battle salt from the server in the replay header; lock: same (tick, id, shot) under two salts → different draws, replays bit-exact. Before the first public match (with N9) |
| S17 | **A casemate cannot traverse its gun.** `TurretTraverse` is `Rotating | Fixed`; the Jagdtiger (±10° historically) lays by hull pivot, which blooms the sight through the steer term | `game_core/src/modules/turret.rs:6-9`, `tank.rs:166-172`, `sim/src/aiming.rs:38-41` | `TurretTraverse::Limited { half_arc_rad, rate_rad_s }`; the firing solution reports `Traverse` only beyond the arc; lock: the Jagdtiger lays 9° without hull motion and refuses 11°; `fixed_casemate_ignores_turret_yaw_commands` replaced |
| S18 | **The non-penetrating path lumps raw module damage.** `requires_penetration: false` capsules take `base_damage_hp` raw: 280 HP of OF-412 on a 150-HP suspension in one lump, bypassing the 0.45 wound scale | `sim/src/combat.rs:446-453`, `damage_layout/authoring.rs:449` | through the wound scale; lock: one HE slap wounds, never destroys, a healthy suspension |
| ~~S19~~ | ~~**The reticle says Clear when the server buries the shell at the muzzle.**~~ **CLOSED (2026-09-07, `fix/s19-buried-muzzle-terrain`).** `trace_shell` returns `Obstacle { Terrain }` from the ground-contact branch, as `shell_step` does; lock: a muzzle inside a 3 m step → the server's first-step `Terrain` impact and the reticle trace name the same surface | `sim/src/shell_trace/mod.rs`, `sim/tests/suite/shell_trace.rs` | done |
| S20 | **The reticle collides with a turret that is not there.** The server sets `turret_detached`; the client's `trace_sets` never does, though the wire carries `detached_turrets` | `sim/src/shell_step.rs:232`, `shell_trace/types.rs:67`, `client/src/hud/reticle_sweep.rs:66-72` | thread `detached_turrets` into `trace_sets`; lock: a headless wreck, shell lane at turret height — both traces pass |
| S21 | **Two eyes.** Spotting and the bot trigger use `line_of_sight` (radius 0, 2 m march without interpolation, +0.3 m slack, straight chord, no water, no wrecks); the shell uses `segment_impact` (1 m march, interpolation, calibre radius, arc). On a ±30° crest the eye sees ~0.9 m through what kills the shell; the Bystra hull-down contract is signed with the eye. (= V0, carried here for the shot's half) | `sim/src/spotting.rs:132-169, 221-241`, `shell_trace/terrain.rs:4-33`, `sim/tests/suite/bystra_hull_down.rs:57` | one march (cell-edge crossings, exact on the piecewise-linear surface) for eye and shell; the bot fires only when the SHELL trace is clear; the hull-down contracts re-locked with both kernels |
| ~~S22~~ | ~~**A dead kernel with a real bug in the hot file.**~~ **CLOSED (2026-09-07, `chore/s22-legacy-boxes-out`).** `legacy_boxes.rs` and the `armor_volumes: None` branch deleted; `TraceTank.armor_volumes` is non-optional and `armor_coverage.rs` is the lock that every playable kind owns volumes | `sim/src/shell_trace/{tank,types}.rs` | done |
| S23 | **The glance band is invisible and the aim time is undocumented.** A 65° shot is 15 % weaker than WoT's and the reticle shows nominal pen; aim time = three e-folds (a „2.5 s” is WoT's 0.8 s), bloom additive (GDD §6 says multipliers) | `armor/resolve.rs:19-26, 61-73`, `aim_dispersion.rs:19-45` | GDD row 32 (decided: keep both, on the record); the effective pen after bite in the reticle hint; locks on the steady states (T-54 at full speed 7.07 mrad, 95 % settled at aim time); splash occluded by wrecks (`shell_splash.rs:57-77`) with a lock |

### V — visibility as a resource (as amended 2026-09-07)

| ID | Defect | Evidence | Closes when |
|---|---|---|---|
| V0 | the eye and the shell march the terrain differently (= S21) | | one march; the parity lock on every map |
| V1 | **A bush hides nothing and a tree line hides everything** — DECIDED IN (the owner: „krzaki mają chować”) | `sim` spotting, `terrain::scenery` | foliage kinds carry an optical density per metre; the spotting ray integrates density along its length (the shell's own march); a target is spotted when the integral stays under the observer's budget; scenery still never blocks a shell; locks on a bush at 5 m vs 50 m and on shell/LOS parity |
| ~~V2~~ | ~~camouflage per vehicle~~ — **OUT** (the owner, 2026-09-07; GDD row 30) | | — |
| V3 | the budget is invisible | HUD | a HUD readout of "seen from N m" that reads view range and density (no camo term) — routed to the interface session |
| V4 | no sixth sense; auto-spot at 50 m unverified | `sim` | sixth sense for everyone from the first battle; the 50 m lock |
| R1 (amended) | ~~no concealment stat~~ — the concealment half is OUT; the view-range half stays | `tank.rs:132` | an authored view range per vehicle from the dossier; lock: the scout sees farther, stationary and moving alike |

### W — the world as gameplay topology (new lane)

| ID | Defect | Evidence | Closes when |
|---|---|---|---|
| W1 | **Lanes, crossfire, sniper perches, rotation paths and fallback positions exist only as prose.** Roles today: Observation 16, HullDown 14, Crossing 14, FlankRoute 13, HighGround 11; the report checks the passable graph, the hull-down floor, spawns, symmetry, the destructible floor; exposure lives only in the atlas tool | `map_forge/src/report.rs:136, 1685`, `tools/src/atlas.rs:592-681`, the blueprints' `role:` census | roles appended: `Lane` (polyline), `Crossfire` (a pair), `SniperPerch`, `RotationPath` (polyline), `Fallback`; the report computes each from geometry (a crossfire = two points with LOS to the same lane segment from > 60° apart; a rotation path's masking share from the exposure field; a fallback's distance behind its lane); lock: every shipped map declares every role class and the report's numbers are goldens |
| W2 | Cover density along lanes is unmeasured | `report.rs` | a per-lane cover census (boxes per 100 m, hull-down spots per lane) in the report and the dossier |
| W3 | Bots do not read the topology; they steer straight with an unstuck routine and never path around cover | `battle_host/src/bots.rs:18-30` | bots route on the lane/rotation graph and use cover boxes as waypoints; every X and Z feature the bots cannot use is a human-only advantage in the AI battle |
| W4 | The representative fragment (decision 19) has no acceptance frame | — | Kamienna's route authored (parapet → bluff → market lane), the ~10 s sequence recorded from the player camera with the real HUD, the owner's five questions answered before the next block |

### B — buildings

Inherited: B1 (eaves and ridges), B2 (ground connection), B3 (kit of parts, instanced), B4 (coursing and streaks), B5 (a ruin with form), B6 (variety: L-shapes, annexes, terraces). B3 first. Plus:

| ID | Defect | Evidence | Closes when |
|---|---|---|---|
| B7 | **No kerb, no pavement, no setts.** The city street routes to the rock lane and wears the cliff's crack tile; the road dissolves into grass without an edge (VR-04) | `terrain/src/ground.rs:217-222`, `renderer_api/src/ground_detail.rs:68-72`; `ostrogorsk_canyon.png` | T5 scoped to the city: kerb + pavement strip on the instanced path, a setts detail tile; lock: `a_paved_road_wears_setts_not_cliff_cracks` |

### D — the picture (the register's home moved here from `docs/art-direction-program.md`)

Inherited and open: D4 (no dark mass — CLOSES WITH D34–D37), D8 (no edge wear), D9 (dirt lane never populated), D15 (nothing to look at up close beyond the T-54), D18 (Orliny borrows Bystra's light), D33 (the hall brightened; the turntable 1.9× the floor after K24-1). D17 CLOSES (the lineup paints since K24). Full text: `docs/art-direction-program.md`.

| ID | Defect | Evidence | Closes when |
|---|---|---|---|
| D34 | **The low sun does not own the field.** Golden evening: elevation 14.9° → the key delivers 44.5 % of the flat field's light, the blue indirect 55.5 %; light on the field has R/B 1.08 against the key's 2.4; cast shadows 1.8:1. Measured on the review frame: ground p5/p50/p95 = 0.18/0.22/0.27, 0 % of pixels below 0.07 | `renderer_api/src/lighting.rs:461-469`, `shaders/lighting_common.wgsl:50-59`; `prokhorovka_golden_evening.png` | profile data only: at ≤ 20° the indirect drops and warms (ambient ≈ (0.09, 0.10, 0.15), fill ≤ 0.08) or the key rises so the flat-ground key share ≥ 0.6 and incident R/B ≥ 1.5; the CPU lock `the_low_sun_owns_the_field` measures light delivered on n = +Y, replacing `KEY_LIT_FRACTION 0.75` |
| D35 | **The reference frame looks away from the sun.** `review_views.rs:434` looks +x with the sun at −x: every shadow hides behind its caster; the sun-haze scatter is zero antisolar | `scene_build/src/review_views.rs:434-435`, `lighting_common.wgsl:131-134` | a second canonical Prokhorovka evening view INTO the sun from the player's eye; a deep-shade (< 0.07 linear) FLOOR per frame; `OUTDOOR_DARK_TARGET` 0.08 asserted on the sunward frame |
| D36 | **Cloud shade cannot be seen and the lids cast hard shadows.** Cloud shade ≤ 13 % of field light in 150–300 m blobs of a different noise than the dome; the overcast look has a 74° white key with shadow strength 1.0 → 2.2:1 shadows, harder than the evening's, and a field 1.6× brighter | `shadow_common.wgsl:50-63`, `cloud_map.rs:24-25`, `lighting.rs:484, 507-508`, `shadow.rs:298` | cloud shade 0.3 → ~0.7 on clear looks, 2–3× finer; per-look shadow strength (the lid's key folded into ambient, shadow 0.3); locks: `cloud_shade_depth ≥ 0.3` on clear profiles, lid shadow ratio ≤ 1.3, a golden with ≥ 5 % of ground pixels ≥ 20 % darker than their 50 m neighbourhood |
| D37 | **The sky is lavender.** A linear mix of a blue zenith and an orange horizon lands at (0.50, 0.44, 0.46) in the played band; antisolar clouds are shaded blue | `shaders/sky.wgsl:94, 160-162` | N6's ladder or a three-stop gradient with a warm band and a hue-preserving mix; lock: sky-band mean R ≥ B on the golden looks, measured on the golden |
| D38 | **The leaf sprites are the darkest, most saturated object in every frame.** Authored clusters: mean linear luma 0.09, saturation 0.60 (the world's mid albedo 0.27, the window ≤ 0.45); occlusion baked in AND `CORE_SHADE 0.68` again; the amber key raises the saturation to 0.73; grade saturation 1.25 on top; 47 % of crown pixels below 0.07 vs 0 % of the ground. NOT a different lighting model (refuted: same key, ambient, fog, tonemap) | `assets/flora/*/clusters_color.png` (measured), `world_forge/src/tree/authored.rs:402`, `scene_build/src/foliage.rs:184-191`, `lighting.rs:478` | clusters re-baked at the world's scale (luma 0.15–0.25, sat ≤ 0.50), `CORE_SHADE` ≥ 0.85; a CPU lock on the embedded PNG: `cluster_sprite_albedo_sits_in_the_world_window` |
| D39 | **The vehicle is not PBR and three of four roles have no highlight.** Lambert + Blinn on the key only; `rough = role × (0.55 + G)` saturates: cast 0.81–0.94, track 0.80–1.0, rubber 1.0 → zero specular and zero environment; rolled plate 3 % Blinn; the metalness lane is read and nulled by `smoothness²`. Rated 3/10 for "reads as tonnes of steel" | `shaders/vehicle.wgsl:499-505, 514, 527-545`, `vehicle_forge/src/artifact/material_synthesis.rs:77-131` | an additive roughness (`role_r + (G − 0.5)·span`) with an ordered lobe per role (glass 0.1, barrel 0.45, rolled 0.55, cast 0.62, track 0.5 with live metal, rubber 0.8); lock: a CPU mirror of the formula asserting a strictly ordered, non-zero lobe per exterior role and env energy on TrackMetal > 5× Rubber |
| D40 | **The vehicle textures shimmer by construction.** 8 layers × 256² uploaded with `mip_level_count 1`, `Nearest` mip filter, per-texel white-noise normal/cavity/albedo; no hull LOD in battle (Lod0 always) | `renderer_wgpu/src/scene_renderer/vehicle_materials.rs:34, 133`, `material_synthesis.rs:191-194`, `asset_catalog_loader.rs:173` | mip chains at upload; two disciplined octaves (0.3–0.6 m) instead of the texel hash; lock: `mip_level_count == 9` and a spectral bound per layer above half-Nyquist (rule 5 for vehicles) |
| D41 | **Paint is one tone.** The largest variation scale on a hull is 0.38 m; rule 5 wants a 2–5 m macro octave; no edge wear, no curvature term, dirt confined to the gear and excluded from armour by design | `vehicle.wgsl:357-358, 366-369`; D8, D9 | a 3 m object-space octave on albedo/roughness; `fwidth`-driven bare-steel at convex edges; a ground-height mud band on every role; hub-centred oil; locks: local contrast at a 2 m kernel on the hero crop, bared steel on the cupola rim > 0, the lower-hull crop darker and warmer than the deck |
| D42 | **Garage and field disagree on the same hull with no lock.** The hangar adds +44 % ambient, a GI probe, exposure +5 %, black point −33 % and a dust film; the field's shaded flank falls under the black point; the bible asserts parity | `lighting.rs:645-737` vs `:242-304`, `vehicle.wgsl:400-405, 472`, `docs/vehicle-presentation-bible.md:14-15, 27` (tint value ≠ code) | one hull rendered under both rigs in a lock asserting lit-deck chroma within a band; the bible quotes the code's constant |
| D43 | **The ground is a marbled mat.** Three isotropic tone fields with no form: straw patchwork +29 % (65/19 m), the field quilt = contour bands of a smooth noise ±17 % with a grass↔straw swap, macro tone ±10 %; dirt luma 0.307 = grass 0.310 (the road has no value contrast); the clumps sit 2–3 per 8 m cell (a lattice), cast no shadow, wear the ground's albedo | `terrain.wgsl:99-123, 163-167, 249-257`, `ground.rs:238-240, 322-324`, `grass.rs:38, 425-435`, `prokhorovka-hill-252-2.map.ron:217-224` | dirt luma ≤ 0.85 × grass (data); plots as authored polygons with real edges (the quilt from them), `MACRO_TONE_AMP` and the patchwork halved; clumps 0–4 per cell under a 60 m occupancy noise; locks: a map-report check `dirt_luma ≤ 0.85 × grass_luma`, a transect step-edge count, a clump-count histogram not concentrated on {2, 3} |
| D44 | **A distant tree line is a green rectangle.** The hedge body is a 24-vertex LEGACY box, Lambert-lit with the cliff's strata on its faces, one flat value 90 × 15 px at 400 m; the test locks the box ("24 vertices") | `scene_build/src/battlefield.rs:1058-1097, 2387-2404`, `scene.wgsl:200-206` | the body as FOLIAGE with a broken top (the crown-hull mesh scaled per box, ≤ 120 tris/box); locks: `no_tree_line_vertex_rides_legacy`, the top varies ≥ 0.5 m along the run |
| D45 | **The garage hides its hero.** The muzzle lands 88 px under the stats plate (silhouette centre x ≈ 920 vs the UI-free band's 775); the gate gap is an HDR sky; the turntable is 1.9× the floor after K24-1; the crew plate is 330 × 400 u for five words (VR-06; the loadout strip does NOT cover the hull — it covers the turntable) | `client/src/app/garage/camera.rs:53-59`, `scene_build/src/hangar.rs:47, 62, 94-96, 139, 418-434` | a hero `pivot_offset` of +1.0 m along the hull forward; `TURNTABLE` ≈ 0.22; the crew plate a 62 u strip (interface session); lock: the projected muzzle and stern inside the panel-free band on every vehicle |

### U — interface (routed to the interface session; `docs/interface-policy.md` owns these files)

Inherited U1–U11 stay with that program. New, from the graphics review verified in code:

| ID | Defect | Evidence | Closes when |
|---|---|---|---|
| U12 | **Labels are stencil at 1.0 px stems.** `Style::LABEL` = Big Shoulders Stencil Medium 16 u → em 13.3 px, stem 1.0 px at 900p; Plex SemiBold 20 u = 2.1 px; the goldens are 960 × 540 (stem 0.6 px) and could not show it; 20 u overprints the value in compare mode, 18 u fits | `garage/screen.rs:697-704, 605-612, 389-390`, `chrome.rs:55-62`, `ui_kit/src/font/manifest.rs:55`, `look_goldens.rs:29-30` | a dated policy row: stencil for headings ≥ 24 u only; labels → `VALUE_STRONG` 18 u; locks: no stencil below 24 u, a garage frame at 1080p under the image lock |
| U13 | **The reticle numbers fail the policy's own 3:1.** Pen "201" at 1.07:1 on straw, armour "120" 1.62:1, range "214" 1.89:1 — drop shadow only, no plate; the 3:1 lock skips text without a plate (the reticle is a legacy payload) | `hud/reticle_readouts.rs:91-121, 193-221`, `font/layout.rs:61-73`, `look_goldens.rs:1099-1107` | an enamel plate under the two readout rows (a dated H25 exception); the 3:1 lock extended to the legacy numbers on the golden |
| U14 | **Range printed twice; the zoom label answers no question** | `hud/marker.rs:250`, `reticle_readouts.rs:193`, `hud/readouts.rs:25-39`, `reticle_overlay.rs:11-35` (H27's table) | the marker's range dropped, the zoom label burned (ratchet ceiling 6 → 5), both in the audit table |
| U15 | **No compact hit-log form.** Six 620 px rows for 8 s in Standard; one formatter | `hud/damage_log.rs:20-21, 81-139, 320` | a short row by default (outcome · damage · target), detail on N — a new decision row on the closed H8 |
| U16 | **Texture competes with text.** After #801: brushed ±44 %, painted up to ±52 % (`TILE_NEUTRAL_GAIN 2.0`), ~35 plates in one olive, no glass in the garage | `ui_kit/src/sheet.rs:174-230`, `hud.wgsl:28`, `theme.rs:140-183` | DECIDED: #801's plate reading stays on bars; any plate carrying text ≤ 12 u takes the enamel tile; amplitude behind letters ≤ ±10 %; the `SHEET_HASH` re-pinned once |
| U17 | **The crew plate is 330 × 400 u for five role words** | `garage/screen.rs:581-615` | a 62 u strip (with D45) |

### K, P, C, Q, N, H, R, F, O, A, L, M — inherited, unchanged

K0, K3–K6, K9–K19, K21–K23 (K24 CLOSED: `Nation::paint` merged as #780/#781 — one RGB per nation; the materials are K6), P1–P3, C1–C4, Q1 (its terrain half = T9), Q2, Q4, Q7, N1–N8, N9 (the owner's WSL), N11, H1–H6, R2, R3 (R1 amended above, R4 decided), F8–F11, O1, O2 (every D34–D45 PR rewrites the lock it names), A2, L1–L3, M5b/M7b/M8 (`docs/game-modes.md`). Full text in the second pass.

## 4. Lying and blind locks (rewritten in the PR that closes their row)

| Lock | Measures | Must measure | Row |
|---|---|---|---|
| `look_locks.rs:48` `KEY_LIT_FRACTION 0.75` | a slope tilted 26° toward the sun | light delivered on n = +Y | D34 |
| `look_goldens.rs:398` `OUTDOOR_DARK_FLOOR 0.0055` vs `TARGET 0.08` | never worse than the worst frame | the target on the sunward frame | D35 |
| `every_sunlit_pass_takes_the_cloud_shade` | that the multiply exists | that a patch is visible | D36 |
| `tree_line.rs:526-586`, `battlefield.rs:2387` | the box contains the trees | the body is not a box | D44 |
| `grass.rs:799` Clark–Evans | randomness of tufts | passes with a lattice of clumps; a clump-count histogram | D43 |
| `material_floor.rs` | a minimum material amplitude | also a maximum frequency (mips) | D40 |
| `every_battle_readout_sits_on_glass_at_three_to_one` | text on a plate only | also the legacy reticle numbers | U13 |
| `no_garage_string_renders_below_the_legibility_floor` | `size_u ≥ 16` | the face and the stem in px | U12 |
| `HERO_OVER_ROOM 1.5×` | median hero vs room, no UI | the silhouette inside the panel-free band | D45 |
| `t54_top_track_links_ride_the_wheels_with_only_the_horn_in_the_slot` (`− 0.075`) and `running_gear_dynamics.rs:110` | a plate 75 mm inside the tyre "rides on it"; "the top run does not move" | a clearance lock over a driven crest and ditch | J7 |
| `the_result_does_not_depend_on_roster_order` (2 bodies) | one constraint | ≥ 3 bodies, client vs server summation order | X8 |
| `tbone_ram_registers_only_when_hulls_actually_touch` (+0.5 m) | that a ram registers | the bill's spread over the sub-tick phase | X7 |
| `windmill_shelf_masks_a_hull_from_the_bridge_but_fires_over_the_crest` | the eye | the shell | S21 |
| `shell_falls_under_gravity_after_firing`, `…uses_deterministic_dispersion`, `fixed_casemate_ignores_turret_yaw_commands` | only `v.y < launch`; one draw; a mechanically false zero | drop and TOF; two draws and the salt; the limited arc | S14, S16, S17 |
| `cover_visuals_never_leave_the_collision_box` (one direction) | mesh ⊆ box | also box ⊇ mesh (fill) per kind | X1 |

## 5. Verification

The merge gate is `scripts/verify.ps1` (see `CLAUDE.md`); `scripts/preflight.ps1` first. In addition, for this program:

- Any renderer or profile change (D34–D45, N-, T-, F-rows) lands with an MX330 A→B→A cold
  sandwich; the budget moves per item, never fleet-wide. The picture rows land with a frame
  sent to the owner, and the frame is taken from the player's eye, once INTO the sun.
- Any sim number that moves (J1–J6, X-, S-, Z-rows) re-pins the replay fixtures in the same
  PR and says why; `mobility_baseline.rs` is re-recorded as a set, never a column at a time.
- Any wire change (a `StaticCoverKind` tier, a phase byte per segment, a fall-direction byte,
  the dispersion salt, the gearbox state if it rides the wire) is append-only and bumps
  `PROTOCOL_VERSION`; the yaw = 0 / bands-overlap path stays bit-identical on every replay.
- Goldens are blessed deliberately, in the PR that moves them, with the before/after in the
  message; HUD and garage goldens only from the interface session.
- Every row that names a lying lock (section 4) rewrites it in the same PR.
- Each vehicle migration (K3) ends with the close-up review under the model-logic bar and the
  K0 overlay; each destruction class (Z8–Z13) ends with a `destruction_showcase` frame looked
  at before merge; the representative fragment (W4) ends with the owner playing it.
