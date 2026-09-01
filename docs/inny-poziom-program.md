# Inny Poziom — The Second Pass

Approved 2026-09-01. The owner named ten things that read as unfinished — the geometry kernels and
the fleet, the picture, the physics, Honest Steel, the armour, the tracks, the flora, the shot, the
fleet's identity, the HUD — and asked for all of them to be taken "to a completely different level".
Eight read-only audits of the code and a look at the review frames in `target/` answered with one
diagnosis, and this document is that diagnosis turned into a register and a wave plan.
[art-direction-program.md](art-direction-program.md) is the model: the register below names the
evidence, the wave, and the lock that closes each row. When the register is empty this document
becomes history and the policies it edits stand alone.

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
   kilometres read identically" while the ring starts 40 m past the border.

## The three rules this program is built on

1. **Measure the eye, not the model.** Every register row names a frame or a number, and the lock
   that closes it. No row closes on a description. Where no metric exists yet, the row says so and
   the first PR of its wave derives one from a frame judged good — never from the broken frame
   (the lesson of art-direction D31).
2. **Rollout before invention.** A capability counts as landed when it stands on every vehicle,
   map or species it applies to. No wave starts a new capability while the previous one is on
   one-of-N. This is the fix-as-rule rule at program scale.
3. **One truth per system.** Where the audit found N models of one thing — fifteen track models,
   three ground samplers, damage modelled three times, twenty-two sets of HUD floats — the wave that
   touches it leaves one owner and every consumer reading from it.

## Decisions taken with this program

- **No physics engine.** `crates/runtime/physics` is ~2 800 lines of deliberate custom code
  (SAT footprints, a Jacobi impulse solver, heightfield contact); `parry3d` has one call site with
  no production caller. rapier3d would replace the integrator, contacts and friction — the parts
  the repo already rebuilt on purpose (`docs/physics-policy.md`) — and would not touch a single
  item the owner complains about: belt geometry, sag, link-to-wheel wrap, ruts, per-wheel travel
  and belt scroll are presentation and stay hand-built. The replay-exact locks need cross-platform
  bit determinism; rapier's `enhanced-determinism` promises same-binary determinism only. The
  track work goes into fewer models, not a different engine.
- **No rigid debris.** Collapse is client theatre (particles, cards, dust, sound) over the
  replicated phase swap. Debris that could block a hull would have to be replicated, and the
  honesty doctrine says scenery never blocks gameplay.
- **No external UI crate for the client.** egui brings its own look and font stack against the
  one-look policy and a second pipeline against the single HUD draw call the MX330 budget is built
  on. It is allowed in `apps/editor` and `apps/tools`, where no look policy applies. The client's
  layout layer is written inside `ui_kit`.
- **Terrain and buildings are recorded, not rebuilt here** (K8). Terrain is a 5 m heightfield with
  vertex colour and no kernel; buildings and rocks are their own generators sharing only the vertex
  type with the kernels. Both need their own program with a perf measurement first; this one does
  not pretend to cover them.
- **Crew proficiency stays pinned at 1.0** (R4) — progression is proof, never power.

## Defect register

Columns: the defect, the evidence, the wave, and what closes it. IDs by area: K kernels and fleet,
G tracks and ground, Z destruction, P armour, F flora, S the shot, R roles, U interface, O picture.

