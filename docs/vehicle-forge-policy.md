# Armored Vehicle Forge Policy

The Armored Vehicle Forge is the authoring-and-bake layer that sits above low-level procedural
geometry. Its goal is World-of-Tanks-beta-like *readable realism*: one benchmark vehicle that reads
as a specific, photo-backed machine — recognizable silhouette, cast turret mass, plate seams, track
recess — without hand-modeling every tank in a DCC tool. Procedural source stays the source of
truth; the runtime renders baked, optimized assets rather than rebuilding tanks each frame.

This policy locks the philosophy. It is the bar every later Forge task is measured against: *does
this move the benchmark vehicle closer to the Forge quality target?*

## Locked Decisions

- **Model pipeline:** procedural source + baked assets + runtime variation. Not runtime full-tank
  generation, and not hand-authored DCC meshes.
- **First quality benchmark:** the canonical **T-54-3 obr. 1951**. One excellent vehicle before
  shallow upgrades to all vehicles. `T-55A` was removed outright in the Genialna Flota program
  (protocol v33) — the roster carries no clones.
- **Renderer target:** PBR-lite with baked maps (albedo, normal, AO/roughness, optional cavity).
- **Name:** *Armored Vehicle Forge*. The old flat `VehicleBlueprint` is a prototype stepping stone,
  not the destination model.

## Relationship To Existing Work

- `vehicle_geometry` remains the low-level, renderer-neutral mesh/kernel crate. It is not renamed or
  deleted; the Forge grows on top of it. See [vehicle-geometry-policy.md](vehicle-geometry-policy.md).
- `VehicleBlueprint` (in `game_core`) is the current single shape source of truth for the few
  migrated vehicles. It is treated as a prototype: useful, but gradually superseded by the Forge's
  semantic part graph. It is not ripped out early.
- The pre-Forge lineup screenshots are kept as the comparison baseline. New Forge output is judged
  against them, not against a blank slate. The photo-reference record lives in the per-vehicle
  dossiers (`docs/vehicles/*.md`); the old fleet-wide photo audits were retired once their findings
  either shipped or moved into those dossiers.

## Ownership And Boundaries

The `vehicle_forge` crate owns the layers above raw mesh construction: reference packs, semantic
vehicle models, bake profiles, and Forge artifacts. It may depend on `vehicle_geometry`,
`game_core`, `glam`, and `serde`.

It must **not** depend on or reference any renderer backend: `renderer_api`, `renderer_wgpu`,
`wgpu`, `winit`, or `egui`. The Forge is an authoring/bake layer, not a renderer layer. This is
enforced by `quality::architecture_rules::vehicle_forge_stays_renderer_free`, which checks both the
manifest and the source. The renderer consumes Forge *artifacts*; it is never consumed by the Forge.

## The Six Layers

1. **Reference Layer.** Collects sources — photos, side/front/top views, dimension data,
   interpretation notes — and records ratio targets per vehicle: length/width/height, track height,
   turret width, gun protrusion, wheel count, mantlet size. Output is a `ReferencePack`: the proof
   of where the proportions come from. Ratio reports state percentage deltas, not just pass/fail, so
   a mesh that is technically valid but *proportionally wrong* still fails.

2. **Semantic Vehicle Model.** Replaces the single flat blueprint with a part graph: hull plates,
   lower tub, sponsons, fenders, track runs, road wheels, turret shell, turret cheeks, mantlet, gun,
   cupola, hatches, hooks, welds. Each part carries a local frame, material role, gameplay role,
   source note, and LOD policy. Mount frames are derived from semantic parts, not hand-typed magic
   values.

3. **Forge Geometry Kernel.** The existing `extrude`/`revolve`/`chamfered_prism` operators stay as
   the foundation. The Forge adds stronger operators: plate boxes with thickness/bevels/normal
   seams, multi-section lofts for hulls and turrets, a cast-shell builder for asymmetric turret
   cheeks, a real track belt, a wheel train (road wheels, idler, drive sprocket, rollers), and
   detail scatter for bolts, hatches, handles, and welds. The kernel emits not just positions but UV
   islands, tangents, material IDs, and bake metadata.

4. **Bake Artifact Layer.** The Forge produces an asset, not just an in-memory mesh. The target
   layout is a manifest (`manifest.json`: vehicle, variant, LODs, materials, source hash), geometry
   (`meshes.bin`), maps (`albedo.png`, `normal.png`, `ao_roughness.png`, optional `cavity.png`), and
   review renders (front, rear, profile, top, battle-oblique). Early bakes may live in memory, but
   the artifact format is designed up front.

