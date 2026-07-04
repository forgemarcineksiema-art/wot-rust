# Shadow Policy

Phase 3 of `docs/atmosphere-policy.md`: the directional sun casts shadows. Without them, procedural
vehicles read as objects floating *near* the ground rather than *on* it — no cast shadow grounds the
hull, no self-shadow seats the turret on the hull or the hatches in the roof. Cast contact shadow is
the single strongest "it is really there" cue.

## What we optimise for

The valuable shadows here are **contact shadows** — the vehicle's own shadow on the ground and its
self-shadowing — not distant-terrain self-shadowing. On a gentle rolling heightmap there are few
sharp far occluders, and aerial perspective (a later atmosphere phase) hazes the far field anyway.
So this is a **focused single shadow map** tightly fitted around the action, not a full-map cascade.

## Model

- **One orthographic shadow map, sun-aligned, focused on a bounded box** (`SunShadowParams`) centred
  on the camera subject. A 2048² map over a ~32 m half-box is ~3 cm/texel — crisp on hatches and the
  gun. Far vehicles cast nothing, but they are small and (soon) hazed; nobody looks at their ground
  shadow. Cascades (CSM) are a later upgrade *only if* the far field ever demands it.
- **Texel-snapped light matrix** (`sun_light_view_projection`): the projected focus centre is rounded
  to the shadow-texel grid so the shadow edge does not shimmer/crawl as the battle camera pans. This
  is the non-obvious step that separates stable shadows from sparkling ones; it is unit-tested
  without a GPU.
- **Only the key (sun) light is occluded.** Ambient (sky/ground hemisphere), fill and rim are
  indirect and stay lit: `radiance = hemi_ambient + shadow·key + fill + rim`, and the key-driven
  specular is shadowed too. Shadows deepen form without crushing the image to black.
- **Normal-offset + a small depth bias** to kill acne without peter-panning; a **3×3 PCF** tap for
  soft contact edges (hard shadows read "gamey").

## Occluders (this phase)

The shadow pass renders the **vehicles** as occluders — the primary casters (ground shadow +
self-shadow). Terrain and vehicles both **receive** (sample the map) in the main pass. Terrain
casting terrain (ridges) is deferred: low value inside a focused box on gentle ground, and it keeps
the pass to one depth pipeline. It is a clean follow-up when a map needs it.

## Boundaries

- `renderer_api` owns the backend-neutral light matrix + params (`sun_shadow`), pure `glam`, no
  `wgpu`. It sits beside `view_projection_matrix` and `SceneLighting`.
- `renderer_wgpu` owns the shadow depth target, the depth-only occluder pipeline, the shadow bind
  group (a `texture_depth_2d` + comparison sampler at group 2 of both the scene and vehicle shaders),
  and the pass. `light_view_proj` and the shadow params ride the one shared camera uniform, built in
  the single `CameraUniform::from_scene` place.
- Shadows are render-only: no simulation, snapshot, hitbox, armour or replay state depends on them.
  This frees fast iteration and keeps determinism untouched.

## Capability tiering

Per `docs/wgpu-capability-model.md`, shadow resolution / PCF scale by adapter tier, and the weakest
tier falls back to **no shadow** — a 1×1 lit dummy map bound with `strength = 0`, so the bind groups
and shaders stay valid on every tier with no branching in the pipeline layout. The mechanism (a
`shadow_params.strength` knob and a swappable resolution) ships here; automatic tier probing is a
tracked follow-up.

## Tests and gates

- unit (no GPU): `sun_light_view_projection` snaps the focus centre onto the shadow-texel grid
  (anti-shimmer), and maps the focus centre near the shadow-map centre.
- render-frame (GPU): the ground directly under a vehicle is darker than open ground (a cast shadow
  exists); a `strength = 0` render matches the pre-shadow look (fallback is a true no-op).
- `wgsl_layout`: the camera uniform encodes at its new std140 size with `light_view_proj` +
  `shadow_params`; both shaders parse with the shadow bind group at group 2.

The canonical gate remains `./scripts/verify.ps1`.

## Then: one lit pipeline

Sampling the shadow map forces the same code into both `scene.wgsl` and `vehicle.wgsl`. That
duplication is the agreed trigger to unify the two pipelines behind one lit shader / one vertex
format with optional channels — done **after** this phase, so the visible win ships first.