### K — kernels and the fleet

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| K1 | **The recipe seam.** `vehicle_recipes` depends on `revolve` + `vehicle_geometry` only; `cast_loft`, `panel`, `solid`, `detail` are reachable solely from `vehicle_build`, whose 19 files are all `t54_*`. `mesh_source.rs:23-28` and `production_bake.rs:18-25` hard-code the T-54 as the only hybrid vehicle | `vehicle_recipes/Cargo.toml`, `vehicle_forge/src/mesh_source.rs` | W3 | one mesh-source rule with no per-kind match; every kernel is a legal dependency of a recipe (layer rule updated, `layer_rules.rs`) |
| K2 | **T-54 content lives inside a kernel crate.** `solid/src/t54.rs`, `t54_fittings.rs`, `t54_plates.rs` | `crates/kernels/solid/src/` | W3 | no vehicle-named file under `crates/kernels/`; a quality gate greps for it; the parts become a fleet part library in `vehicle_build` |
| K3 | **Seven vehicles are unbuilt.** Bake sizes: T-54 27 565, Tiger I 3 734, Panther II 3 000, Jagdtiger 2 864, Tiger II 2 828, IS-3 2 206, T-34-85 2 064, Centurion 1 960. A Centurion turret is a 5-station ellipse ring from a scale table shared by four vehicles (`turret_fittings.rs:481-521`); a mantlet is a 3-number tuple; there are no fenders | `shipped_cost`, `centurion.rs` (54 lines) vs `vehicle_build` (4 566 lines) | W3 | each roster vehicle carries every part class of the fleet part library its dossier lists (hull solids with armour angles, lofted turret, revolved mantlet, fenders, hatches, grab handles, tow cable, vision blocks, stowage, exhaust, lights, weld seams) — an inventory gate per vehicle — and passes the T-54's close-up review set (`closeup_probe`) under the model-logic bar |
| K4 | **No mesh boolean.** Every kernel is convex-only (`solid/convex.rs:5`, `sweep/lib.rs:77`, `panel/lib.rs:49`). Apertures are faked: the embrasure is a Gaussian dent (`t54_turret_loft.rs:49-56`), hatches are drums on a roof, the grille well is boxes. CSG exists in `sdf` and is unmeshed and dead (0 construction sites of `PartShape::Cast`) | `vehicle_build/src/part.rs:128,152` | W3 | a `cut` operator (convex tool subtracted from a solid or loft, watertight result) locked by a manifold + volume-delta test and used by at least one hatch and one embrasure on at least two vehicles; `sdf`/`sdf_mesh` either become that operator's path or are deleted |
| K5 | **No edge topology, so no fillets or rolled edges.** Chamfer only on axis-aligned boxes; the general pass was withdrawn (`solid/src/t54_fittings.rs:14-21`); `chamfered_prism` bevels 4 of 12 edges; loft caps are flat fans | `vehicle_geometry/src/builder.rs:24-58` | W3 | edge adjacency in the mesh contract; a fillet operator under the roundness law (segments from radius); locked by a facet-angle bound on filleted edges |
| K6 | **No per-part UV or bake.** One kernel authors `uv0` (`solid/convex.rs:161`); the rest are triplanar; `mesh.rs:78-80` claims otherwise. Normal/AO are per-role noise (`material_synthesis.rs`), and the synthesis is tuned to invisibility because it has nothing real to show | `vehicle_forge/src/artifact/material_synthesis.rs` | W3 | every kernel output carries authored UVs (a test over kernel outputs); per-part normal + AO bake with a golden; `vehicle-forge-policy.md` row corrected |
| K7 | **The documents lie.** 22 000 vs 29 000 cap; "UVs done"; M8 "in progress" with zero migrated; `hybrid.rs` describing deleted SDF; tracks "use `revolve`" while links are box prisms | `docs/vehicles/t-54.md:193-195`, `docs/vehicle-forge-policy.md:231-234`, `docs/procedural-kernel-program.md` | W3 (first PR) | a `roadmap_claims`-style anchor pins the T-54 budget; the stale paragraphs are rewritten to the code |
| K8 | **World objects are not on the kernels.** Buildings: an ad-hoc box/face generator, 7 styles, real roofs and pierced windows, zero chimneys/dormers/gutters. Rocks: `icosphere(1)` under two sine octaves. Terrain: 5 m heightfield, vertex colour | `world_forge/src/building.rs`, `rock.rs:140-150`, `scene_build/src/battlefield.rs:1506-1559` | recorded | out of scope by decision above; each gets its own program after W3, opened with a perf measurement |