5. **PBR-lite Vehicle Renderer.** A separate vehicle pipeline rather than bloating `SceneVertex`.
   `VehicleVertex` (position, normal, tangent, uv, material_id, tint_mask) feeds a shader with normal
   mapping, AO/cavity, roughness specular, and sun + sky fill. Terrain and simple scene meshes stay
   on the existing lightweight path. Armor tint remains a layer over the material, not the vehicle's
   identity.

6. **Runtime Variation Layer.** The runtime adds state to a baked benchmark — hit decals, mud/dust/
   snow, camo/team markings, damaged modules, broken tracks, optional equipment — but it never models
   the full tank. This layer comes only after a stable baked benchmark.

## Gameplay Honesty

A prettier model must not break gameplay:

- Hull and turret/casemate visual bounds stay inside the gameplay hitbox/turret plan.
- The mantlet/gun may protrude, but it has a distinct role.
- Mount frames come from semantic parts, not hand-typed constants.
- Casemate vehicles keep turret yaw ignored.
- The render pose chain stays: hull origin → turret ring → trunnion → muzzle.

## Tests And Gates

Every Forge phase lands with executable checks alongside the prose:

- ratio reports for the benchmark vehicle against its `ReferencePack`, reporting percentage deltas.
- deterministic bakes (stable hashes); source-hash changes are detectable.
- LODs preserve mount frames and hitbox honesty.
- UVs stay inside atlas bounds; tangents are finite and normalized enough for normal mapping.
- the renderer loads vehicle textures and falls back cleanly when a debug texture is missing.
- the review screenshot set contains all required camera views.
- non-Forge vehicles keep rendering through the fallback path until migrated.
- **the mesh-quality contract runs over the fleet, not only over test shapes.**
  `vehicle_geometry/tests/fleet_mesh_quality.rs` audits every submesh of every procedural bake and
  `vehicle_forge/tests/shipped_mesh_quality.rs` audits whatever `authoritative_baked_vehicle`
  resolves to, so the hybrid benchmark is covered too. Invalid indices, non-finite vertices,
  non-manifold edges, zero-area triangles and non-unit normals are hard failures at any count;
  inconsistent winding carries a recorded per-vehicle CEILING that can only shrink.

  This gate exists because it was missing. `quality.rs` had defined a valid mesh since the kernel
  pivot and `mesh_quality.rs` proved the audit correct — on tetrahedra and quads. Nothing ran it on
  a tank, and **seven of eight guns shipped with 24–28 inconsistently wound edges** while every
  ratio, budget, silhouette and determinism gate stayed green and the Studio report printed
  `DEFECTS` to nobody. A contract nobody runs on the real thing is a document, not a gate.

  Recorded debt: **none — the whole fleet winds consistently.** The last entry was the IS-3 hull's
  2 edges, booked as a shape decision at the pike/tub-step junction. It was not one. Both pike
  planes pass through the fold and carry the same plan sweep, so at the fold's own height their
  plan traces have the identical slope `-tan(sweep)` — the glacis and lower slopes cancel — which
  makes the fold, the tub step corner and the step corner **exactly collinear for any slopes,
  sweep or widths**. Fanning across that line swallows the middle window whole; walking the
  boundary through it tiles the region once. The boundary order was forced by the geometry, not
  chosen. `recipes/is3_hull.rs` now locks the collinearity, so the walk stays derivable if the
  hull's proportions ever move.

  The lesson generalises: before booking debt as a shape decision, check whether the shape already
  decided. The ceiling mechanism stays for debt that is genuinely a choice.

- **every declared semantic part must answer to geometry that exists.**
  `vehicle_forge/tests/part_graph.rs::every_declared_part_answers_to_real_geometry` requires each
  non-running-gear part's bounds to contain at least one baked vertex. Containment gates only prove
  parts do not escape the vehicle; they pass an EMPTY box exactly like a full one. The Jagdtiger
  shipped a commander's cupola declared on the empty left flank of its roof while the casting stood
  on the right, because the derived part table placed every cupola at `turret.min.x * 0.4` — a
  bounding-box fraction that is always negative — instead of reading the `cupola_x` the blueprint
  had already authored. Derived parts read the blueprint now.

