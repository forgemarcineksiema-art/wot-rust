# Inny Poziom — The Second Pass

Approved 2026-09-01. The owner named ten things that read as unfinished — the geometry kernels and
the fleet, the picture, the physics, Honest Steel, the armour, the tracks, the flora, the shot, the
fleet's identity, the HUD — and asked for all of them to be taken "to a completely different level".
Eight read-only audits of the code and a look at the review frames in `target/` answered with one
diagnosis, and this document is that diagnosis turned into a register and a wave plan.
[art-direction-program.md](art-direction-program.md) is the model: the register below names the
evidence, the wave, and the lock that closes each row. When the register is empty this document
becomes history and the policies it edits stand alone.

The same day the owner added a second list — the driving camera, shots refused or landing for zero,
the enemy hard to read in the scope, the trajectory, a hit that is only a number, frame drops, the
garage, the T-54 itself, the hull over a small hill, buildings and terrain, sky and weather, water —
and granted this program full design authority: "if something needs redesign, go ahead; you take
responsibility." Eleven more audits answered. The rows they produced are in the register below, and
the redesigns they justified are in the decisions, stated as taken.

## The diagnosis

**The simulation is mature and test-locked in nearly every area named. What is thin is the layer
the player sees, and the rollout.** The same disease repeats in nine of the ten areas: a good thing
was built once — for one vehicle, one map, two species, two turrets — and never made a rule.

| Built once | Rolled out to | Evidence |
|---|---|---|
| Hybrid geometry (27 565 triangles, 69 parts) | 1 of 8 vehicles; the next best is 3 734, the Centurion 1 960 | `cargo test -p vehicle_forge --test shipped_cost`; `vehicle_forge/src/mesh_source.rs` hard-codes `T54_1951 → Hybrid, _ → Procedural` |
| Drzewa 3.0 species (6 grown) | 2 placed (Oak, Bush); Poplar, Willow, FruitTree, Pine stand nowhere | all five `map_forge/blueprints/*.map.ron`; Orliny's view named `pine_belt` contains no pine |
| Armour↔metal parity lock (≤ 10 mm) | 2 turrets of 8 (T-54, Tiger I); hulls, sides and decks unmeasured on 6 vehicles | `vehicle_build/tests/t54_turret_armor_lock.rs`, `game_core/tests/tiger_i_benchmark.rs` |
| Destructible cover authored in anger | 1 map of 5 — Ostrogorsk 39 objects; Prokhorovka 14, Mazurski 9 | map blueprints, `terrain/src/battlefield.rs` HP table |
| Baked contact AO | 1 vehicle of 8 | art-direction D8 |
| Weather timeline | 1 map of 5 — `static_program = map != BystraValley` | `weather_timeline.rs:56` |

The second list added a third pattern: **one global constant standing in for a property of the
thing.** One rate limit (1.4 rad/s) is every hull's suspension; one drowning depth (1.5 m) is every
hull's fording depth; one dispersion factor is three; one rain wind is hard-coded while grass reads
the storm heading; one "0" is drawn for five different outcomes.

Two mechanisms keep it invisible:

1. **"DONE" in the roadmap blinds the register.** `docs/ROADMAP.md` lists fire feel, destruction and
   the flora stack as done, so none of them has a defect register; the only register in the repo is
   the light's (`art-direction-program.md`). A system without a register cannot accumulate debt on
   paper, so its debt accumulates on screen.
2. **Documents lie in the details.** `docs/vehicles/t-54.md` quotes a 22 000-triangle cap against
   29 000 in `t54.rs` and 27 565 measured; `vehicle-forge-policy.md` marks UVs and normal/AO bakes
   "done" when one kernel authors UVs; `procedural-kernel-program.md` M8 "migrate other vehicles"
   has read "in progress" since 2026-08-03 with zero migrated; `hybrid.rs` still describes an SDF
   composition deleted 2026-08-02; `scene_build/src/backdrop.rs` calls its ring "FAR bakes that at
   kilometres read identically" while the ring starts 40 m past the border; the roadmap's creed
   promised "dispersion ~0.1–0.3 mrad" while every gun ships 1.9–3.4; the roadmap's Movement line
   claims "per-wheel suspension with sprung attitude" while the authoritative hull is a rigid beam
   and the springs live in the presentation layer.

## The three rules this program is built on

1. **Measure the eye, not the model.** Every register row names a frame or a number, and the lock
   that closes it. No row closes on a description. Where no metric exists yet, the row says so and
   the first PR of its wave derives one from a frame judged good — never from the broken frame
   (the lesson of art-direction D31).
2. **Rollout before invention.** A capability counts as landed when it stands on every vehicle,
   map or species it applies to. No wave starts a new capability while the previous one is on
   one-of-N. This is the fix-as-rule rule at program scale.
3. **One truth per system.** Where the audit found N models of one thing — fifteen track models,
   three ground samplers, damage modelled three times, twenty-two sets of HUD floats, two
   penetration resolvers, two cloud fields, three wind constants — the wave that touches it leaves
   one owner and every consumer reading from it.

## Decisions taken with this program

- **No physics engine.** `crates/runtime/physics` is ~2 800 lines of deliberate custom code
  (SAT footprints, a Jacobi impulse solver, heightfield contact); `parry3d` has one call site with
  no production caller. rapier3d would replace the integrator, contacts and friction — the parts
  the repo already rebuilt on purpose (`docs/physics-policy.md`) — and would not touch a single
  item the owner complains about: belt geometry, sag, link-to-wheel wrap, ruts, per-wheel travel
  and belt scroll are presentation and stay hand-built. The replay-exact locks need cross-platform
  bit determinism; rapier's `enhanced-determinism` promises same-binary determinism only.
- **The authoritative hull becomes a sprung mass on its road-wheel stations (G7).** The rigid beam
  plus a global 1.4 rad/s attitude ramp is replaced by per-station spring/damper forces with travel
  limits, integrated semi-implicitly at the fixed 60 Hz tick in f32 — the same arithmetic the
  presentation spring already uses, so determinism and replay-exactness survive. The force model,
  belt-drive steering, SAT contacts and the two-phase tick order stay verbatim; the beam's convex
  envelope survives as the springs' rest-length source (its trench bridging and crest overhang are
  correct). Weight transfer, crest compliance, landing rebound and per-vehicle wallow fall out of
  mass and station layout with no new knob. Spring state enters the snapshot append-only.
- **Gravity is 9.81 m/s².** `GRAVITY_MPS2 = 12.0` is undocumented in every policy file; an honest
  tank drops its shell at the real rate. The aim solver integrates the same step, so it adapts; the
  replay fixtures re-pin once.
- **The dispersion creed is rewritten, the data stays.** The guns' 1.9–3.4 mrad is a hard maximum
  radius with a √ centre-biased draw — already tighter than WoT's 3.2–4.2 and without its tails.
  Bringing guns to 0.1–0.3 mrad would give a 12 cm cone at 400 m and delete aim time, bloom and the
  aiming circle. The roadmap line now says what the envelope is.
- **Fire is never refused for geometry.** A shell into a crest or over a wall is a legitimate play
  (HE over cover). What changes is that BLOCKED is drawn identically in both camera modes, the
  elevation-limit case gets its own glyph, and a casemate's yaw limit enters the reticle's arc.
- **Trees never cut the camera boom.** Trunks and shelterbelts leave `camera_obstacles`; the eye
  passes through canopy with a fade, as it does in every shipped tank game. Buildings, walls and
  terrain still cut, with a rate-limited inbound and a velocity look-ahead so the eye never clips.
- **No rigid debris.** Collapse is client theatre (particles, cards, dust, sound) over the
  replicated phase swap. Debris that could block a hull would have to be replicated, and the
  honesty doctrine says scenery never blocks gameplay.
- **Buildings become a grammar-driven, instanced kit of parts (B3).** Today every mullion is unique
  triangles merged into a 1 000 m static buffer. A kit (wall bay, pierced bay, door bay, roof
  segment with overhang, ridge/eave caps, chimney, dormer, downpipe, plinth, dirt apron) placed by a
  split grammar and drawn through the existing instanced path costs less than today and reads more.
  The blocking volume stays the wall-and-roof mass; overhangs, chimneys and downpipes are
  scenery-class (never block gameplay, ≤ 0.4 m past the footprint, and a spotting lock proves a
  cornice cannot hide a hull). SDF/CSG per building is refused: CPU-heavy at map swap and hostile to
  the golden-hash gate.
