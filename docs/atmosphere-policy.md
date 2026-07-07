# Atmosphere Policy

The game had world-class *construction* systems (procedural geometry kernels, a mesh-quality
contract, a deterministic sim) and almost no *presentation* systems. Everything from the mesh
onward — light, sky, image formation, surface narrative, motion body-language — was a flat constant
or missing, so vehicles read as clean grey CAD renders instead of armour under a sky. "Look",
"atmosphere" and "feel" are properties of the presentation layer, not of the mesh.

Atmosphere is that presentation layer for the *world image*: the light a scene is under, the sky it
sits in, the air between the camera and a target 1000 m away, and the tone curve that turns raw
radiance into a graded picture. It is a first-class, designable axis — time of day and weather are
profiles, not hardcoded constants.

This policy is the counterpart of `docs/vehicle-geometry-policy.md`: geometry answers *what shape is
this part*; atmosphere answers *what does this scene feel like*.

## Direction

- Outdoor-first image formation for armoured battles on large terrain maps. Bias every choice toward
  a low vehicle raked by a directional sun on open ground, read from close driving distance out to
  ~1000 m, not toward indoor or stylised-flat looks.
- Physically *motivated*, not physically *exact*: a hemispheric sky/ground ambient, one directional
  sun, height/distance fog with aerial perspective, and a filmic tone curve. Cheap, stable, and
  tunable — never a path tracer.
- Atmosphere is chosen per scene by a **profile** (dawn, midday, overcast, dusk, garage studio), the
  same way `SceneLighting` already swaps the battle look for the garage. Profiles are data.
- The renderer owns image formation. The backend-neutral profile (`renderer_api::SceneLighting` and
  its atmosphere fields) stays free of `wgpu`; it is turned into GPU bytes in exactly one place
  (`renderer_wgpu::CameraUniform::from_scene`).

## Lighting Model

The scene radiance for a surface with world normal `n` is:

```
radiance(n) = hemi_ambient(n) + key·max(dot(n, L_key), 0) + fill·max(dot(n, L_fill), 0)
                              + rim·max(dot(n, L_rim), 0)
hemi_ambient(n) = mix(ground_ambient, sky_ambient, saturate(n.y * 0.5 + 0.5))
```

- **Hemispheric ambient** replaces the former single flat ambient constant — the dominant cause of
  the flat look. Up-facing surfaces take the sky colour; down-facing surfaces take a warmer ground
  bounce. This alone *grounds* a vehicle in its field.
- **Key** is the directional sun: kept low and to the side (a raking morning/afternoon angle), so it
  sculpts the sides of a low hull instead of only lighting horizontal decks. Its colour may exceed
  `1.0` for HDR punch that the tone curve rolls off.
- **Fill** is a soft cool sky light from the opposite upper quarter; **rim** is a restrained back
  light that lifts the silhouette off the sky. The battle rim is now *on* (it was black).

Every `*_direction` points *towards* its light and is normalized in the shader; every `*_rgb` is a
linear colour/intensity. `ground_ambient_rgb` is the only new profile field for hemispheric ambient.

## Image Formation

- The lit result is HDR (specular and a bright sun can exceed `1.0`). A **filmic ACES-lite tone
  curve** maps it to display range so highlights roll off instead of clipping to white, and the
  picture reads as graded rather than as raw rasteriser output. This is the single biggest "mood"
  lever after the sun angle.
- The framebuffer is `*UnormSrgb`, so the hardware applies the linear→sRGB encode on store. Shaders
  therefore output **linear, tone-mapped** colour and must not also apply a manual sRGB `pow`.
- Later phases add a small exposure scalar and an optional grade (lift/gamma/gain or a 3D LUT) to the
  profile so a dusk look is a data change, not a shader edit.

## Sky And Air (phased)

- The flat clear-colour sky becomes a **gradient sky** (zenith→horizon + a soft sun disc/haze) so the
  upper hemisphere the ambient samples and the visible sky agree. A **drifting domain-warped FBM cloud
  sheet** breaks the dome out of a flat two-stop wash — anchored to the ray direction (world-stable,
  no swim), crawling only by the tick-domain presentation clock, lit toward the sun and greyed on the
  shadow side, faded out at the horizon band. Coverage is soft-thresholded so open blue shows between
  the banks, and the same clouds grey down into an overcast lid under the rain profile.
- **Height + distance fog with aerial perspective**: distant terrain and vehicles desaturate toward
  the horizon/sky colour, giving a 1000 m map real depth instead of cardboard cut-outs at range. Fog
  is a profile parameter (density, colour, height falloff), evaluated in the lit shaders from the
  reconstructed world position — no separate pass in the first cut.

## Boundaries

- `renderer_api` owns the backend-neutral atmosphere **profile** (`SceneLighting` + atmosphere
  fields) and must not depend on `wgpu`.
- `renderer_wgpu` owns the GPU uniform, the shared lighting/tone-map WGSL, the (later) sky and
  shadow passes. The camera+lighting uniform stays the single shared group-0 binding for both the
  scene and vehicle pipelines; its field order is mirrored in both WGSL `Camera` structs and locked
  by `wgsl_layout`.
- No atmosphere rule depends on simulation state, wire snapshots, or window events. Time-of-day and
  weather are render-side profiles; they never touch armour truth, hitboxes, or replay state.
- Surface narrative (edge wear, cavity dirt, weld beads — the "Surface Field" system) is a **separate
  policy**. Atmosphere lights the surface; it does not author it. The two meet at the vehicle vertex,
  which gains an ambient-occlusion channel so per-vehicle contact AO reaches the lit shader.

## Tests And Gates

- `wgsl_layout`: the camera+lighting uniform encodes at the WGSL std140 size, and both shaders parse
  and expose `vs_main`/`fs_main` with the camera bound at group 0.
- A `SceneLighting` profile test locks the load-bearing invariants: the battle profile has a live rim
  and a ground ambient distinct from the sky ambient (hemispheric ambient is real, not degenerate).
- Offscreen render-frame tests keep asserting a lit, non-empty scene; the vehicle darkened-albedo
  test stays a *relative* check so it survives tone mapping.
- Human review: `t54_views` / `vehicle_lineup` offscreen screenshots under the battle profile.

The canonical gate remains `./scripts/verify.ps1`.

## Phase Plan

1. **Sky light + filmic image** (this policy's first cut): hemispheric ambient, a raking retuned sun,
   a live rim, and the ACES-lite tone curve, shared by the scene and vehicle shaders. Data + shader
   only; one trailing uniform field. *(implemented)*
2. **Sky gradient + fog/aerial perspective**: gradient sky to match the ambient hemisphere; profile
   fog with aerial perspective for 1000 m depth.
3. **Directional shadow map** (one cascade to start): the step that turns "procedural boxes" into "a
   vehicle on a field". Contact shadows seat hatches, the gun, fenders and tracks.
4. **Weather / time-of-day profiles + exposure + grade**: dawn / midday / overcast / dusk as data;
   exposure and an optional LUT grade on the profile.

Colour-space follow-up (tracked, not phase-gated): vehicle albedo maps upload as `Rgba8Unorm`; if the
Forge bakes them in sRGB they must upload as `Rgba8UnormSrgb` (or be de-gamma'd) so materials are not
gamma-bright. Verify against the Forge bake before flipping.