The canonical gate remains `./scripts/verify.ps1`. Focused crate tests (`cargo test -p
vehicle_forge`, `-p vehicle_geometry`, `-p renderer_wgpu`) are fine for tight loops, but no phase is
complete until the full gate passes.

## The Seal Gate (Garage Distance)

Number gates are the floor, never the bar. Carried over from the 2026-07-17 model-logic audit
(its defect ledger closed clear), this is what every vehicle PR passes before any "sealed" claim:

1. **Garage-distance renders** — front, rear ¾ low, flank close, turret close, gun close —
   reviewed for floaters, interpenetrations, closed openings, scale absurdities.
2. **Functional checklist**: every hatch passes a 0.55 m torso; the barrel has a visible bore;
   the engine deck has intake AND exhaust paths; tracks carry the wheels (contact, sag); every
   attached thing has a bracket or support; nothing shares its exact shape with another vehicle
   unless the real vehicles shared it.
3. Numbers gates (dimensions/ratios/budgets) stay — as the floor, not the bar.

One lesson from that audit outlived its ledger (Jagdtiger JT.3): a cage asserted the cast collar
"spans over a metre" and passed — on a collar buried 275 mm INSIDE the leaning plate. Width was
never the property in question; RELIEF was. **When a lock and a photo disagree, check whether the
lock measures the quantity the photo is about.**

The audit named two systemic causes, and both are answered: the missing functional-logic pass IS
this gate, and the clone factory is gone in code — per-vehicle deck layouts in
`vehicle_recipes/src/deck_details.rs`, per-family `add_german_cast_cupola` /
`add_soviet_slit_cupola` / `add_british_cupola` in `turret_fittings.rs`, per-family
`shoe_pattern` + `wheel_face` in the track shape, and every braked gun wearing a real
double-baffle with OPEN chambers over a recessed dark bore
(`vehicle_recipes/src/armament.rs:153-225`).

## Budget Procedure (Raising Is Conscious, Never Drift)

1. Measure the candidate detail with `detail_cost_probe` (client examples) before deciding.
2. Exhaust the cheap paths first: running-gear instancing (interleaved wheels are more
   *instances*, not more unique triangles), detail kernels, harder LOD1/LOD2.
3. Only then raise a budget, with the probe number in the PR description. Stated honestly: the
   fleet bake envelope is currently ONE `VEHICLE_BUDGETS` covering all nine kinds
   (`crates/vehicle/vehicle_recipes/src/budgets.rs`), so "raise it per-vehicle" is a rule the
   code cannot yet express. The only per-vehicle precedent is the T-54 hybrid's own
   `MEDIUM_LOD0_TRI_BUDGET` of 22k in a different crate
   (`crates/vehicle/vehicle_build/src/t54.rs:19`), and the 2026-08-03 `gun_tri` 500→650 raise
   was a fleet-wide bump justified by a T-54-only measurement (recorded in `budgets.rs`). A
   per-vehicle envelope mechanism is open debt of this procedure.
4. LOD ratios keep holding after any raise; the gates are
   `vehicle_recipes/tests/vehicle_budgets.rs` and `vehicle_recipes/tests/vehicle_lod.rs`.

## The Per-Vehicle Authoring Protocol

Every vehicle passes steps (a)–(h), packed into 2–3 PRs. Data first, model second: the dossier
and its machine-checked targets land *before* the PR that edits the blueprint RON.

**"Dossier and measure"** — zero geometry changes.

- (a) Research the real tank: museum specimens, factory drawings, manuals, model-kit
  cross-checks. Output: a dossier in `docs/vehicles/<x>.md` following
  `docs/vehicles/_template.md`, with the anchor-numbers table
  (dimension | value | source | confidence | encoding).
- (b) `DimensionTarget`s + tightened `RatioTarget`s in the vehicle's reference pack
  (`vehicle_forge/src/packs*.rs`), each with a `ReferenceSource`. Targets the current model
  *fails* enter with tolerance temporarily widened to the current state plus a
  `// TODO(<wave>-<x>): target ±…` — the PR stays green while the intended gate is already
  written down.

**"Shape"**.

- (c) RON correction through the Studio loop (`cargo run -p tools -- studio --vehicle X
  --blueprint-file Y`), contact sheet compared against the dossier photographs, iterated until
  Δ% sits inside tolerance. The RON also feeds the hitbox and the armor volumes — check
  `game_core/src/armor/vehicle_volumes.rs` behaviour after every major change.