- **The sky is a precomputed scattering LUT, not a two-stop gradient (N6)**, which also yields the
  sun's colour ladder by elevation instead of hand-typed keys. Clouds keep their 2D FBM but gain a
  lighting model (Beer–Lambert toward the sun, powder, forward lobe); true volumetrics are refused
  on the MX330. **One cloud field** feeds the dome and the shadow map. Rain becomes screen-space
  streaks plus a real wet response. Weather evolves on every map. One wind field.
- **Fording depth is a vehicle property and drowning is a replicated countdown (H1, H2).** The
  fleet-wide 1.5 m and the silent engine death are replaced by `ford_depth_m` from each dossier and
  a `submerged_s` lane mirrored by the rack-countdown widget. Bots derive their deep-water line
  from it.
- **Water reflects the world at half resolution (H5).** Screen-space reflection for the water pass
  only, measured before it ships; a planar pass (the scene twice) is refused on the budget.
- **The damage number is demoted, the world promoted (S7).** A literal "0" is never drawn; the
  outcome word is. The number's ink may not exceed twice the lit area of the penetration FX.
- **GPU uploads are dirty-range writes into persistent buffers (Q1).** Three whole-mesh
  `create_buffer_init` paths (ground, meadow, statics) are the shape of every remaining hitch.
- **No external UI crate for the client.** egui brings its own look and font stack against the
  one-look policy and a second pipeline against the single HUD draw call the MX330 budget is built
  on. It is allowed in `apps/editor` and `apps/tools`, where no look policy applies. The client's
  layout layer is written inside `ui_kit`.
- **The T-54 goes to the bar before the fleet copies it (K0).** Seven migrations reproduce the
  benchmark's parts; the benchmark's own defects (sprocket, wheels, links, dome length) would be
  copied seven times. K0 and K9–K15 land before the first migration.
- **Terrain and buildings are in scope now (W5)**, by the owner's second list. The earlier "recorded,
  not rebuilt" (K8) is retired; K8's inventory became the B and T rows.
- **Crew proficiency stays pinned at 1.0** (R4) — progression is proof, never power.
- **Night, snow and lightning stay absent.** No shipped map is a night or winter map; adding one is
  a map decision, not a renderer row.

## Defect register

Columns: the defect, the evidence, the wave, and what closes it. IDs by area: K kernels and fleet,
G tracks and ground, C camera, A aim and sight, Z destruction, P armour, F flora, S the shot,
Q the frame, R roles, U interface, B buildings, T terrain, N sky and weather, H water, O picture.

