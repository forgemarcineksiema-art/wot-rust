# Shadow Policy

Phase 3 of `docs/atmosphere-policy.md`: the directional sun casts shadows. Without them, procedural
vehicles read as objects floating *near* the ground rather than *on* it — no cast shadow grounds the
hull, no self-shadow seats the turret on the hull or the hatches in the roof. Cast contact shadow is
the single strongest "it is really there" cue.

## What we optimise for

The valuable shadows are, first, **contact shadows** — the vehicle's own shadow on the ground and
its self-shadowing — and second, the **far field grounding at all**. The original single focused
box optimised only for the first; on a 1000 m map everything past its ~128 m footprint was flat,
hillsides never raked into shade, and the far town floated — exactly the "later upgrade only if the
far field ever demands it" case. The lighting 2.0 program cashed that in: this is now a **two-
cascade** setup — the same crisp focused near box, plus one wide, cheap far cascade for terrain and
statics.

## Model

- **Near cascade: one orthographic shadow map, sun-aligned, focused on a bounded box**
  (`SunShadowParams`) centred on the play field. **The shipped map is 2048²** — the one-look policy
  (2026-07-14) gives every adapter `LightingQuality::canonical()`, so the per-tier split this
  document was written under is gone and the 4096² figure survives only in the dev-only `rich()`
  profile. Over the battlefield's 64 m half-box that is **6.25 cm/texel**.
- **The box is sized by the SCENE, not by the GPU** (`ShadowResources::cascades`). The resolution
  is fixed for everyone; where its texels are aimed is not. The battlefield keeps the 64 m half-box
  pushed 40 m along the look; the garage asks for 30 m (`hangar_shadow_radius_m`), because a 36 m
  hall inside a 128 m box received 7.9% of the map — 576 of 2048 texels across — and staircased
  every skylight shaft at 8.4 screen pixels per texel on a 14 m hero boom. Same texture, same
  memory, same frame cost; 2.9 cm texels instead of 6.25. The smallest box that still contains the
  whole hall under every garage rig is 28.3 m, and a box tighter than the room trades the
  staircase for a cascade seam across the walls — both halves are locked by
  `the_near_shadow_box_contains_the_whole_hall`.
- **Far cascade** (`SunShadowParams::far_cascade`): a 4.5× box (288 m half-size) at half the
  resolution, centred ~200 m along the look, in its **own depth texture** (not an array layer —
  the two maps run at different resolutions, and array layers must share a size). Casters:
  terrain, the dynamic mesh and static scene meshes only — **no vehicle fleet**. At ~0.56 m/texel
  a tank's shadow does not resolve, and vehicles beyond the near box cast nothing before either,
  so the far pass stays nearly free while hillsides and the far town finally ground. Selection is
  by **containment, not split distance**: a fragment whose near-box UV sits inside a small margin
  samples the near map; everything else falls through to the far map (2×2 PCF), whose
  softness sits out where the aerial haze lives. A third cascade is deliberately absent — past the
  far box the 400 m fog fairness band owns the image. `WOT_SHADOW_CASCADES=1` drops back to the
  single near box, byte-for-byte the pre-cascade lookup.
- **Texel-snapped light matrix** (`sun_light_view_projection`): the projected focus centre is rounded
  to the shadow-texel grid so the shadow edge does not shimmer/crawl as the battle camera pans. This
  is the non-obvious step that separates stable shadows from sparkling ones; it is unit-tested
  without a GPU.
- **Only the key (sun) light is occluded.** Ambient (sky/ground hemisphere), fill and rim are
  indirect and stay lit: `radiance = hemi_ambient + shadow·key + fill + rim`, and the key-driven
  specular is shadowed too. Shadows deepen form without crushing the image to black.
- **Normal-offset + a small depth bias** to kill acne without peter-panning. The offset scales with
  the frame's texel footprint, so it follows a scene-sized box down; at ~50 cm texels it grows
  large enough to push a receiver past a caster 1.2 m above it and the shadow disappears, which is
  the practical floor on how coarse a box may get.