- (d) Detail in the recipe (`crates/vehicle/vehicle_recipes`), built from the kernel crates.
- (e) Bespoke part table in `vehicle_forge/src/part_data/<x>.rs`, then tighten the temporary
  tolerances to their targets (delete the TODOs).

**"Cage and seal"**.

- (f) Benchmark cage at Tiger-class density (17–28 anatomy asserts, each quoting the dossier's
  real dimensions in a comment).
- (g) `GOLDEN_BAKE_HASHES` re-record (`vehicle_recipes/src/budgets.rs`) plus the budget, LOD and
  dimension gates (`vehicle_recipes/tests/vehicle_budgets.rs`,
  `vehicle_recipes/tests/vehicle_lod.rs`, `vehicle_forge/tests/dimension_gate.rs`).
- (h) Verification renders in the PR description: Studio contact sheet, a PBR studio render, the
  fleet lineup — and then the seal gate above.

## Milestones

Status 2026-09-05 (Forge 2.0, K7): milestones 0–2, 5 and 6 are **done**; 3 and 4 are **done
for the geometry, open for UVs and bakes** (K6); 7 is **partial**; 8 has **not started as a
migration** — no vehicle beyond the T-54 routes through the part library, and the line below
read "in progress" from 2026-08-03 with zero migrated. Since K1 (2026-09-05) the seam is one
rule: a blueprint with a complete visual builds through `vehicle_build`, everything else is its
recipe wrapped as a `Sketch` description — a vehicle migrates by DATA, never by a new match arm.
What the fleet has is the blueprint cage and the W4 fleet slot (below). The migration is Forge
2.0 K3.

0. **Lock the philosophy** — this document; benchmark and baseline chosen. *(done)*
1. **Reference pack and ratio tests** — `ReferencePack` for T-54, photo-derived ratio tests.
   *(done)*
2. **Semantic part graph** — move T-54 from flat constants to a `ForgePartGraph`. *(done)*
3. **Geometry operators for real tank forms** — plate/loft/cast-shell/track-belt/wheel-train, UVs,
   tangents. *(operators done; UVs are authored by ONE kernel — `solid`'s convex block,
   `solid/convex.rs` — and every other kernel output is triplanar with no chart; tangents follow
   the same split. K6)*