### K — kernels and the fleet

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| K0 | **No reference-overlay gate.** `t54_silhouette.rs` rasterises the turret into its own bounds (proportion only); `review_images.rs` takes no reference input; the Blender overlay is a manual loop. The dome's +45 mm (K15) is exactly what such a gate catches | `vehicle_build/tests/t54_silhouette.rs:15-46`, `docs/vehicles/refs/t-54.md:48-52` | W3 (before migrations) | own-drawn outline polylines (side/front/plan) from the dossier drawing in RON; CPU raster of hull+turret+gear; IoU ≥ 0.95 per view, per vehicle, as a gate |
| K1 | **The recipe seam.** `vehicle_recipes` depends on `revolve` + `vehicle_geometry` only; `cast_loft`, `panel`, `solid`, `detail` are reachable solely from `vehicle_build`, whose 19 files are all `t54_*`. `mesh_source.rs:23-28` and `production_bake.rs:18-25` hard-code the T-54 as the only hybrid vehicle | `vehicle_recipes/Cargo.toml`, `vehicle_forge/src/mesh_source.rs` | W3 | one mesh-source rule with no per-kind match; every kernel is a legal dependency of a recipe (layer rule updated, `layer_rules.rs`) |
| K2 | **T-54 content lives inside a kernel crate.** `solid/src/t54.rs`, `t54_fittings.rs`, `t54_plates.rs` | `crates/kernels/solid/src/` | W3 | no vehicle-named file under `crates/kernels/`; a quality gate greps for it; the parts become a fleet part library in `vehicle_build` |
| K3 | **Seven vehicles are unbuilt.** Bake sizes: T-54 27 565, Tiger I 3 734, Panther II 3 000, Jagdtiger 2 864, Tiger II 2 828, IS-3 2 206, T-34-85 2 064, Centurion 1 960. A Centurion turret is a 5-station ellipse ring from a scale table shared by four vehicles (`turret_fittings.rs:481-521`); a mantlet is a 3-number tuple; there are no fenders | `shipped_cost`, `centurion.rs` (54 lines) vs `vehicle_build` (4 566 lines) | W3 | each roster vehicle carries every part class of the fleet part library its dossier lists (hull solids with armour angles, lofted turret, revolved mantlet, fenders, hatches, grab handles, tow cable, vision blocks, stowage, exhaust, lights, weld seams) — an inventory gate per vehicle — and passes the T-54's close-up review set (`closeup_probe`) under the model-logic bar, and K0's overlay gate |
| K4 | **No mesh boolean.** Every kernel is convex-only (`solid/convex.rs:5`, `sweep/lib.rs:77`, `panel/lib.rs:49`). Apertures are faked: the embrasure is a Gaussian dent (`t54_turret_loft.rs:49-56`), hatches are drums on a roof, the grille well is boxes. CSG exists in `sdf` and is unmeshed and dead (0 construction sites of `PartShape::Cast`) | `vehicle_build/src/part.rs:128,152` | W3 | a `cut` operator (convex tool subtracted from a solid or loft, watertight result) locked by a manifold + volume-delta test and used by at least one hatch and one embrasure on at least two vehicles; `sdf`/`sdf_mesh` either become that operator's path or are deleted |
| K5 | **No edge topology, so no fillets or rolled edges.** Chamfer only on axis-aligned boxes; the general pass was withdrawn (`solid/src/t54_fittings.rs:14-21`); `chamfered_prism` bevels 4 of 12 edges; loft caps are flat fans | `vehicle_geometry/src/builder.rs:24-58` | W3 | edge adjacency in the mesh contract; a fillet operator under the roundness law (segments from radius); locked by a facet-angle bound on filleted edges |
| K6 | **No per-part UV or bake.** One kernel authors `uv0` (`solid/convex.rs:161`); the rest are triplanar; `mesh.rs:78-80` claims otherwise. Normal/AO are per-role noise (`material_synthesis.rs`), and the synthesis is tuned to invisibility because it has nothing real to show. Cast grain on the T-54 is AO bands only (`t54.rs:340-344`) | `vehicle_forge/src/artifact/material_synthesis.rs` | W3 | every kernel output carries authored UVs (a test over kernel outputs); per-part normal + AO bake with a golden; cast grain as a baked normal on cast roles; `vehicle-forge-policy.md` row corrected |
| K7 | **The documents lie.** 22 000 vs 29 000 cap; "UVs done"; M8 "in progress" with zero migrated; `hybrid.rs` describing deleted SDF; tracks "use `revolve`" while links are box prisms; the forge manifest (2026-08-09) says 24 577 triangles against 27 565 measured today | `docs/vehicles/t-54.md:193-195`, `docs/vehicle-forge-policy.md:231-234`, `docs/procedural-kernel-program.md`, `target/forge/t54-1951/manifest.json` | W3 (first PR) | a `roadmap_claims`-style anchor pins the T-54 budget to the measured bake; the stale paragraphs are rewritten to the code |
| K9 | **The sprocket does not engage the track.** Teeth are 7.7 mm nubs proud of a ⌀463 drum, tip r 0.231 inside the belt (real tip r 0.341 passes through the shoe window); the ring is 44 mm wide against 120; the 40 ring bolts sit entirely inside the ring's metal (x 0.260–0.280 within 0.240–0.284, r 0.186–0.218 within 0.181–0.224) — 960 dead triangles no lock looks at; `gear_t.rs:776` pins the wrong OD (`reach ≤ 0.268`) | `vehicle_geometry/src/running_gear_end_wheels.rs:197,220,242-249` | W3 (K0 block) | teeth to r 0.341 through closed windows cut in the link plate (K4's `cut`), ring 120 mm, bolts proud; gate: tip ≥ plate outer r + 0.02 and a ray through the window; fleet rule "a fastener's AABB is not contained in what it fastens" |
| K10 | **The top run is rigid above travelling wheels.** Per-wheel travel is a client residual clamped −0.08..+0.20; a wheel at +0.20 puts its rim 0.18 m through the 0.90 run, at −0.08 the run floats 10 cm above the wheel it rests on; `running_gear_dynamics.rs:109` locks the defect ("the top run does not move") | `vehicle_geometry/src/running_gear_place.rs:255`, `client/src/vehicle/render_frame.rs:297-322` | W2 (with G7) | carrier height = wheel centre + lift; lock: a wheel lifted +0.15 raises the links over it ≥ 0.12 and no link lies inside any wheel radius |
| K11 | **Road wheel dimensions are wrong and the "holes" are sectors.** Assembly 360/157/45 mm against the drawing's 423/185/53; holes are the gaps between straight ribs, and the lock measures metal fraction only | `t54_1951.blueprint.ron:92`, `running_gear.rs:285`, `road_wheel_faces.rs:91-110` | W3 (K0 block) | `DimensionKind::RoadWheelWidth` 0.423 ± 0.01; 24 round punched holes under the roundness law with a hole-rim lock |
| K12 | **Links are seven box prisms** (plate, two 18 mm flat eye bars, two ribs, a 32×56 mm horn, backing; 84 tris) with no pin, no windows; the idler crank is a stub pointing straight down; the idler web ring is 90 % buried in the rim | `running_gear_belt.rs`, `running_gear_end_wheels.rs:41-47,123-150` | W3 (K0 block) | OMSh link: cylindrical eye barrels, windows beside a 50 mm horn, pin ends; idler crank pivoting aft; part-inventory rows and a through-opening ray test |
| K13 | **The world-space lock rule is partial.** The three 2026-08-09 P1s are fixed (`gear_t.rs:936,989,1037`, `handedness.rs`, `t54_nose_honesty.rs`), but fittings handedness enumerates 5 named parts, not every asymmetric part, and no rule says a fastener must show a face outside what it fastens (K9's bolts prove it) | `vehicle_build/tests/handedness.rs`, `fleet_running_gear.rs:115` | W3 (K0 block) | handedness over every part whose mesh is asymmetric in x; the fastener rule as a fleet walk |
| K14 | **Fuel tanks and bins float on the fenders** — no brackets or straps; barrel profile stations are unsourced; spare track links and hull grab rails absent | `vehicle_build/src/t54_*`, `gun_parts.rs:30-38` | W3 (K0 block) | brackets/straps as parts; barrel stations from the dossier; inventory rows |
| K15 | **The dome is 45 mm too long** (1.28 + 1.128 = 2.408 against the dossier's 2.363) and only a ratio lock (±0.06) watches it | `t54_hybrid_turret.rs:81-87` | W3 (K0 block) | dome length pinned to the dossier ± 10 mm; closed for real by K0 |

### G — tracks and the ground

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| G1 | **Three ground samplers per hull per frame.** The support envelope (`physics/src/track_contact.rs`, rubble-aware, convex hull of stations), the probe cross (`physics/src/contact.rs:67-118`, four samples around the hull centre), and the client's per-wheel residual (`client/src/vehicle/render_frame.rs:300`, no rubble). They disagree exactly at crests and ditches. The drive's slope term reads the 3 m probe (0.189 on the audit hill) while the attitude reads the beam (0.224) | the three files, `forces.rs:41-44`, `contact.rs:83` | W2 | one function owns ground contact; the client reads station heights from the physics result; `render_frame.rs` no longer samples the heightfield; drive slope and attitude slope are one number; `running_gear_dynamics.rs:233` extended to assert rendered station heights == physics station heights on a crest |
| G2 | **Belt speed is re-derived client-side from pose deltas** (`engine/src/components.rs:117`); slip, a thrown belt's asymmetry and forced yaw never reach the visuals | `engine/src/components.rs:100-135` | W2 | per-side belt speed on the wire (append-only, protocol bump); the scroll reads it; locks: a thrown belt scrolls zero while the hull is dragged, a slipping belt scrolls faster than ground speed |
| G3 | **The FX path invents the gauge.** Ruts and the shed ribbon use `hitbox.half_width_m * 0.86` as the track centreline — 1.505 m on a T-54 against the real `half_gauge_x` 1.32 m. The same class of bug P2.1 fixed for belt scroll | `client/src/app/motion_fx.rs:93`, `client/src/vehicle/track_ribbon.rs:88` | W2 | one source (`half_gauge_x` from the blueprint) under `single_source_constants`; lock: rut centreline within 2 cm of the belt centreline |
| G4 | **Damage is modelled three times**: the HP pool (`game_core/src/track.rs`), drive scalars (`sim/src/drive_modules.rs`, `DAMAGED_SPEED_FLOOR`, `BROKEN_ONE_*`), sag tiers (`render_frame.rs:338`) | the three files | W2 | one `TrackCondition` derived from HP, consumed by drive and render |
| G5 | **Admitted and open**: gradeability really 0.42 against a documented 0.60; per-belt ground unimplemented (`contact.rs:105` samples material at the hull centre); cm-scale velocity kinks from the 5 m heightfield snap (`docs/battle-camera-policy.md:101`); low obstacles-as-ground deferred (P2.2) | `docs/contact-and-tracks-program.md` register | W2 | per-belt ground material; gradeability locked at the documented value or the document corrected to the measured one; the kink bound is G7's continuity lock |
| G6 | **Fifteen track models** in total (damage pools, drive status, belt-drive steering, support envelope, probe cross, vertical follow, authoritative attitude, footprint SAT, a dead parry query, blueprint `TrackShape`, geometry running gear, client suspension, belt scroll, presentation spring, ribbon/ruts/audio/HUD). Only one cross-check exists (`running_gear_dynamics.rs:233`) | audit 2026-09-01 | W2 | the count is not the target; the cross-checks are — every pair that must agree (stations, gauge, condition, speed) has a lock, and `parry_query.rs` is deleted |
| G7 | **The hull is a rigid beam on a global ramp — the arcade moment is structural.** There is no sprung mass in the authoritative model (`hull_attitude.rs:2` "deliberately spring-free"); pitch is the slope of the convex hull under the stations, rate-limited at 1.4 rad/s (80 °/s), clamped ±0.6 rad; height is a hard snap (`vertical.rs:82-84`). On a 1.2 m × 12 m hill at 8 m/s the 5 m grid turns the hill into a tent with a 25° crease; when the apex crosses the 0.906 m gap between stations the pitch target swings +0.220 → −0.220 rad in 6.8 ticks (223 °/s demanded), the limiter takes 18.9 ticks, peak lag 16.2°, pitch rate a square wave; the beam rides 4 cm below the apex. No value of the one constant is both un-snappy and un-floaty. A T-54 and a Jagdtiger pitch identically; weight transfer is a 2.0°-capped client illusion the gun and armour cannot see; `mobility_baseline.rs:56` already admits "gradeability is a fleet constant, not a vehicle property" | `physics/src/track_contact.rs:157-170`, `hull_attitude.rs:17-32`, `vertical.rs:18-22,82-84`, `engine/src/attitude.rs:11-31` | W2 (first) | the sprung-hull decision above. Locks: (1) crest-walk continuity — pitch acceleration bounded, no square wave; (2) per-vehicle wallow — T-54 and Jagdtiger pitch natural frequencies differ by the ratio their mass and station layout predict; (3) brake dip is authoritative — gun depression measured from the sim loses the dip; (4) landing rebound at every crest; (5) `mobility_baseline.rs` rows byte-identical, `climb_envelope.rs`, hull-down and replay-exact locks survive; `physics/tests/hull_attitude.rs:80,94` rewritten to the sprung contract, replays re-pinned once |

### C — the driving camera

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| C1 | **Trees cut the boom.** `TreeTrunk`, `TreeLine`, `Wreck`, `StoneWall` all enter `camera_obstacles`; only terrain gets bisection — cover is an exact entry `t` with an instant cut and no inbound smoothing, released at 14 m/s. Driving a shelterbelt slams 12 m → ~2 m per trunk | `client/src/app/live_cover.rs:36-40`, `camera/collision.rs:31-35`, `camera/present.rs:201` | W2 | trees and shelterbelts leave the obstacle list (the decision above) with a canopy fade; lock: a drive through the treeline never changes the boom |
| C2 | **The cut moves the gun, and there are two cameras.** The gun solves from the logical camera (no boom smoothing, a hard eye-over-terrain clamp, `collision.rs:95-105`), the view and reticle draw from the presented camera; every boom cut and clearance clamp pivots the sight ray about the target — the gun swings while the view eases | `app/reticle.rs:30`, `app/camera_link.rs:77-84`, `app/render.rs:506,589` | W2 | one eye: aiming reads the presented camera; inbound cuts against buildings/walls/terrain are rate-limited with a velocity look-ahead so the eye never clips; lock: no frame where the sight ray and the view ray diverge by more than the look-ahead bound |
| C3 | **The vertical leash is saturated on every slope.** A critically damped spring tracking a 1.76 m/s ramp needs 0.39 m of lag against a 0.05 m leash — it is a rigid offset that crawls 0.10 m at 0.225 m/s (0.44 s) at each slope reversal | `camera/smoothing.rs:24-57`, `camera_feel.rs` | W2 (after G7) | re-measured once the hull is sprung (its own motion becomes C¹); then rate-based tracking (match hull vertical velocity, capped) instead of a displacement leash. Refused and not re-proposed: ω7/0.35 m, 0.10 m, isotropic ω16, heave into the camera, 0.02 rad dive, 3 m/s kick, 29.7 Hz tremor, full-velocity anchor matching |
| C4 | **The terrain crease reaches the eye ~1:1.** `sample_height` is piecewise-linear over triangles with a data-dependent diagonal that flips cell to cell — 2–4 velocity corners per second at 10 m/s, each 0.2–0.6 m/s | `terrain/src/heightmap.rs:52,132-145` | W2 → T1 | the camera anchor samples a smoothed surface along the path; the count halves with T1's 2.5 m ring |

### A — aim and sight

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| ~~A1~~ | ~~**The reticle and the server resolve penetration by different code.** `penetration_from_outcome` dropped the thickness scale (the trace outcome never carried it) and resolved a bare track/skirt band while the server summed the spaced stack plus the hull side. A 20 mm track read green, the server charged band + belt → "0" + TRK. No test compared the two~~ — **CLOSED (2026-09-01, `fix/one-penetration-resolver`)**: one `game_core::resolve_traced_impact` over a `TracedImpact` (shell, armour, hull attitude, zone, angle, distance, thickness scale, shell direction, standing belts); the sim gathers it from `TankState`, the reticle from the snapshot; `TraceOutcome::Tank` carries `thickness_scale` and `direction`; `sim::verdict_for_traced_impact` is the seam's measuring point. Locks: `the_reticle_and_the_server_agree_on_ten_thousand_traced_impacts` (whole roster, every round, random attitudes, a third of belts thrown — penetration, effective armour and remaining penetration must all agree) and `a_track_hit_prices_the_belt_and_the_side_plate_behind_it` | `client/src/hud/reticle.rs`, `sim/src/combat.rs`, `game_core/src/armor/impact.rs` | W1 | one resolver in `game_core` called by both; a property test over 10 000 traced impacts asserting `hint.penetrates == server.penetrated` — done |
| A2 | **The creed contradicted the data by 10×.** ROADMAP promised 0.1–0.3 mrad; guns ship 1.9–3.4 (radius, hard max, √ draw, `aim_dispersion.rs:69,90,94-99`). At 200 m a fully aimed T-54 has a 58 cm cone; P(0.5 m cupola) = 65.6 %, at 400 m 46.4 % — that is what "hard to hit exact spots" is, and it is WoT-class by design | `docs/ROADMAP.md:11`, `catalog_soviet.rs:229` | W1 (first PR) | the creed line rewritten (done in this PR); a `roadmap_claims` anchor pins the fleet's rest-dispersion range to the catalogs |
| ~~A3~~ | ~~**A pitch limit looks like a wall.** A target below the gun's depression clamps `in_arc=false` → the same Blocked form, the shell into the dirt — indistinguishable from cover~~ — **CLOSED (2026-09-02, `fix/sight-feedback-a3-a5-a6`)**: the firing solution names which end of the arc bit (`ArcLimit::{Depression, Elevation}`), the feedback carries it, and the overlay draws the arc's own form in BOTH modes — a stop bar under (or over) the ring joined by a stub, and "DEPRESSION LIMIT" / "ELEVATION LIMIT" under the block distance; lock `an_arc_limit_wears_a_stop_bar_and_a_label_in_both_modes`. The audit's "nearly mute in third person" was half wrong: the block distance was already drawn in both modes; the impact X and the pen hint stay sniper-only by `docs/aiming-model-policy.md` (third person is situational awareness and speaks no armour), a policy this program keeps | `app/aim.rs`, `hud/reticle.rs`, `hud/reticle_overlay.rs`, `hud/reticle_readouts.rs` | W1 | a distinct pitch-limit form in both modes, locked — done |
| A4 | **Yaw reachability is never tested.** `in_arc` checks pitch only; a casemate's yaw is forced to 0 (`aiming.rs:30-33`) so a Jagdtiger's reticle is green off-axis and the round leaves down the hull line; `seam_tests.rs:164` skips every out-of-arc placement as "a vehicle/map question" | `app/aim.rs:195`, `hud/reticle/seam_tests.rs:164,230` | W1 | `effective_turret_yaw` folded into `in_arc`; the seam sweep counts out-of-arc placements as a third counter with its own ceiling |
| ~~A5~~ | ~~**A silent refusal.** A fire input while the session is not ready is dropped before any feedback runs~~ — **CLOSED (2026-09-02, `fix/sight-feedback-a3-a5-a6`)**: the click into a session that cannot take it arms the red pulse and the UiReject knock before the edge is retired; lock `a_fire_click_into_a_session_that_cannot_take_it_is_refused_out_loud`. The second half of the audit's row was wrong: the 30-tick deploy shield IS set (`garage/actions.rs`) and swallowing the second half of a double-click on BATTLE is its purpose | `app/loop_step.rs` | W1 | every refusal reaches the UiReject knock — done |
| ~~A6~~ | ~~**A literal "0" is drawn for five outcomes** (ricochet, non-pen, near-pen, shatter, track stack) — `hit_indicator.rs` pushed `damage_hp` unconditionally~~ — **CLOSED (2026-09-02, `fix/sight-feedback-a3-a5-a6`)**: `hit_label` prints the damage when there was any and the OUTCOME in a word when there was none — RICOCHET / SHATTER / TRACKED / NO PEN — in the outcome colour, smaller than a damage number (a lesson, not a score); locks `a_zero_damage_hit_prints_its_outcome_in_a_word`, `a_zero_never_becomes_a_number_and_damage_never_becomes_a_word`, `the_word_is_drawn_where_the_zero_used_to_be`. ABSORBED belongs to S5 (our own hull) | `client/src/hit_indicator.rs`, `ui_strings.rs` | W1 | the outcome word replaces the number for zero-damage outcomes; "0" never rendered — done |
| A7 | **The scope hides the enemy.** The vignette window is 0.94 in y-units, so horizontally it reaches 0.529 of clip-x — ~61 % of a 16:9 frame at α 0.87 near-black; no enemy outline, highlight or nameplate exists when spotted (spotting is cull-only); at 18° FOV a T-54 at 300 m is 3.4 % of frame height; and **no sniper frame is ever measured** — the D31 readability lock sees an 8 m third-person close-up | `hud/scope_overlay.rs:17,23`, `app/aim.rs:335`, `client/tests/look_goldens.rs:759-830` | W1 | a `prokhorovka_sniper_contact` review view (8° FOV, target at 300 m) under the D31 local-contrast floor; the window measured in the frame's aspect; a spotted-enemy edge treatment measured on that view |
| A8 | **The trajectory reads wrong for visual reasons.** Gravity 12.0 m/s² (22 % over Earth, undocumented); the tracer is a 22 ms streak that vanishes for 95 % of a 400 m flight; no shell follow. The solver itself is right (WoT-style aim-at-point integrating the sim's own step) | `game_core/src/math/mod.rs:17`, `fx/tracer.rs:15,63-67` | W1 | 9.81 (the decision above); a persistent shell billboard for the whole flight with the true sim path; replays re-pinned |

### Z — Honest Steel

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| Z1 | **Collapse is a state swap.** `sync_cover_destruction` diffs phase bytes and fires one `impact_burst(Cover)` + `track_dust` at the object's centre — 5 sparks and 6 puffs — then swaps the baked mesh. An 18 m tenement dies in eleven particles | `client/src/app/ingest.rs:303-345`, `fx/impacts.rs:122-127` | W1 | a staged client sequence scaled to the footprint (dust curtain, a settle beat, falling chunk cards, an audio hit through `audio`); lock: FX extent and count scale with footprint area; `tenement_intact`/`tenement_rubble` gain a mid-collapse probe frame |
| Z2 | **Cover damage is two constants**: HE 300, everything else 80. A 57 mm and a 152 mm fell a barn in the same number of shells | `sim/src/state.rs:92-97` | W1 | damage scales with shell energy or caliber through one function; lock: the bigger gun needs fewer shots; replay fixtures re-pinned deliberately |
| Z3 | **Three of five maps are static battlefields.** Destructible objects: Ostrogorsk 39, Prokhorovka 14, Mazurski 9. `Wreck`, `RailCover`, `Crag` are indestructible forever; all 12 `SceneryKind` variants never change | `terrain/src/battlefield.rs:128-147`, `terrain/src/scenery.rs:14-42` | W1 | each map dossier states its destructible count and the map report gates it (floor = today, target set per map by its dossier); `Wreck` becomes damageable |
| Z4 | **No destruction register; the roadmap says DONE** | `docs/ROADMAP.md` | W1 (first PR) | the roadmap line says "mechanics done, the theatre is this register" — done |

### P — armour

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| P1 | **Parity is two spot checks.** Metal-outside-armour and armour-proud-of-metal are locked at ≤ 10 mm for the T-54 turret and ≤ 1 mm for the Tiger I turret; the other six vehicles have "≥ 4 vertices on the plane" tests; hulls, sides and decks are unmeasured everywhere. "What blocks the shell blocks the eye" is doctrine plus two numbers | `t54_turret_armor_lock.rs:35,84`, `tiger_i_benchmark.rs:115`, `panther_ii_benchmark.rs:44` | W3 (per vehicle) | a fleet-wide, hull-inclusive parity metric per zone as a gate for every roster vehicle — floor at today's value, target 10 mm — landing with each K3 migration |
| P2 | **Thickness is a six-bucket facet.** Mantlet, deck, track and skirt plates are formula-derived (`zone.rs:113-123`); only the T-54 authors its roof, lower front and taper | `game_core/src/armor/zone.rs`, `modules/catalog_*.rs` | W3 (per vehicle) | per-plate authored millimetres in the blueprint from the dossier, locked per vehicle |
| P3 | **No turret ring, no hatch zone, no interior or rack armour, no stowage or fuel as spaced** | `armor/vehicle_volumes.rs` | W3 | `ArmorZone` appended (ring, hatch) with real patches; external stowage as screens where the dossier says so |

### F — flora

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| ~~F1~~ | ~~**The cones.** The map-border backdrop ring — 450 trees per map, 40–380 m outside the red line, on all five maps — is built from the pre-Drzewa six-segment frustum kit (`push_scenery_tree_far`: three stacked hex frusta for Oak, a 0.95→0.12 spike for Poplar) at a 3.0–3.4× far scale, so the ring's trees stand 25–38 m tall — bigger than any tree inside the map — and read as hexagonal Christmas trees on every horizon. Drzewa 3.0 never touched this path~~ — **CLOSED (2026-09-01, `feat/backdrop-ring-drzewa`)**: the ring stands on every species' impostor — the same crossed quads over the same atlas sprite the ladder draws past 150 m (`foliage::push_impostor_quads`, one expansion for both routes); the frustum kit is deleted; the mix is `HorizonSpec::flora` per map (pine on Orliny and Mazurski, willow along Bystra, poplar around Ostrogorsk); scale 1.0–1.3 of a mature individual, locked on the vertices (`no_ring_tree_towers_over_its_species`), species locked per map (`the_ring_grows_the_species_its_horizon_names_and_never_one_alone`); the atlas grew 1024×512 → 2048×1024 (5.6 → 22.4 MB, re-locked to the byte) | `scene_build/src/backdrop.rs:43-75`, `scene_build/src/foliage.rs:38-46,197-257`; `target/orliny_pine_belt.png`, `target/bystra_treeline.png` | W1 | the ring is grown from Drzewa 3.0's impostor rung per species; the frustum kit has zero call sites and is deleted; lock: no backdrop tree taller than 1.3× its species' envelope; the ring's species mix follows the map's climate; perf measured on the MX330 (A→B→A) |
| F2 | **Monoculture.** Inside every map: Oak and Bush only. Poplar, Willow, FruitTree and Pine are fully grown (PRs #626–#628) and placed nowhere. Orliny's `pine_belt` view has no pine; Bystra's river has no willow | all five map blueprints | W1 | every map places ≥ 3 species; a report gate: no species exceeds 70 % of a map's trees; the per-map dossier carries its species table |
| F3 | **The shelterbelt is boxes on sticks.** `TreeLine` is a szpaler of slabs on stick trunks | `scene_build/src/battlefield.rs:950` | W1 | built from Drzewa 3.0's Mid rung; blocking AABB bit-identical before/after (honesty doctrine, the W2 art-direction rule) |
| F4 | **Density.** 12–22 oaks per square-kilometre map | map blueprints | W4 | per-map tree floor from the dossier, perf measured |
| F5 | **No variety.** No dead trees, saplings, stumps, logs or hedges as scenery (stumps and logs exist only as destruction wreckage); one canopy colour per species (`foliage.rs:183-192`); no seasonal or climate tint; statics-baked species get no wind | `world_forge/src/tree/mod.rs`, `foliage.rs:108` | W4 | `SceneryKind` appended (dead tree, stump, log, hedge); per-map canopy tint in the map blueprint; wind on every placed species |
| F6 | **No open flora row anywhere**; D5/D12/D13/D14 all closed; the backdrop comment claims kilometres at 40 m | `docs/art-direction-program.md`, `backdrop.rs:39-42` | W1 (first PR) | the comment is rewritten; flora debt lives here |

### S — the shot, and the hit

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| S1 | **No dynamic light.** The renderer has the sun and nothing else; a 100 mm gun at dusk lights neither its glacis nor the ground, and an impact lights nothing. This is the single largest reason a shot reads as a decal | `renderer_api` (no point-light concept), `fx/emitters.rs:29-56` | W1 | one muzzle light and one impact light in terrain, scene and vehicle shading, caliber-scaled in radius and energy; lock: a dusk probe frame's ground luminance under the muzzle rises by a measured floor; cost measured on the MX330 (A→B→A) before the budget is touched |
| S2 | **Sniper mode gets nothing.** The camera kick returns early when scoped, and most aimed shots are scoped | `client/src/camera/smoothing.rs:129-136` | W1 | a rotational tremor in sniper (no translation), caliber-scaled, decaying within a locked window; `camera_feel.rs` updated from "leaves sniper rigid" to the new promise |
| S3 | **Nothing is caliber-scaled but audio and tracer width.** Flash 1.0→1.6 m, smoke 8 particles, dust ring 10, recoil stroke 12 m/s, hull impulse 0.16 rad/s, camera kick 0.9/0.5 m/s — all constants, identical for 75 mm and 128 mm | `fx/emitters.rs`, `engine/src/components.rs:146-174`, `engine/src/attitude.rs:35`, `smoothing.rs:71-72` | W1 | every channel derives from one recoil momentum (`mass_kg × muzzle_velocity_mps`, both already on `ShellSpec`) through one function; lock: 128 mm exceeds 75 mm on every channel |
| S4 | **No mechanical layer.** No breech clack at fire, no casing, no recoil-cycle metal, no dust shaken off the hull — one synthesized blast with nothing before or after it | `audio/src/voices/cannon.rs` | W1 | breech, casing and cycle voices in `audio` (pure DSP), hull dust on fire; mixer locks |
| S5 | **Being hit is mute in the body.** Camera shudder only; `HullAttitude` has `fire_impulse` and no `hit_impulse`; no "armour held" callout although the exactly-once lane already carries the absorbed-impact truth; no screen effect | `engine/src/attitude.rs`, `hud/reticle_readouts.rs` | W1 | an incoming impulse on the hull spring from hit direction and energy; an absorbed-impact callout fed by the personal-truth lane; locks |
| S6 | **The roadmap lists fire feel DONE**, so it was never audited | `docs/ROADMAP.md:21` | W1 (first PR) | reworded — done |
| S7 | **The number out-pixels the world 7:1 and outlasts it 8:1.** At 300 m in the scope a T-54 is 64 px wide; a bounce's whole world answer is ~60 px² of faint additive spark for 0.3 s; the damage number is 42 × 23 px of opaque saturated ink for 2.5 s (~400 px²) | `fx/impacts.rs:81-95,156-198`, `hit_indicator.rs:10,101-129` | W1 | the number demoted (size ~0.045 em, TTL ≤ 1.2 s); lock: number ink ≤ 2× the lit area of the penetration FX |
| S8 | **The target does not move when hit.** No hit impulse exists for any hull (`fire_impulse` is the only one); no turret jerk | `engine/src/attitude.rs:197-200`, `sync_cues.rs:50-60` | W1 | `hit_impulse(direction, damage_fraction)` beside `fire_impulse`, driven client-side from the replicated `DamageEvent`; lock: a 300-HP pen deflects the target ≥ the fire impulse's 0.8° |
| S9 | **The penetration signature is one size and non-pens have no answer.** One 0.09 s flash + 7 grey puffs for every pen; no spall cloud on a bounce; HE on armour takes the same path as AP (no fireball, no smoke ring) because `shell_step.rs:134,175` emits no `ShellImpact` for tank hits | `fx/impacts.rs:200-239`, `app/ingest.rs:175`, `sim/src/shell_step.rs` | W1 | pen flash scaled by damage (lock: ≥ 4× the bounce fan's area); a bounce spawns ≥ 1 occluding row; a `ShellImpact` for hull hits so HE gets its blast |
| S10 | **Sparks detach from a crossing hull.** Tanks render interpolated one snapshot (50 ms) behind; the FX spawn at the world hit point — up to 0.5 m ≈ 10 px off at 300 m; the breach hole (reliable lane) can open 50 ms before the sparks | `client/src/render_state.rs:36,99-133`, `session.rs:686-717` | W1 | impact FX seated in the target's local frame (reuse `decals.rs::pose_of`) and rendered at the interpolated pose; lock: centroid ≤ 5 cm from the seated decal at 10 m/s crossing |
| S11 | **The armour clang is inaudible at range.** Gain 0.8 × 18/318 = 0.045 at 300 m (−27 dB), air low-pass to 3.3 kHz, 0.87 s delay — under the engine bed the pen thunk and the ricochet whine are one sound | `audio/src/voices/impact.rs`, `audio/src/mixer.rs:167`, `spatial.rs:56-80` | W1 | `ArmorStruck` gets its own reference distance; lock: pen vs ricochet distinguishable by RMS/ZCR at 300 m |
| S12 | **A landed pen gives the shooter nothing but pixels.** No camera micro-kick, no voice or ribbon; only the shooter's own hit shakes the camera (`ingest.rs:79-83`) | `client/src/app/ingest.rs` | W1 | a shooter micro-kick on a landed pen, scaled by damage fraction; lock in `camera_feel.rs` |

### Q — the frame

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| Q1 | **Three whole-mesh re-uploads on the render thread.** Any HE round on terrain: the entire ground mesh via `create_buffer_init` (a fresh GPU allocation, never scaled to the dirty span, `ground.rs:287-308`); the whole card meadow when the crater touches it (`dressing.rs:42-53`); any cover phase step or scar: the worker re-bakes only dirty buckets, then the harvest reassembles all `STATICS_BUCKET_COUNT` buckets and uploads the lot (`terrain.rs:22-36`). The J fixes bounded the *frequency*; the per-event cost is still the whole mesh | `client/src/app/render.rs:84-96,177-216`, `renderer_wgpu/src/scene_renderer/{ground,dressing,terrain}.rs` | Q lane | persistent capacity-reserved buffers with `write_buffer` over the dirty vertex span; statics as per-bucket buffers so a bucket re-bake uploads that bucket; lock: `battle_age_cost` prints uploaded MiB per event and it scales with the change, not the mesh |
| Q2 | **No hitch instrument on a live battle.** `perf_capture` has p50/p95/p99/max on a static, sim-free path; `battle_age_cost` sizes the ground row but not the statics row; nothing measures "the longest frame and what ran in it" under sim + render + audio + network together. Found closing F1: `perf_capture` and twelve more probes never bound the leaf atlas, so from Drzewa 3.0 PR6 every card and impostor in their frames was an opaque white quad — the 2026-08-09 numbers were taken with that instrument. Fixed in F1 (`bind_battle_foliage_atlas` + the source lock `probe_foliage_atlas.rs`) | `client/examples/probe/perf_capture.rs:613,672`, `battle_age_cost.rs:237-285` | Q lane (first) | `battle_age_cost` extended: the statics row, and a live `battle_host` session firing HE on a schedule with the wall-clock-minus-fence percentile method — p99/max on the frame that harvests a real crater or collapse |
| Q3 | **The last honest number is over budget and 23 days stale.** 2026-08-09, MX330, 1×: full scene 11.95 ms, full + 7v7 15.38 ms, 7v7 with gear forced NEAR 17.88 ms — 1.21 ms over before any hitch; no combined worst case (dense trees + close 7v7 + a hitch) has ever been measured; the release probe binary predates ~100 commits | memory `battle-frame-budget-2026-08`, `docs/ROADMAP.md:48-49` | Q lane | a fresh A→B→A on current master as the lane's first PR; the deficit assigned per item with a measurement, never fleet-wide; every renderer row in this register (S1, F1, F4, N2, N6, H5, B3) lands with its own delta |
| Q4 | **A blocking `std::sync::Mutex` on the real-time audio thread**, shared with the main thread's `with_engine`; the docstring claims it never blocks the callback's cadence, which nothing enforces | `client/src/audio_out.rs:6,48,89-93` | Q lane | a lock-free parameter mailbox (triple buffer or atomics); lock: the audio callback path contains no lock |

### R — roles

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| R1 | **No concealment stat.** `spotting.rs:72-77` returns a fleet-wide binary; a Jagdtiger and a T-34-85 hide identically, and view range has two values (400 × 5, 440 × 3) | `sim/src/spotting.rs`, `tank.rs:132` | W2 | a per-vehicle concealment factor and an authored view range from the dossier; lock: the bigger silhouette is seen farther, stationary |
| R2 | **One dispersion factor.** Movement, hull traverse and turret traverse share `movement_bloom_mrad` times fleet constants 0.35 / 0.25; no gun can be "bad on the move, fine on the turret" | `sim/src/aim_dispersion.rs:41-43` | W2 | three authored factors per gun; lock |
| R3 | **No ground pressure.** `SuspensionModule` has no track width; terrain resistance is per-material only, so a 32 t T-34 and a 75 t Jagdtiger handle mud identically | `terrain/src/ground.rs:87-99`, `physics/src/contact.rs:45` | W2 | track width on the module; pressure from mass and contact patch; soft-ground resistance scales with it; lock: the heavy hull is slower on mud and equal on cobble |
| R4 | Crew proficiency pinned at 1.0 for all | `crew.rs:54` | decided | stays — proof, never power |

### U — interface and garage

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| U1 | **`ui_kit` is a triangle emitter, not a toolkit.** 1 097 lines of `push_x(&mut Vec<HudVertex>, …)` in clip space with a manual aspect; no layout containers, anchors, padding, scissor, DPI (nothing reads `scale_factor`), focus, hover or press state, text wrapping, second size class, one font at one weight | `crates/ui/ui_kit/src/`, `font/bake.rs:11-16` | U (parallel) | a layout layer (rect tree, edge anchors, row/column with padding, measured text, a `Ui` context carrying aspect, DPI and scissor), theme size classes and a second weight, a hover/press/focus machine, `set_scissor_rect` in the HUD pass |
| U2 | **245 vertex-equality tests** across `client/src/hud` and `app/garage` are the real cost of any redesign | `hud/reticle_overlay_tests.rs` (30), `garage/actions.rs` (26), … | U (first) | a semantic draw list keyed by element name; tests query it; the vertex-equality count ratchets to zero as elements migrate |
| U3 | **ASCII only.** The atlas bakes 0x20–0x7E; unknown glyphs are silently skipped; `ui_strings.rs` forbids non-ASCII | `ui_kit/src/font/bake.rs:42`, `font/layout.rs:225` | U | Latin Extended-A baked; the ASCII rule removed; lock: an unknown glyph renders visibly, never skips; a Polish string in a golden |
| U4 | **22 HUD elements on hard-coded floats**; absent entirely: kill feed, team lists, scoreboard, ping | `client/src/hud.rs:174-262` and `hud/*.rs` | U | the HUD is a layout description; the missing elements exist; the sight locks and `reticle_strip` are preserved unchanged |
| U5 | **No product shell**: no settings, keybinds, battle results, lobby, localization; Escape offers no exit from a cold-booted garage (`garage/mod.rs:255-259`) | `docs/ROADMAP.md:88-89`, `docs/hala-4-program/plan.md:270-274` (W2–W6) | U | the screens exist with goldens; `escape_always_offers_a_way_out` |
| U6 | **The stat column is nine anonymous numbers** — no labels (the test concedes it, `stats.rs:136-137`), no bars although `push_bar` exists, no compare; every plate is the same `PANEL` at 0.86 alpha, so the crew column carries the weight of the nine numbers that decide the fight | `app/garage/panels/stats.rs:25-29`, `layout.rs:63-69` | U | labelled rows with bars against the roster min/max; a plate hierarchy in the theme; lock: `every_stat_row_prints_its_own_label`; `garage_screen` re-blessed |
| U7 | **"Which tank am I in and why" is unanswerable.** `VehicleClass::label()` exists and is printed only in the tech tree; the nameplate is tier + nation | `panels/nameplate.rs:31`, `stats.rs:20-22`, `techtree.rs:69-77` | U | the nameplate names the class and role; lock |
| U8 | **Ammo is illegible and unexplained.** Designations at 0.016 under the screen's own 0.022 floor (`inspector_legend.rs:22-24` vs `loadout.rs:54-55`); no pen/damage per shell; the rack creed unexplained | `panels/loadout.rs:49-74` | U | pen/damage per slot, a one-line rack rationale; lock: `no_garage_string_renders_below_the_legibility_floor` |
| U9 | **No press state, no tooltips, no keybind legend.** One white-8 % hover wash for the whole screen; 14 keys bound and only `[R] REPAIR` printed | `garage/overlay.rs:57-59`, `garage/actions.rs:176-247` | U | three-state controls (`PRESSED` token, `every_clickable_has_three_states`), tooltips, a hint strip with its golden |
| U10 | **BACK wears the commit red** (`techtree.rs:98` reuses `BATTLE`) and the SIGNAL red is outside the palette (hala-4 W6) | `panels/techtree.rs:98`, `docs/art-direction-program.md` | U | `signal_red_is_only_worn_by_commit` |
| U11 | **Clicking a module on the 3D tank does nothing** — `hit_test_hangar` knows strip rects only; the turret cannot be rotated | `garage/overlay.rs:157-209` | U | hero hit-testing opens the slot; turret rotation on drag |

### B — buildings

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| B1 | **The silhouette is flush-cut.** No geometry crosses the collision AABB: roofs end at `half_width` (`push_gable:1790`), no eaves overhang, ridge, hip or valley pieces, no chimney, dormer, gutter or downpipe (grep = 0) — every skyline is a rectangle with a triangle on it and nothing casts an overhang shadow | `world_forge/src/building.rs:55-138,1790`; `target/ostrogorsk_avenue.png` | W5 | every style's roof projects ≥ 0.25 m past the wall with eave, ridge and chimney parts; a `building_views` review example at 15/40/150 m per style intact + damaged |
| B2 | **No ground connection.** `append_building` seats the mesh at `center.y − half.y` on one sampled point; no footing course, dirt apron or grass at the wall — the buildings sit on the lawn like game pieces | `scene_build/src/battlefield.rs:1355-1440`; `target/bystra_town_lane.png` | W5 | a footing course meeting the sampled ground along the perimeter, a dirt apron decal, grass at the wall; lock: `a_building_meets_its_ground` sampling the seam |
| B3 | **Every building is unique merged triangles** in the 1 000 m static buffer — a mullion is fresh geometry each time | `battlefield.rs::append_building`, `scene_resources.rs` | W5 | the kit-of-parts decision above; lock: the town's static-mesh triangle count drops and the frame delta is recorded |
| B4 | **Walls are one flat albedo** with the shared detail octave: no brick or plaster coursing, no dirt streaking, no AO in the reveals — pierced windows read as painted past 40 m | `building.rs::building_palette:1454`, `scene.wgsl` | W5 | coursing and streak terms per wall role, AO in reveals; lock: an albedo-variance floor per facade in the review view |
| B5 | **A ruin is a heap of 7–10 slabs** with no relation to the intact mass; no standing walls, no visible floors | `building.rs:1520` | W5 | a ruined form keeping ≥ 2 wall planes and a floor slab, with its own honesty test |
| B6 | **Repetition.** 7 styles × 4 wall colours × 3 roof colours, all detached rectangles with the ridge along the long axis; the Ostrogorsk square is six near-identical white blocks | `building.rs:55-138`; `target/ostrogorsk_square.png` | W5 | the grammar produces L-shapes, annexes, terrace rows, yards and shop fronts; lock: a footprint-shape histogram per town with a floor on non-rectangles |

### T — terrain

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| T1 | **The 5 m grid is the root of two other rows.** It turns a 12 m hill into a tent (G7) and puts 2–4 velocity creases per second under the camera (C4). 1 m map-wide is refused (25× samples; the ground bake is already 362 ms of a 517 ms map swap) | all five blueprints `cell_m: 5.0`, `terrain/src/heightmap.rs` | W5 | a 2.5 m ring near the player, chunk-streamed, measured (map-swap ms, frame ms, G7's crease count); map-wide 2.5 m only with that measurement |
| T2 | **No erosion.** Heightfields are authored by gesture; hills have no erosion lines, no drainage, no talus | `map_forge` editor, `orliny_talus.png` | W5 | a hydraulic-erosion pass as an editor bake step (zero runtime cost); goldens re-blessed with a slope histogram lock |
| T3 | **The ground tiles to the horizon.** One woven noise carpet at every distance, no macro colour/normal variation at 100–400 m | `terrain.wgsl`, `target/prokhorovka_clear_afternoon.png` | W4 | one macro variation fetch; lock: a variance/FFT metric on the clear-afternoon golden |
| T4 | **Cliffs and slopes are underused.** `GroundClassifier` has slope rules and a `steep` layer but no triplanar cliff material | `terrain/src/ground.rs`, `terrain.wgsl` | W4 | triplanar cliff keyed on slope; review views `orliny_talus`, `ostrogorsk_canyon` |
| T5 | **Roads are painted vertex colour** with a baked camber normal: no edges, kerbs or decals; tracks and craters are the only decals | `terrain_maps.rs:15`, `battlefield.rs:1506-1559` | W5 | road edge geometry and decals through the instanced path; lock: draw-call count and the O1 meadow metric |

### N — sky and weather

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| N1 | **The golden evening fails the policy** — lavender-milk top to bottom, no warm horizon band, no visible sun, a flat khaki field with no long shadows; the profile's god rays 0.15 and bloom are gated off on the canonical tier, so none of the golden-hour machinery reaches the frame. This is D4's proof | `target/prokhorovka_golden_evening.png`, `lighting.rs`, `lighting_quality.rs::canonical()` | W4 | a golden asserting the lower-quartile luma ≤ 0.28 and horizon R/B ≥ 1.6 in the played band |
| N2 | **Clouds have no volume.** A two-tone lerp `mix(shade, lit, dot(dir,sun)·0.5+0.5)`: no Beer–Lambert, powder, silver lining, self-shadow or thickness; the overcast lid is a structureless gradient over 45 % of the frame | `sky.wgsl:159-163`, `target/prokhorovka_overcast.png` | W4 | the 2D cloud lighting model (3–4 marched taps where cloud > 0); lock: cloud-region max/min luma ≥ 2.0 on the clear-afternoon golden with the shipped 3-octave mask; Δ ≤ 0.8 ms |
| N3 | **The drawn cloud field and the shadow cloud field are different noises** (Hoskins FBM with a ridged octave on the dome vs periodic splitmix value noise on the ground, `shadow_common.wgsl:47-48` admits "not pixel-exact") — a shadow can land under open sky | `sky.wgsl:37-74,132-137`, `cloud_map.rs:24-70` | W4 | one field: the dome samples the baked tile (or both evaluate one function); lock: shadow coverage under a drawn cloud ≥ 0.9 |
| N4 | **Weather is frozen on four of five maps.** `static_program = map != BystraValley || seed == 0`; the sun travels 2.5° and only there | `weather_timeline.rs:56,117-125` | W4 | a program per map; the fairness lock extended over the timeline, not the static look |
| N5 | **Rain does not wet the world.** Geometry streaks with a private hard-coded wind (`rain.wgsl:14`), no splash, no puddle ripples, wetness darkening at 0.08 gloss — the rain frame reads as dry grass with lines over it | `rain_pipeline.rs:15`, `terrain.wgsl:133-140`, `target/bystra_town_rain.png` | W4 | screen-space streaks + a real wet response (albedo ×0.72 on soaked ground, Fresnel sheen on roads, puddle ripples); lock: ground luma under rain ≤ 0.75× dry on the Bystra golden |
| N6 | **The sky is a two-stop gradient** `mix(horizon, zenith, pow(up, 0.42))` with per-profile constants; no sun colour ladder by elevation; the reflection curve (`sqrt`) disagrees with the dome's | `sky.wgsl:93-94`, `lighting_common.wgsl:77-80` | W4 | the scattering-LUT decision; lock: a sun-elevation sweep asserting monotone warmth as elevation falls and zenith saturation ≥ horizon saturation; Δ ≤ 0.15 ms |
| N7 | **God rays and bloom exist and never ship** (`GOD_RAYS` not in `canonical().shader_detail`, `bloom_mips: 0`) | `post.wgsl:37-63`, `lighting_quality.rs` | W4 (after N2) | the buy-back protocol (interleaved cold-GPU A/B, Δ ≤ 0.5 ms on the evening view) decides each; rays through a structured sky pay, through a flat lid do not |
| N8 | **Three winds.** Grass and foliage read the storm heading; rain has its own constant; smoke, dust and FX have none | `meadow_common.wgsl:50-52`, `rain.wgsl:14`, `fx.wgsl` | W4 | one `wind_params` lane; lock: no shader declares a private wind constant (mirrors `every_sunlit_pass_takes_the_cloud_shade`) |

### H — water

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| H1 | **Every hull drowns at 1.5 m.** `DROWN_DEPTH_M` is fleet-wide; no fording depth exists in `TankSpec` or the dossiers (real: T-54 1.4 m, Tiger I 1.6 m); `BOT_DEEP_WATER_M = 1.1` is a second constant | `sim/src/drowning.rs:14-21`, `battle_host/src/bot_routes.rs:121` | W2 | `ford_depth_m` per vehicle from the dossier; drowning and the bots' line derive from it; lock per vehicle |
| H2 | **Drowning is silent until the engine is dead.** `submerged_s` is never replicated; no HUD countdown, no bubble or gurgle; the first signal is an engine-dead `DamageEvent` at 2 s, death ≈ 6 s | `sim/src/tank_state.rs:82`, `net` (absent) | W2 | `submerged_s` on the wire (append-only), the rack-countdown widget mirrored, audio at t = 0 |
| H3 | **Bots ride down slick banks** (register H1 of the tracks program: 2.107 m past the line) — on a descending wet bank reverse thrust < gravity + drag, the escape freezes heading and slides | `terrain/src/ground.rs:66-83`, `bot_routes.rs:291` | W2 | the escape steers along the depth contour (bearing scored by the depth gradient); lock: no bot exceeds its ford depth over all soak seeds |
| H4 | **HE on water is undamped** — a full `burst_he_splash` at the waterline | `sim/src/shell_step.rs:144` | W2 | splash attenuated by depth at the burst; lock |
| H5 | **The river reads as plastic.** Sky-only Fresnel (a mirror with nothing in it under overcast), both depth-tint endpoints desaturated (`river_mute` 0.74), a hard polygon shoreline, a stray concentric ripple, a skirt seam, and at 300 m the fog blend to `sky_horizon_rgb` erases the channel entirely; the bridge floats with no reflection or waterline | `shaders/water.wgsl:51-62,155,172`, `target/bystra_bridge.png`, `target/bystra_panorama.png` | W4 | half-resolution SSR for the water pass (measured), a shore-foam band, the aerial blend capped for water so the channel survives 300 m, the ripple and seam fixed; lock: a `bystra_bridge` golden with the bridge's reflection present |
| H6 | **No wet hull, no rain ripples.** Fording leaves no darkening or drip; puddles are gloss only | `motion_fx.rs:70-172`, `terrain.wgsl:135-140` | W4 | wet-hull gloss/darkening decay after fording with drip particles; rain ripples on water and puddles |

### O — the picture

The light's register stays in [art-direction-program.md](art-direction-program.md); this program
feeds it rather than duplicating it. D8 (contact AO on one vehicle), D9 (dirt lane never populated),
D15 (nothing to look at up close), D17 (pastel showcase) are W3 outputs — they close with K3 and
K6. D4 (no dark mass on the steppe) closes with N1 and B1; D18 (Orliny's borrowed light) with N4.

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| O1 | **The card meadow reads as a wavy moiré** in the mid-field band, most visible at grazing light | `target/flora_lineup.png`, `target/prokhorovka_evening_contact.png` | W4 | a metric derived from a frame judged good (look-metric-validation rule), then a floor |

## Wave plan

**W1 — Widok.** Everything the player sees first, none of it touching the authoritative hull.
Order: F1 (the cones: one PR, the biggest area of any frame), the register rewording (F6/Z4/S6/A2),
A1 (one penetration resolver — the "shoot for 0" fix), A3–A6, A8 (gravity + the visible shell),
A7 (the sniper review view, then the scope), F2, F3, Z1–Z3, S3 (the momentum function), S1 (the
lights, with their perf sandwich), S2, S4, S5, S7–S12. Gate: every W1 row closed; the frustum kit
gone; the lights' cost recorded.

**W2 — Jazda.** The authoritative hull and everything that reads it. G7 first (the sprung hull,
with G1's one ground truth folded in), then G4, G3, G2 (the wire), G5, G6, K10, then the camera
C1, C2, C3 (re-measured on the sprung hull), C4, then R1–R3 and the water sim H1–H4. Gate: the
crest-walk continuity lock, the per-vehicle wallow lock, `mobility_baseline` byte-identical,
replays re-pinned once for the wire bump.

**W3 — Kuźnia 2.0.** K7 first (the documents), then the seam (K1, K2), then the benchmark to the
bar (K0, K9, K11–K15), then seven vehicle migrations — one PR each, with its dossier, its P1 parity
gate, its P2 thicknesses and its K0 overlay — then the three capabilities the kernels lack (K4, K5,
K6), each proven on the T-54 and rolled to the fleet in the same wave. P3 closes with the last
migration. Gate: the K3 inventory gate and the K0 overlay green for every roster vehicle;
D8/D9/D15/D17 closed in the light's register.

**W4 — Obraz.** The sky and the air first (N6, N2, N3, N1, N4, N8, N5, N7), then the ground's
distance (T3, T4), the water's look (H5, H6), the flora's density and variety (F4, F5), O1, D18.
Gate: every look golden at its target; each renderer row with its own measured delta.

**W5 — Miasto i Ziemia.** Buildings and the terrain's structure: B3 first (the kit and the
instanced path — it pays for everything after it), then B1, B2, B4, B5, B6; T2 (erosion in the
editor), T1 (the 2.5 m ring, measured), T5. Gate: `building_views` per style at three distances,
the footprint-shape histogram, the map-swap and frame measurements.

**Q — the frame**, a lane from day one, in parallel with W1: Q2 (the instrument) first, then Q3
(the fresh number and the per-item deficit), Q1 (the uploads), Q4. Every renderer row in every wave
lands with the lane's measurement.

**U — the interface**, in parallel from day one on files no other wave touches: U2 and U1 first
(the foundation), then U3, U6–U11 (the garage), U4 (the HUD), U5 (the shell).

A wave is done when every row in it is closed, its locks are in the ratchet, and the roadmap's
"DONE" lines it touched are reworded. A row closes with a number or a frame, never with a sentence.

## Verification

The merge gate is `scripts/verify.ps1` (see `CLAUDE.md`). In addition, for this program:

- Any renderer change (S1, F1, F4, N2, N6, H5, B3) lands with an MX330 A→B→A measurement (single
  runs vary by 3–5 ms with thermals); the budget moves per item, never fleet-wide.
- Any sim number that moves (Z2, G2, G7, A8, H1–H4, R1–R3) re-pins the replay fixtures in the same
  PR and says why; `mobility_baseline.rs` rows stay byte-identical under G7.
- Any wire change (G2, G7, H2) is append-only and bumps `PROTOCOL_VERSION`.
- Goldens are blessed deliberately, in the PR that moves them, with the before/after in the message.
- Each vehicle migration (K3) ends with the close-up review under the model-logic bar and the K0
  overlay, not with a triangle count.
