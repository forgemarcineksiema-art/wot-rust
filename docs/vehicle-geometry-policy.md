# Vehicle Geometry Policy

Vehicle models are gameplay-readable silhouettes first and decorative assets second. The project
targets stylized realism in the spirit of early 2010s tank games: clear proportions, readable armor
plates, recognizable turrets and casemates, and enough surface depth to stop vehicles from reading
as boxes, without requiring glTF assets or texture authoring for the first playable slice.

## Direction

- Build a small procedural vehicle-geometry system instead of expanding ad hoc box generation.
- Keep glTF as a later optional import path, not as the foundation for current vehicle visuals.
- Optimize for distinct silhouettes, mount-aware animation, hitbox honesty, and stable runtime cost.
- Prefer low/mid-poly forms with strong normals, panel tones, and baked vertex color over high-detail
  meshes that obscure gameplay or exceed the renderer budget.

## Ownership

The `vehicle_geometry` crate (`crates/kernels/vehicle_geometry`) is a pure geometry kernel: mesh
types, builders, and the mesh-quality audit. Its manifest declares only `earcut`, `game_core`,
`glam`, `serde`, and `thiserror` — no renderer, no sim, no upward dependencies. Vehicle recipes,
the fleet budget envelope, and the golden bake hashes moved up to `crates/vehicle/vehicle_recipes`
(#416); the T-54 hybrid's part description lives in `crates/vehicle/vehicle_build`.

`client` selects recipes for snapshots and adapts baked geometry to the current render path.
`renderer_api` owns handles, frame objects, and backend-neutral render contracts. `renderer_wgpu`
owns GPU buffers, shader layouts, uploads, materials, instancing, and draw submission.

## Mesh Contract

A baked vehicle is split into local-space submeshes:

- `hull`: hull, tracks, wheels, fenders, hull greeble, and fixed casemate body when present.
- `turret`: rotating turret, mantlet, cupola, hatch details, and turret greeble.
- `gun`: barrel, muzzle brake, bore cue, and recoil sleeve.

Each submesh carries local bounds, triangle/vertex counts, material tags, smoothing groups, and a
stable mount frame. Turret submeshes sit on a turret-ring frame. Gun submeshes sit on a trunnion
frame. Casemate vehicles keep turret yaw ignored and attach gun elevation to the fixed casemate
frame.

The first output format should be renderer-neutral. Conversion to `SceneVertex` is an adapter step
while the current dynamic mesh path exists. Static `MeshHandle` registration and instanced
`RenderObject` emission are the intended runtime path.

## Procedural Kernel

The kernel should support a compact set of operations that are expressive enough for tank shapes:

- `loft` or `sweep` between cross sections for hulls, casemates, and cast turret caps. Swept
  sections must be convex; the kernel asserts this at bake time rather than shipping
  self-overlapping caps.
- `revolve` for barrels, mantlets, cupolas, road wheels, drive sprockets, and idlers.
- `chamfered_prism` for rolled-plate hulls, turrets, hatches, and fenders.
- `mirror` for left/right vehicle symmetry.
- `array` for repeated wheels, rollers, teeth, bolts, and track cues.
- `weld` and smoothing-group normal generation for cast surfaces and hard armor seams.

Avoid broad CSG, skeletal animation, runtime procedural mutation, or texture/UV requirements in the
first implementation. Those can come later behind explicit policies and layout tests.

## Recipe Contract

Recipes are typed Rust builders first. They should be easy to move into a data format later, but
JSON should not be the initial source of truth for shape authoring.

Each vehicle recipe must declare:

- family style, such as Soviet low cast turret, German vertical heavy, German sloped heavy, or fixed
  tank destroyer casemate.
- hull side profile, top profile, track plan, turret or casemate plan, gun plan, and material roles.
- mount frames for hull origin, turret ring, gun trunnion, and muzzle.
- gameplay fit rules against `HitboxProfile`.
- triangle and vertex budgets.

The first vertical slice used the since-removed `T55A`, but the current Forge benchmark is `T54_1951`. It proves the hardest useful path for this style: low
hull, rounded cast turret, small cupola, long 100 mm gun, visible road wheels, and tracks that read
as more than side boxes.

## Surface Style

The first surface pass uses the existing vertex format:

- smoothing groups for cast turret parts, wheels, mantlets, and barrels.
- hard edges for welded plates, fenders, hatches, and armor seams.
- baked vertex-color occlusion cues in contact areas: under fenders, between road wheels, at the
  turret ring, behind the mantlet, and near track recesses.
- per-material color roles for rolled armor, cast armor, rubber, dark track metal, barrel steel,
  hatches, and subtle dirt.
- team or ownership color as a tint layer, not the vehicle's identity.

Vertex-format growth is append-only, and a new lane lands WITH its instruments in the same change:
a layout test (`renderer_wgpu/tests/wgsl_layout.rs` plus the `SceneVertex` size assert) and a
comparison golden proving existing content unchanged. The UV lane (Imported Flora 2.0, FL-1)
shipped exactly this way — appended at location 12, `[0, 0]` everywhere for procedural content —
and it is the standing precedent for any future lane (texture arrays, normal maps): never a
reorder, never a lane without a lock.

## Runtime Path

The dynamic mesh path is acceptable only for the first proof slice. It rebuilds and uploads all tank
geometry each frame, so it must not become the long-term home for richer vehicles.

The intended path is:

1. Bake each `VehicleKind` recipe once at startup or asset-generation time.
2. Register baked submeshes as stable mesh handles owned by the renderer backend.
3. Emit render objects for hull, turret/casemate, and gun using snapshot transforms.
4. Group by mesh and material, then draw with instancing.
5. Keep shells and other transient effects on separate lightweight paths.

This keeps geometry cost stable as tank count grows and matches the renderer upload policy.

Both render paths — the legacy dynamic mesh build and the instanced objects — pose parts through
one shared chain (`client::vehicle_pose::VehiclePose`: hull yaw about the origin, turret yaw about
the ring, gun pitch about the trunnion, casemates holding yaw). A drift-lock test compares their
world-space vertices, so the paths cannot quietly disagree while the dynamic path still exists.

## Tests And Gates

Every geometry phase needs executable checks:

- finite vertices, valid indices, outward winding, and normalized normals.
- body fit/fill against `HitboxProfile`, excluding gun length by design.
- turret fit/fill against the gameplay turret plan box (`HitboxProfile::with_turret_plan`), so
  the two-volume hit model in `sim::shell_trace` stays honest against the visible turret.
- turret and gun mount transforms for yaw and pitch.
- casemate vehicles ignoring turret yaw.
- per-vehicle silhouette uniqueness beyond raw box dimensions.
- vertex and triangle budgets per submesh and per full vehicle.
- deterministic bake hashes for recipe output.
- photo-reference audit for each authored recipe: at least side/front or three-quarter sources,
  the visible silhouette cues taken from them, and any intentional gameplay/stylization deviations.
- offscreen `vehicle_lineup` screenshots for human visual review.

The canonical gate remains `./scripts/verify.ps1`. Narrow development loops may run focused crate
tests first, but no implementation phase is complete until the full gate passes.

## Phase Plan

1. Document and enforce this policy.
2. Add the `vehicle_geometry` crate with the kernel, neutral mesh types, and unit tests.
3. Build the canonical `T54_1951` benchmark through the current baked render path. (`T55A` was later removed outright — the roster carries no clones.)
4. Add smoothing groups and baked vertex-color surface treatment.
5. Move rich vehicle geometry to mesh handles and instanced render objects before rolling out all
   vehicles.
6. Add recipes for the remaining vehicles using shared family components.
7. Consider optional UV/textures or glTF import only after the procedural path is playable.