4. **PBR-lite vehicle pipeline** — `VehicleVertex`, material textures, normal/AO maps, shader path,
   screenshot regression. *(pipeline done; the normal/AO "maps" are per-role synthesised noise —
   `vehicle_forge/src/artifact/material_synthesis.rs`, tuned to invisibility — and the only baked
   AO is the T-54's contact pass; there is no per-part normal/AO bake. K6)*
5. **Bake artifact and toolchain** — Forge CLI writes artifact folders; client loads baked assets.
   *(done)*
6. **First production benchmark** — T-54 with LOD0/1/2, full screenshot set, passing ratio/
   geometry/renderer/perf gates. *(done — the Model Idealny programme took it further, to zero
   dimensional deviations; see the lessons below)*
7. **Runtime variation** — decals, dirt/camo, equipment, damage and track state. *(partial)*
8. **Migrate other vehicles** — Jagdtiger, Tiger I, Tiger II, then Panther II after an explicit
   interpretation decision. *(NOT started — no vehicle has migrated since 2026-08-03; the whole
   fleet is blueprint-born and caged on the procedural recipe, and per-vehicle visual authoring
   runs through the fleet slot. Forge 2.0 K3, after K1/K2 open the seam)*

Each migrated vehicle must arrive with a `ReferencePack`, part graph, ratio tests, LODs, screenshot
review, and a baked material set.

### The Fleet Slot (W4 F5)

A vehicle's visual is a sum of OPTIONAL parts loaded from `<slug>.visual.ron`
(`game_core/blueprints/`): a partial file improves the look without claiming cut-truth, and `None`
is a decision, not an omission. The Tiger I is the first and so far only vehicle to author one
(#422) — its gun group: the bore-honest KwK 36, the double-baffle brake with open chambers
(`revolve::muzzle_brake`), and the Walzenblende spanning exactly the armour's mantlet patch band.
The T-54 is the exception by design: its `VisualDetail` is generated in Rust from the blueprint
(`game_core/src/vehicle_blueprint/source.rs` — an embedded file "would be a second source about to
disagree" with the generated tree).

## Lessons From The Closed T-54 Programme (Model Idealny, 2026-07-29)

The programme that took the benchmark to zero deviations (PRs #330–#362) closed with its truth in
two places: the dossier (`docs/vehicles/t-54.md`) and the gate
(`vehicle_forge/tests/dimension_gate.rs`, which asserts every T-54 anchor is Locked and the debt
list is empty). Two findings from it apply to every vehicle that follows:

**The instrument and the thing it measured were the same mistake.** Hull length caught the stowed
beam; bare-roof height caught the hatch lid; cupola diameter caught the roof plate; ground
clearance looked in a window narrower than the floor it was measuring; the phantom-width lint read
the hitbox against a model that shared its error; three tests REQUIRED the wrong shape outright.
The workshop itself opened the programme with the same defect class: self-calibrated anchors with
dangling citations, mirrored review tiles, and a fast loop that did not send the mesh it claimed
to. Every one of these surfaced only when the geometry became correct — which is the argument for
fixing the vehicle and the tape measure in the same commit.

**A number written down twice is a number about to disagree with itself.** The tub width, the
wheel stations, the fender centre, a top-run anchor with `0.96 − 0.32` hidden inside a bare
`0.66`, the DShK inheriting the tank gun's calibre — each was a copy, and each broke the moment
its source moved.

**M14 was one finding doing two jobs, and half of it is closed** (P2.1,
`docs/contact-and-tracks-program.md`). The T-54's box reaches 0.141 m past its outermost armour
volume, and that box used to decide both where a shell connects and where the hull could BE.
Movement no longer reads it: a hull is blocked, shoved and billed for ramming as `HullPlan` — the
outer face of the belt and the hull's own plates, straight off the blueprint — and
`a_hull_is_blocked_by_exactly_the_metal_it_is_drawn_with` asserts that to zero rather than to a
ceiling.

What stays open, and stays the USER's, is the SHELL half: a round still connects with 0.141 m of
air beside the track. That is the harder half — narrowing the hitbox moves armour resolution, the
bots' aim gate and every fixture that fires at a known point — and it is measured every run by
`the_shell_volume_does_not_grow_further_past_the_visible_vehicle`.

## Open Fleet Debt (2026-08-03)

Recorded so the next vehicle PR starts from the true state. Four gaps, each with the code that
proves it:

1. **Six vehicles derive `cupola_height` from the collision box.** Only the T-54 (0.131) and
   Tiger I (0.115) author it; everyone else falls back to "whatever fills the gap up to the
   hitbox apex" (`game_core/src/vehicle_blueprint/mod.rs:172-174`), and since #426 that derived
   number is SHOOTABLE ARMOR — the cupola drum volume is built from it
   (`game_core/src/armor/vehicle_volumes.rs:142`). Derived today: Tiger II 0.34, T-34-85 0.33,
   Panther II 0.27, Centurion 0.21, IS-3 0.10, Jagdtiger 0.07 — dossier numbers for none of them.
2. **Six vehicles have no `glacis_ports`.** #428 retired the front weakspot "smear" fleet-wide,
   but only the T-54 (one bow port) and Tiger I (two) authored the replacement patches
   (`game_core/blueprints/t54_1951.blueprint.ron:141`,
   `game_core/blueprints/tiger_i_ausf_e.blueprint.ron:91-94`); the other six now front nothing
   but the mantlet patch and a derived cupola.
3. **Seven of eight turret armor volumes are a swept CIRCLE, not the casting.** The sector plane
   anchors on the authored `turret_loft` support function only where one exists — today the T-54
   alone (`game_core/src/armor/vehicle_volumes.rs:344-348`); every other turret keeps the swept
   radius until its own dossier arrives.
4. **Centurion, IS-3 and T-34-85 have no `DimensionTarget`s**, so their dossier numbers are
   unmeasured: the dimension gate skips packs with no dimensions
   (`vehicle_forge/tests/dimension_gate.rs:20-22`) and the packs carry the unclosed TODOs
   (`reference/is3.reference.ron`, `reference/centurion_mk3.reference.ron`, `reference/t34_85.reference.ron` carry empty `dimensions`). The cost is already visible:
   the IS-3 dossier says 2.44 m to the turret roof while the shipped blueprint bakes
   `roof_y: 2.39` (`game_core/blueprints/is3.blueprint.ron:53`) and no gate can notice. The
   T-34-85 additionally had no dossier at all until the unresearched stub
   (`docs/vehicles/t-34-85.md`).
