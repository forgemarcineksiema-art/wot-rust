# Vehicle Geometry Photo Analysis

This audit ties the procedural vehicle recipes back to reference imagery and to the current
rendering technology. The executable tests prove that the meshes are finite, deterministic, within
budget, and honest against gameplay hitboxes. The photo pass covers the non-executable question:
whether the lineup reads like the intended vehicles from the angles players actually see.

## Executive Assessment

Current state: the geometry system is technically solid, but the photo-grounded authoring record is
only partially mature.

- Geometry integrity: strong. The `vehicle_geometry` crate bakes all known vehicles, tests fit/fill
  constraints, mount frames, deterministic hashes, budgets, and broad silhouette uniqueness.
- Runtime integration: strong. The client now renders baked submeshes through cached mesh handles
  and `RenderObject` transforms rather than rebuilding every tank mesh each frame.
- Photo traceability: mixed but now stricter for the benchmark. T-54-3 obr. 1951 is the canonical
  Forge target and has a single `VehicleBlueprint` source for visible shape, hitbox, mount frames,
  and armor slopes. T-55A remains legacy-compatible, but it is no longer a production benchmark or
  review-lineup vehicle.
- Visual fidelity: acceptable for stylized early playable geometry, not yet museum-grade. The
  lineup communicates vehicle classes and family differences, but several reference-visible cues
  are simplified: German interleaved wheels, Tiger II turret mass/taper, Jagdtiger casemate detail,
  and the exact T-54 road-wheel/track presentation.

## Evidence Collected

### Commands

Focused geometry gate:

```powershell
cargo test -p vehicle_geometry
```

Result during this audit: passed.

Architecture gate:

```powershell
cargo test -p quality --test architecture_rules
```

Result during this audit: passed.

Single current lineup render:

```powershell
cargo run -p client --example vehicle_lineup -- target\vehicle_lineup_codex_analysis.png
```

Result during this audit: wrote `target/vehicle_lineup_codex_analysis.png` at 1800x620.

Multi-camera audit render:

```powershell
cargo run -p client --example vehicle_lineup_views -- target\vehicle_geometry_views
```

Result during this audit: wrote seven 1800x720 PNGs.

Contact sheet generated for quick review:

```text
target/vehicle_geometry_views/contact_sheet.png
```

### Screenshot Set

These files are generated artifacts under `target/`; regenerate them instead of treating them as
source assets.

| View | Purpose | File |
| --- | --- | --- |
| Front | Checks frontal width, turret/casemate face, mantlet seating, track spacing | `target/vehicle_geometry_views/01_front.png` |
| Rear | Checks rear deck mass, rear slope simplification, track symmetry | `target/vehicle_geometry_views/02_rear.png` |
| Right profile | Checks hull length, gun protrusion, turret/casemate side shape, road-wheel read | `target/vehicle_geometry_views/03_right_profile.png` |
| Left profile | Mirrors profile read and catches asymmetric artifacts | `target/vehicle_geometry_views/04_left_profile.png` |
| High three-quarter | Checks combined silhouette from a gameplay-readable camera | `target/vehicle_geometry_views/05_high_three_quarter.png` |
| Top-plan oblique | Checks plan shape, turret plan, gun axis, hull/turret footprint honesty | `target/vehicle_geometry_views/06_top_plan_oblique.png` |
| Turret slew battle oblique | Checks pose chain under hull yaw, turret yaw, and gun pitch | `target/vehicle_geometry_views/07_turret_slew_battle_oblique.png` |
| Contact sheet | Quick index of all generated views | `target/vehicle_geometry_views/contact_sheet.png` |

## Reference Set

The photo references used in this pass are deliberately broad. They are not a final per-bolt
modeling guide; they are evidence for silhouette, massing, and major fittings.

- T-54 benchmark: Wikimedia Commons T-54/T-55 gallery
  (`https://commons.wikimedia.org/wiki/T-54/T-55`), plus the project source notes in
  `docs/vehicles/t-54.md`.
- Tiger I: Wikimedia Commons Tiger I category
  (`https://commons.wikimedia.org/wiki/Category:Tiger_I`), plus the project source notes in
  `docs/vehicles/panzerkampfwagen-vi-tiger.md`.