- **PCF: the shipped tier runs four taps at ±1 texel**, the dev tier nine at ±1 (`PCF_WIDE` is not
  in `canonical()`'s detail mask). Each tap is a hardware bilinear comparison covering 2×2 texels,
  so the four sweep the same ±2 texel footprint the nine do: the reduced tier buys the wide
  kernel's SOFTNESS without its tap count. Softness is not a luxury here — a sun 0.53° across
  throws a penumbra widening ~0.9 cm per metre of blocker distance, so a hard edge is wrong, and a
  soft edge is also what stops a 6.25 cm texel from reading as a staircase. Both kernels must
  **straddle** the fragment; the reduced one used to tap `{0, 1}`, putting its centroid half a
  texel up-right and dragging every near-cascade shadow edge with it. Locked by
  `the_pcf_kernel_is_centred_on_the_fragment`, which mirrors the scene and measures 0.01 px of
  disagreement centred against 4.39 px off-centre.

## Occluders

The shadow pass renders the **whole world** as occluders: the static scene buffer (terrain, plus the
buildings and trees baked into it), the dynamic mesh, and the vehicle fleet. Everything both casts
and receives. This is the upgrade the maps demanded — a town of buildings floating shadowless read as
a paper diorama, and gentle terrain wants its hillsides to rake into shadow under a low sun. Two
depth pipelines (one per vertex stride, scene vs vehicle) share one `vs_main`; the pass runs whenever
shadows are on, no longer gated on a vehicle being present.

Because the static world is an **open** surface (its sun-facing side is its front face) the pass draws
with **no face culling** and leans on a slope-scaled hardware depth bias plus the shader normal
offset to hold off acne — the front-face cull that suited closed hulls would have dropped exactly the
terrain/ roof casters we now want.

The focused box is sized (±64 m) and **pushed forward along the chase camera's look** so its ~128 m
footprint straddles the near/mid combat field, not the empty ground behind the camera; studio shots
pin it to the subject instead. Drawing the full terrain buffer into the map (only the box's slice
matters) is a known cost — tile/box culling of the caster set is the tracked perf follow-up, aligned
with the world-tiling lever in the perf plan.

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
tracked follow-up. (The tier split itself is superseded by the one-look policy — see Model above.)

## Deliberately not shipped

**Contact-hardening (PCSS)** — a blocker search that widens the penumbra with blocker distance, so
a track's contact shadow stays tight while a canopy 10 m up goes soft. It is the physically right
answer and the next real step; it is not here because it costs a second set of taps per fragment
and the one-look policy raises a budget **per item, with a measurement on the min spec** — which
this machine cannot produce. The cheap half of its benefit is already taken: the shipped kernel now
carries the wide kernel's reach at four taps, which is what removes the staircase. Buy the rest
when there is an MX330 number, not before.

**Turning shadow `strength` down to make shadows gentler** — rejected on sight. It greys the shade
instead of softening it, reads as fake, and pushes the frame-wide dark-mass debt (D4) the wrong
way. Softness belongs in the kernel and the shade's floor belongs in the grade's toe (D27); neither
is a reason to stop the sun being blocked.

## Tests and gates

- unit (no GPU): `sun_light_view_projection` snaps the focus centre onto the shadow-texel grid
  (anti-shimmer) for **each cascade's own grid**, and maps the focus centre near the shadow-map
  centre; every near-covered point lands strictly inside the far box (the handoff contract); a
  point 250 m out misses the near box and lands in the far one (the added capability).
- render-frame (GPU): the ground directly under a vehicle is darker than open ground (a cast shadow
  exists); a static occluder **200 m past the shadow focus** darkens the ground beneath it —
  impossible with the single box; a `strength = 0` render matches the pre-shadow look (fallback is
  a true no-op).
- executable budget: shadow memory is locked to exact bytes in `quality.rs` tests — the shipped
  canonical profile at 2048²+1024² = 20 MB, the dev-only `rich()` at 4096²+2048² = 80 MB. These
  were the *integrated* and *discrete* tiers before the one-look policy; everyone now renders the
  canonical one. Moving either is a deliberate diff. Note the scene-sized focus box costs nothing
  here: it re-aims the same texels.
- `wgsl_layout`: the camera uniform encodes at its new std140 size with both cascade matrices +
  `shadow_params`/`cascade_params`; both shaders parse with the shadow bind group at group 2.

The canonical gate remains `./scripts/verify.ps1`.

## Then: one lit pipeline

Sampling the shadow map forces the same code into both `scene.wgsl` and `vehicle.wgsl`. The shared
WGSL half of that is **done**: the camera struct, the lighting model/display transform and the
shadow/SSAO lookups now live in composed common fragments (`shader_library.rs` — one copy, locked
by a dedup test), which is what let the cascade lookup land as a single edit. Unifying the two
pipelines/vertex formats themselves remains open, pending a need.