### G — tracks and the ground

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| G1 | **Three ground samplers per hull per frame.** The support envelope (`physics/src/track_contact.rs`, rubble-aware, convex hull of stations), the probe cross (`physics/src/contact.rs:67-118`, four samples around the hull centre), and the client's per-wheel residual (`client/src/vehicle/render_frame.rs:300`, no rubble). They disagree exactly at crests and ditches | the three files | W2 | one function owns ground contact; the client reads station heights from the physics result; `render_frame.rs` no longer samples the heightfield; `running_gear_dynamics.rs:233` extended to assert rendered station heights == physics station heights on a crest |
| G2 | **Belt speed is re-derived client-side from pose deltas** (`engine/src/components.rs:117`); slip, a thrown belt's asymmetry and forced yaw never reach the visuals | `engine/src/components.rs:100-135` | W2 | per-side belt speed on the wire (append-only, protocol bump); the scroll reads it; locks: a thrown belt scrolls zero while the hull is dragged, a slipping belt scrolls faster than ground speed |
| G3 | **The FX path invents the gauge.** Ruts and the shed ribbon use `hitbox.half_width_m * 0.86` as the track centreline — 1.505 m on a T-54 against the real `half_gauge_x` 1.32 m. The same class of bug P2.1 fixed for belt scroll | `client/src/app/motion_fx.rs:93`, `client/src/vehicle/track_ribbon.rs:88` | W2 | one source (`half_gauge_x` from the blueprint) under `single_source_constants`; lock: rut centreline within 2 cm of the belt centreline |
| G4 | **Damage is modelled three times**: the HP pool (`game_core/src/track.rs`), drive scalars (`sim/src/drive_modules.rs`, `DAMAGED_SPEED_FLOOR`, `BROKEN_ONE_*`), sag tiers (`render_frame.rs:338`) | the three files | W2 | one `TrackCondition` derived from HP, consumed by drive and render |
| G5 | **Admitted and open**: gradeability really 0.42 against a documented 0.60; per-belt ground unimplemented (`contact.rs:105` samples material at the hull centre); 0.7 m float at every crest and cm-scale velocity kinks from the 5 m heightfield snap (`docs/battle-camera-policy.md:101-118`); low obstacles-as-ground deferred (P2.2) | `docs/contact-and-tracks-program.md` register | W2 | per-belt ground material; gradeability locked at the documented value or the document corrected to the measured one; a crest-walk test bounds float ≤ 0.10 m and the per-tick velocity kink |
| G6 | **Fifteen track models** in total (damage pools, drive status, belt-drive steering, support envelope, probe cross, vertical follow, authoritative attitude, footprint SAT, a dead parry query, blueprint `TrackShape`, geometry running gear, client suspension, belt scroll, presentation spring, ribbon/ruts/audio/HUD). Only one cross-check exists (`running_gear_dynamics.rs:233`) | audit 2026-09-01 | W2 | the count is not the target; the cross-checks are — every pair that must agree (stations, gauge, condition, speed) has a lock, and `parry_query.rs` is deleted |

### Z — Honest Steel

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| Z1 | **Collapse is a state swap.** `sync_cover_destruction` diffs phase bytes and fires one `impact_burst(Cover)` + `track_dust` at the object's centre — 5 sparks and 6 puffs — then swaps the baked mesh. An 18 m tenement dies in eleven particles | `client/src/app/ingest.rs:303-345`, `fx/impacts.rs:122-127` | W1 | a staged client sequence scaled to the footprint (dust curtain, a settle beat, falling chunk cards, an audio hit through `audio`); lock: FX extent and count scale with footprint area; `tenement_intact`/`tenement_rubble` gain a mid-collapse probe frame |
| Z2 | **Cover damage is two constants**: HE 300, everything else 80. A 57 mm and a 152 mm fell a barn in the same number of shells | `sim/src/state.rs:92-97` | W1 | damage scales with shell energy or caliber through one function; lock: the bigger gun needs fewer shots; replay fixtures re-pinned deliberately |
| Z3 | **Three of five maps are static battlefields.** Destructible objects: Ostrogorsk 39, Prokhorovka 14, Mazurski 9. `Wreck`, `RailCover`, `Crag` are indestructible forever; all 12 `SceneryKind` variants never change | `terrain/src/battlefield.rs:128-147`, `terrain/src/scenery.rs:14-42` | W1 | each map dossier states its destructible count and the map report gates it (floor = today, target set per map by its dossier); `Wreck` becomes damageable |
| Z4 | **No destruction register; the roadmap says DONE** | `docs/ROADMAP.md` | W1 (first PR) | the roadmap line says "mechanics done, the theatre is this register" |