- Tiger II: Wikimedia Commons Tiger II category
  (`https://commons.wikimedia.org/wiki/Category:Tiger_II`), plus the project source notes in
  `docs/vehicles/panzerkampfwagen-vi-b-tiger-ii.md`.
- Jagdtiger: Wikimedia Commons `Jagdtiger side USAOM.jpg`
  (`https://commons.wikimedia.org/wiki/File:Jagdtiger_side_USAOM.jpg`), plus the project source
  notes in `docs/vehicles/jagdtiger.md`.
- Panther II: Wikimedia Commons `Panther II.Fort Knox.jpg`
  (`https://commons.wikimedia.org/wiki/File:Panther_II.Fort_Knox.jpg`), plus the project source
  notes in `docs/vehicles/panzerkampfwagen-v-panther-ii.md`.

## Technology State

### Data ownership

`game_core` owns durable vehicle identity and gameplay data: `VehicleKind`, `TankSpec`,
`HitboxProfile`, module loadouts, mount frames, and armor facets. The newer `VehicleBlueprint`
path is important because it lets one shape description feed several consumers:

- collision hitbox;
- turret plan;
- mount frames;
- armor slopes and weakspot multipliers;
- procedural visual geometry.

At the time of this audit, only `T54_1951` has a blueprint. That makes T-54 the strongest example
of "what you see is what the simulation believes." The rest of the vehicles still use older
hand-authored recipe constants for hull proportions, tracks, turrets/casemates, and guns.

### Geometry kernel

`vehicle_geometry` is a renderer-neutral bake step. It outputs `BakedVehicle` values split into
three submeshes:

- hull: body, tracks, wheels, fenders, fixed casemate body when applicable;
- turret: rotating turret or fixed casemate superstructure;
- gun: barrel, mantlet/mask, muzzle brake or bore evacuator.

The kernel builds low/mid-poly forms with operations such as extrude, revolve, chamfered prisms,
mirroring, arrays, smoothing groups, and deterministic vertex-color shading. This is intentionally
not a glTF or texture workflow yet. It is a deterministic procedural authoring system built for
gameplay-readable silhouettes.

### Render path

The client adapts `vehicle_geometry` meshes to `renderer_api::SceneVertex` in
`VehicleMeshCatalog`. Meshes are registered once per vehicle/submesh and then rendered as
`RenderObject`s. The important technical detail is pivot handling:

- hull mesh is registered around the hull origin;
- turret mesh is registered around the turret-ring frame;
- gun mesh is registered around the gun trunnion.

`client::vehicle_pose::VehiclePose` applies the same chain at draw time: hull yaw around origin,
turret yaw around the ring, gun pitch around the trunnion, and casemate vehicles holding turret yaw
at zero. This keeps the visual mesh path aligned with the same gameplay mount frames used by
simulation and aiming.

### Screenshot technology

`crates/apps/client/examples/vehicle_lineup_views.rs` uses the same baked render-object path as the
runtime client. It is not a separate mesh renderer. The example changes only camera position,
camera FOV, hull yaw, turret yaw, and gun pitch in the snapshots. That makes the screenshots useful
evidence for the current production path, not a special debug visualization.

## Automated Coverage

The current tests cover:

- finite positions, valid indices, triangle counts, and unit normals;
- body fit/fill against `HitboxProfile`, excluding gun length by design;
- turret/casemate fit/fill against the gameplay turret plan box;
- segmented track geometry on both sides;
- per-vehicle silhouette uniqueness beyond raw box dimensions;
- deterministic bake hashes and unique hashes;
- vertex and triangle budgets;
- sane turret ring, trunnion, and muzzle mount frames;
- cupola seating, turret ring collars, and mantlet sockets;
- T-54 blueprint hull inset and wide sponson;
- casemate pose semantics through `VehicleKind::effective_turret_yaw_rad` and `VehiclePose`.

These checks are good at catching broken geometry. They do not prove historical fidelity. For that,
the project needs photo-derived ratio tests and per-vehicle blueprint comments.

## Visual Findings By Vehicle

### Prototype Medium

Status: test/gameplay placeholder, not a photo-authored historical vehicle.

Evidence:

- The vehicle is present in `VehicleKind::ALL` for test coverage, but it is not in
  `VehicleKind::PLAYABLE` and is not rendered in production lineups.