### P — armour

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| P1 | **Parity is two spot checks.** Metal-outside-armour and armour-proud-of-metal are locked at ≤ 10 mm for the T-54 turret and ≤ 1 mm for the Tiger I turret; the other six vehicles have "≥ 4 vertices on the plane" tests; hulls, sides and decks are unmeasured everywhere. "What blocks the shell blocks the eye" is doctrine plus two numbers | `t54_turret_armor_lock.rs:35,84`, `tiger_i_benchmark.rs:115`, `panther_ii_benchmark.rs:44` | W3 (per vehicle) | a fleet-wide, hull-inclusive parity metric per zone as a gate for every roster vehicle — floor at today's value, target 10 mm — landing with each K3 migration |
| P2 | **Thickness is a six-bucket facet.** Mantlet, deck, track and skirt plates are formula-derived (`zone.rs:113-123`); only the T-54 authors its roof, lower front and taper | `game_core/src/armor/zone.rs`, `modules/catalog_*.rs` | W3 (per vehicle) | per-plate authored millimetres in the blueprint from the dossier, locked per vehicle |
| P3 | **No turret ring, no hatch zone, no interior or rack armour, no stowage or fuel as spaced** | `armor/vehicle_volumes.rs` | W3 | `ArmorZone` appended (ring, hatch) with real patches; external stowage as screens where the dossier says so |

### F — flora

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| F1 | **The cones.** The map-border backdrop ring — 450 trees per map, 40–380 m outside the red line, on all five maps — is built from the pre-Drzewa six-segment frustum kit (`push_scenery_tree_far`: three stacked hex frusta for Oak, a 0.95→0.12 spike for Poplar) at a 3.0–3.4× far scale, so the ring's trees stand 25–38 m tall — bigger than any tree inside the map — and read as hexagonal Christmas trees on every horizon. Drzewa 3.0 never touched this path | `scene_build/src/backdrop.rs:43-75`, `scene_build/src/foliage.rs:38-46,197-257`; `target/orliny_pine_belt.png`, `target/bystra_treeline.png` | W1 (first PR) | the ring is grown from Drzewa 3.0's impostor rung per species; the frustum kit has zero call sites and is deleted; lock: no backdrop tree taller than 1.3× its species' envelope; the ring's species mix follows the map's climate; perf measured on the MX330 (A→B→A) |
| F2 | **Monoculture.** Inside every map: Oak and Bush only. Poplar, Willow, FruitTree and Pine are fully grown (PRs #626–#628) and placed nowhere. Orliny's `pine_belt` view has no pine; Bystra's river has no willow | all five map blueprints | W1 | every map places ≥ 3 species; a report gate: no species exceeds 70 % of a map's trees; the per-map dossier carries its species table |
| F3 | **The shelterbelt is boxes on sticks.** `TreeLine` is a szpaler of slabs on stick trunks | `scene_build/src/battlefield.rs:950` | W1 | built from Drzewa 3.0's Mid rung; blocking AABB bit-identical before/after (honesty doctrine, the W2 art-direction rule) |
| F4 | **Density.** 12–22 oaks per square-kilometre map | map blueprints | W4 | per-map tree floor from the dossier, perf measured |
| F5 | **No variety.** No dead trees, saplings, stumps, logs or hedges as scenery (stumps and logs exist only as destruction wreckage); one canopy colour per species (`foliage.rs:183-192`); no seasonal or climate tint; statics-baked species get no wind | `world_forge/src/tree/mod.rs`, `foliage.rs:108` | W4 | `SceneryKind` appended (dead tree, stump, log, hedge); per-map canopy tint in the map blueprint; wind on every placed species |
| F6 | **No open flora row anywhere**; D5/D12/D13/D14 all closed; the backdrop comment claims kilometres at 40 m | `docs/art-direction-program.md`, `backdrop.rs:39-42` | W1 (first PR) | the comment is rewritten; flora debt lives here |

### S — the shot

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| S1 | **No dynamic light.** The renderer has the sun and nothing else; a 100 mm gun at dusk lights neither its glacis nor the ground. This is the single largest reason a shot reads as a decal | `renderer_api` (no point-light concept), `fx/emitters.rs:29-56` | W1 | one muzzle light in terrain, scene and vehicle shading, caliber-scaled in radius and energy; lock: a dusk probe frame's ground luminance under the muzzle rises by a measured floor; cost measured on the MX330 (A→B→A) before the budget is touched |
| S2 | **Sniper mode gets nothing.** The camera kick returns early when scoped, and most aimed shots are scoped | `client/src/camera/smoothing.rs:129-136` | W1 | a rotational tremor in sniper (no translation), caliber-scaled, decaying within a locked window; `camera_feel.rs` updated from "leaves sniper rigid" to the new promise |
| S3 | **Nothing is caliber-scaled but audio and tracer width.** Flash 1.0→1.6 m, smoke 8 particles, dust ring 10, recoil stroke 12 m/s, hull impulse 0.16 rad/s, camera kick 0.9/0.5 m/s — all constants, identical for 75 mm and 128 mm | `fx/emitters.rs`, `engine/src/components.rs:146-174`, `engine/src/attitude.rs:35`, `smoothing.rs:71-72` | W1 | every channel derives from one recoil momentum (`mass_kg × muzzle_velocity_mps`, both already on `ShellSpec`) through one function; lock: 128 mm exceeds 75 mm on every channel |
| S4 | **No mechanical layer.** No breech clack at fire, no casing, no recoil-cycle metal, no dust shaken off the hull — one synthesized blast with nothing before or after it | `audio/src/voices/cannon.rs` | W1 | breech, casing and cycle voices in `audio` (pure DSP), hull dust on fire; mixer locks |
| S5 | **Being hit is mute in the body.** Camera shudder only; `HullAttitude` has `fire_impulse` and no `hit_impulse`; no "armour held" callout although the exactly-once lane already carries the absorbed-impact truth; no screen effect | `engine/src/attitude.rs`, `hud/reticle_readouts.rs` | W1 | an incoming impulse on the hull spring from hit direction and energy; an absorbed-impact callout fed by the personal-truth lane; locks |
| S6 | **The roadmap lists fire feel DONE**, so it was never audited | `docs/ROADMAP.md:21` | W1 (first PR) | reworded |

### R — roles

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| R1 | **No concealment stat.** `spotting.rs:72-77` returns a fleet-wide binary; a Jagdtiger and a T-34-85 hide identically, and view range has two values (400 × 5, 440 × 3) | `sim/src/spotting.rs`, `tank.rs:132` | W2 | a per-vehicle concealment factor and an authored view range from the dossier; lock: the bigger silhouette is seen farther, stationary |
| R2 | **One dispersion factor.** Movement, hull traverse and turret traverse share `movement_bloom_mrad` times fleet constants 0.35 / 0.25; no gun can be "bad on the move, fine on the turret" | `sim/src/aim_dispersion.rs:41-43` | W2 | three authored factors per gun; lock |
| R3 | **No ground pressure.** `SuspensionModule` has no track width; terrain resistance is per-material only, so a 32 t T-34 and a 75 t Jagdtiger handle mud identically | `terrain/src/ground.rs:87-99`, `physics/src/contact.rs:45` | W2 | track width on the module; pressure from mass and contact patch; soft-ground resistance scales with it; lock: the heavy hull is slower on mud and equal on cobble |
| R4 | Crew proficiency pinned at 1.0 for all | `crew.rs:54` | decided | stays — proof, never power |

### U — interface

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| U1 | **`ui_kit` is a triangle emitter, not a toolkit.** 1 097 lines of `push_x(&mut Vec<HudVertex>, …)` in clip space with a manual aspect; no layout containers, anchors, padding, scissor, DPI (nothing reads `scale_factor`), focus, hover or press state, text wrapping, second size class | `crates/ui/ui_kit/src/` | U (parallel) | a layout layer (rect tree, edge anchors, row/column with padding, measured text, a `Ui` context carrying aspect, DPI and scissor), theme size classes, a hover/press/focus machine, `set_scissor_rect` in the HUD pass |
| U2 | **245 vertex-equality tests** across `client/src/hud` and `app/garage` are the real cost of any redesign | `hud/reticle_overlay_tests.rs` (30), `garage/actions.rs` (26), … | U (first) | a semantic draw list keyed by element name; tests query it; the vertex-equality count ratchets to zero as elements migrate |
| U3 | **ASCII only.** The atlas bakes 0x20–0x7E; unknown glyphs are silently skipped; `ui_strings.rs` forbids non-ASCII | `ui_kit/src/font/bake.rs:42`, `font/layout.rs:225` | U | Latin Extended-A baked; the ASCII rule removed; lock: an unknown glyph renders visibly, never skips; a Polish string in a golden |
| U4 | **22 HUD elements on hard-coded floats**; absent entirely: kill feed, team lists, scoreboard, ping | `client/src/hud.rs:174-262` and `hud/*.rs` | U | the HUD is a layout description; the missing elements exist; the sight locks and `reticle_strip` are preserved unchanged |
| U5 | **No product shell**: no settings, keybinds, battle results, lobby, localization | `docs/ROADMAP.md:88-89` | U | the screens exist with goldens |

### O — the picture

The light's register stays in [art-direction-program.md](art-direction-program.md); this program
feeds it rather than duplicating it. D8 (contact AO on one vehicle), D9 (dirt lane never populated),
D15 (nothing to look at up close), D17 (pastel showcase) are W3 outputs — they close with K3 and
K6. D4 (no dark mass on the steppe) and D18 (Orliny's borrowed light) are W4. One new row:

| ID | Defect | Evidence | Wave | Closes when |
|---|---|---|---|---|
| O1 | **The card meadow reads as a wavy moiré** in the mid-field band, most visible at grazing light | `target/flora_lineup.png`, `target/prokhorovka_evening_contact.png` | W4 | a metric derived from a frame judged good (look-metric-validation rule), then a floor |

## Wave plan

**W1 — Widok.** Everything the player sees first, none of it touching the authoritative model.
Order inside the wave: F1 (the cones: one PR, the biggest area of any frame), F6/Z4/S6 (the
register rewording, one PR), F2, F3, Z1, Z2, Z3, S3 (the momentum function, because S1/S2/S4/S5
scale from it), S1 (the light, with its perf sandwich), S2, S4, S5. Gate: every W1 row closed; the
frustum kit gone; the light's cost recorded.

**W2 — One truth of the track.** G1 first (the sampler), then G3, G4, G2 (the wire), G5, G6. R1–R3
ride here because they touch the same ground and spec files. Gate: the cross-check locks exist for
every pair named in G6; replay fixtures re-pinned once, deliberately, for the wire bump.

**W3 — Kuźnia 2.0.** K7 first (the documents), then the seam (K1, K2), then seven vehicle
migrations — one PR each, with its dossier, its P1 parity gate and its P2 thicknesses — then the
three capabilities the kernels lack (K4, K5, K6), each proven on the T-54 and rolled to the fleet
in the same wave, not the next. P3 closes with the last migration. Gate: the K3 inventory gate
green for every roster vehicle; D8/D9/D15/D17 closed in the light's register.

**W4 — Obraz.** F4, F5, O1, D4, D18. Content and per-map identity, on top of a fleet that finally
has surfaces to light.

**U — the interface**, in parallel from day one on files no other wave touches: U2 and U1 first
(the foundation), then U3, U4, U5.

A wave is done when every row in it is closed, its locks are in the ratchet, and the roadmap's
"DONE" lines it touched are reworded. A row closes with a number or a frame, never with a sentence.

## Verification

The merge gate is `scripts/verify.ps1` (see `CLAUDE.md`). In addition, for this program:

- Any renderer change (S1, F1, F4) lands with an MX330 A→B→A measurement (single runs vary by
  3–5 ms with thermals); the budget moves per item, never fleet-wide.
- Any sim number that moves (Z2, G2, R1–R3) re-pins the replay fixtures in the same PR and says why.
- Goldens are blessed deliberately, in the PR that moves them, with the before/after in the message.
- Each vehicle migration (K3) ends with the close-up review under the model-logic bar, not with a
  triangle count.