- It has distinct bake output and must satisfy the same mesh integrity, hitbox, mount, and budget
  tests as the real vehicles.

Assessment:

The prototype is useful as a simple control shape: box turret, straightforward hull, ordinary gun.
It should not be treated as photo validated. It is more valuable as a regression sentinel for the
procedural pipeline than as a content target.

Conclusion:

Keep it as a test baseline, but exclude it from historical photo-fidelity claims.

### T-54-3 obr. 1951

Status: best current photo-analysis implementation path.

Evidence:

- `VehicleBlueprint::for_vehicle(VehicleKind::T54_1951)` now returns a blueprint.
- The blueprint feeds hitbox, mount frames, and armor slopes.
- The recipe reads the same blueprint for hull, running gear, turret, and gun.
- Tests verify body containment, lower tub inset, wide sponson coverage, cupola seating, mantlet
  socket scale, and general fit/fill.
- Front and top-plan screenshots show the broad Soviet layout: low hull, round turret, centered gun,
  visible sponson and track separation.

Photo-backed cues currently represented:

- low, wide Soviet medium hull;
- sloped glacis tied to the armor slope;
- rounded cast turret and small cupola;
- front mantlet/socket integration;
- wider upper sponson over narrower lower tub;
- track run and road-wheel side read.

Risks:

- The current blueprint uses five visible road wheels, a continuous track belt, distinct idler and
  drive sprocket volumes, and no return rollers. These are now explicit T-54 benchmark gates, not
  a broad T-54/T-55 family assumption.
- The cast turret is still a procedural dome. It reads correctly from game distance, but it does not
  capture the exact asymmetric cast front/cheek mass of a museum-grade T-54.

Conclusion:

The T-54 path is architecturally correct. The next work should refine ratios and wheel count, not
replace the approach.

### T-55A

Status: legacy-compatible, not a production Forge benchmark.

Evidence:

- It remains in `VehicleKind` for wire/test compatibility.
- It may still bake for low-level compatibility tests.
- It is not in `VehicleKind::PLAYABLE`, not in the Forge `ReferencePack`, and not in production
  review lineups.

Photo-backed cues currently represented:

- low Soviet medium silhouette;
- rounded cast turret;
- cupola;
- D-10 family gun read with bore evacuator;
- track/wheel side detail.

Risks:

- Treating T-55A as a near-identical benchmark variant creates a clone problem and dilutes the
  module-level review of the T-54.
- T-55A/D-10T2S details, especially the bore evacuator, must not define the canonical T-54-3 obr.
  1951 silhouette.

Conclusion:

Do not migrate or promote T-55A as the next production benchmark in this pass. Keep it as
compatibility coverage unless a future content plan gives it a distinct, non-clone role.

### Tiger I

Status: broad silhouette is correct; suspension and detail fidelity are simplified.

Evidence:

- Global geometry tests pass.
- The recipe uses a near-vertical hull idiom, box turret, drum-like cupola, short 8.8 cm gun, and
  German heavy running gear constants.
- Profile screenshots show a tall, rectangular heavy tank distinct from Tiger II and Panther II.

Photo-backed cues currently represented:

- flat vertical heavy hull character;
- tall welded box turret;
- short 8.8 cm L/56 gun compared with Tiger II;
- heavy track band;
- cupola.

Risks:

- Tiger I side photos strongly communicate dense, overlapping/interleaved road wheels. The current
  geometry reads more like a dark track slab with wheels than a Tiger-specific suspension.
- The turret sides and front plate are still generalized. They communicate "box turret" but not the
  richer Tiger I plate layout.

Conclusion:

Good for current stylized gameplay readability. A Tiger I blueprint should prioritize suspension
mass, turret box proportions, front plate thickness read, cupola placement, and gun length ratio.

### Tiger II

Status: class/family read is good; turret and hull need photo-ratio pinning.

Evidence:

- Global geometry tests pass.
- Screenshots separate Tiger II from Tiger I through longer hull, stronger glacis, sloped turret,
  and longer gun.
- Project vehicle notes already ground gameplay data in public historical references.

Photo-backed cues currently represented:

- long German heavy hull;
- raked glacis;
- sloped/faceted production-turret approximation;
- long 8.8 cm KwK 43 gun;
- heavy track mass.

Risks:

- The turret is currently a low-poly wedge-like approximation. From reference photos, production
  Tiger II turrets have very recognizable front mass, side taper, and rear bustle. Those should be
  pinned by blueprint ratios.
- The current running gear is shared with other German heavies at a simplified level.

Conclusion:

The broad read is correct. The next fidelity step is not "more triangles" in general; it is better
ratio constraints for turret front, turret side taper, rear bustle length, glacis run, and gun
protrusion.

### Jagdtiger

Status: readable casemate tank destroyer; needs a blueprint because its silhouette is dominated by
one large superstructure.

Evidence:

- Global geometry tests pass.
- `VehicleKind::has_fixed_casemate` semantics keep turret yaw ignored.
- `VehiclePose` keeps casemate/gun pose stable under turret-yaw input.
- Side/profile screenshots show the fixed tall casemate, long chassis, and long 12.8 cm gun.

Photo-backed cues currently represented:

- Tiger II-like long chassis;
- tall fixed casemate;
- raked front superstructure;
- very long/fat gun;
- no traversing turret.

Risks:

- Jagdtiger side references make casemate height, front slope, mantlet mass, and hull length
  extremely visible. The current geometry communicates the class but does not yet encode those
  ratios as source-tracked data.
- Because the casemate is emitted through the turret submesh slot for rendering, tests must keep
  proving that gameplay yaw semantics do not accidentally turn it into a rotating turret.

Conclusion:

Jagdtiger is the best German candidate after the T-54 benchmark is genuinely good. It would prove that the
blueprint system handles non-turreted vehicles and casemate armor geometry cleanly.

### Panther II

Status: playable interpretation is visually distinct, but the historical/photo basis needs explicit
documentation.

Evidence:

- Global geometry tests pass.
- Screenshots show it as a sloped German medium/heavy hybrid: sharp glacis, compact turret, long
  gun, and lower profile than the Tigers.
- The Wikimedia Panther II photo reference itself is complicated: the captured/displayed vehicle is
  documented as having been turretless when captured and later shown with a Panther G turret.

Photo-backed cues currently represented:

- Panther-like sloped hull;
- compact turret;
- long gun;
- lower medium profile compared with Tiger I/Tiger II.

Risks:

- A photo-grounded Panther II recipe must state whether it models the surviving museum vehicle, the
  historical prototype hull, or a gameplay-plausible planned turret interpretation.
- Without that decision, "photo fidelity" is ambiguous: a visually plausible game vehicle may still
  not correspond to any single photographed configuration.

Conclusion:

Keep it as a playable variant, but document the interpretation before adding ratio tests. Panther
II should not be judged by the same "production vehicle photo" standard as Tiger I, Tiger II, or
T-54.

## Cross-Cutting Conclusions

1. The current system's strongest property is determinism. Every bake can be hashed and every
   submesh can be tested. That makes it safe to evolve recipes incrementally.
2. The current system's weakest property is evidence capture. Several recipes are visually plausible
   but do not yet encode where their proportions came from.
3. `VehicleBlueprint` is the right migration target. It is the clean bridge between photo analysis,
   gameplay hitboxes, armor slopes, mount frames, and visible meshes.
4. The screenshot pipeline is now good enough for repeatable human review. The seven-view lineup
   catches front, rear, side, top-plan, gameplay oblique, and pose-chain issues.
5. More visual detail should be ratio-driven, not decoration-driven. Add tests for stable silhouette
   measurements before adding more panels, bolts, or small fittings.

## Recommended Next Work

1. Finish the T-54 LOD0 close-up benchmark before adding adjacent Soviet variants.
2. Add a small "photo ratios" test module. Start with robust ratios:
   hull length/width, hull height/track height, turret width/hull width, gun protrusion/hull length,
   casemate height/hull height, and lower tub inset/upper sponson width.
3. Migrate Jagdtiger next to prove the blueprint supports casemate vehicles, fixed turret yaw, and
   casemate armor-front semantics.
4. Migrate Tiger I and Tiger II after that, using separate German-heavy blueprint helpers so Tiger I
   does not inherit sloped-heavy assumptions and Tiger II does not collapse into a generic wedge.
5. Keep `vehicle_lineup_views` as a repeatable review artifact. Run it before claiming a recipe is
   visually improved, and compare the generated PNGs against the reference set.
