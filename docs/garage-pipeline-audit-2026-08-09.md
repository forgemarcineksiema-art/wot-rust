# Garage pipeline audit — 2026-08-09

The companion to [geometry-pipeline-audit-2026-08-08.md](geometry-pipeline-audit-2026-08-08.md),
which audited how a VEHICLE is built. This one audits the room it is parked in, and the frame the
two of them make together: geometry, materials, the bake, the light rig, shadows, anti-aliasing,
and the gates that are supposed to catch all of it.

Every number states the command that produced it. Where a conclusion is a judgement call it says
so. `docs/ROADMAP.md` carries the standing verdict this audit answers to — *"the garage owes its
rebuild"* — and this is the measured version of that sentence.

The short form: **the garage's geometry stack is honest and its light rig is disciplined; what is
missing is that almost none of the laws the fleet obeys have reached the room, and the gate cannot
ask for them.** Five of the twelve findings below are a law that exists, is applied elsewhere, and
was never pointed at the hangar.

---

## 1. The five layers

| layer | where | what it does |
|---|---|---|
| room geometry | `scene_build/src/hangar.rs`, `hangar_gallery.rs`, `hangar_props.rs` | ~500 axis-aligned boxes (`slab`) and 20 cylinders (`push_cylinder`), all procedural, all `SceneVertex` |
| bake | `hangar_bake.rs` | conformal subdivision to a 2.2 m edge → BVH → 16 rays/vertex, one bounce plus emission → the `bounce` lane |
| upload | `client/src/app/garage_render.rs:112` `ensure_scene` | the hall rides the TERRAIN slot (80 m chunks); water/dressing/ground cleared; interior background; shadow focus + radius; `bloom_mips(3)` |
| the hero | `asset_render.rs` → `pbr_mesh.rs` → `vehicle.wgsl` | the LOD0 bake, `GearDetail::Near`, 12 material roles over 8 texture layers, triplanar or parametric per vertex |
| the frame | `renderer_wgpu/src/frame_graph.rs` | ShadowNear → ShadowFar → SSAO×3 → Scene → Bloom → Post → FXAA (+HUD) |

Two shaders form the garage picture: `scene.wgsl` for the hall and `vehicle.wgsl` for the tank.
**They are two different lighting models in one frame**, and the seam between them runs exactly
along the edge of the subject — see G5.

## 2. Measurements

The hall's mesh, from a throwaway `scene_build` example over `hangar_scene_mesh()` (release):

```
vertices              17 771        triangles       13 374
vertex buffer         1 249 KiB     indices         40 122
cold build            1 902 / 1 919 / 1 940 ms   (three runs)
subsequent call       0.4 – 0.5 ms  (OnceLock + clone)
vertices with bounce  7 812 (44.0%)       peak bounce   3.28
emissive vertices     120  (five lamp faces)
gloss == 0            12 463 / 17 771  =  70.1%
surface role          17 771 × 0.0 (LEGACY)  =  100%
triangles under a 3 cm equivalent edge   524
```

The locked hero frame, from `cargo test -p client --test look_goldens -- --nocapture` (960×540,
the always-on CPU half — it decodes the committed PNGs, no GPU needed):

| frame | dark | mid | bright | p05 | p50 | p95 | spread | sat | local |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `garage_hero` | **80.5%** | 18.5% | **1.0%** | 0.019 | 0.086 | 0.586 | 0.567 | 0.163 | **0.0047** |
| `garage_screen` | 84.8% | 13.8% | 1.4% | 0.039 | 0.092 | 0.573 | 0.535 | 0.149 | 0.0080 |
| `garage_tech_tree` | 82.3% | 17.0% | 0.8% | 0.041 | 0.095 | 0.581 | 0.540 | 0.152 | 0.0049 |
| `garage_option_list` | 86.3% | 12.5% | 1.2% | 0.039 | 0.092 | 0.510 | 0.472 | 0.151 | 0.0082 |
| darkest outdoor frame (`bystra_dawn_fog`) | 55.0% | 18.5% | 26.5% | 0.050 | 0.220 | 0.747 | 0.697 | 0.375 | 0.0062 |

`local` 0.0047 is **the lowest local contrast of all twenty-four locked frames in the game** (the
outdoor set runs 0.0052–0.0144). The garage is the least detailed picture the player is shown.

Hero against room, measured over the committed `garage_hero.png` with a ~540×168 px box centred on
the frame (the parked T-54 subtends ~8.0 m × 2.5 m at ~67 px/m under the hero rig):

```
HERO BOX   mean 0.195   p50 0.120   p95 0.5805   dark 72.9%
ROOM       mean 0.156   p50 0.078   p95 0.5860   dark 82.1%
```

The hero is 1.5× the room at the median and **is not the brightest thing in the frame** — the
room's p95 is not beaten. 73% of the hero's own box sits below the 0.25 dark threshold.

**Caveat, stated because the number is load-bearing:** that crop is derived from the camera
geometry, not from a render mask, so it carries turntable, floor and background. The direction is
solid, the exact figures are approximate. The honest instrument is a subject crop in the harness,
which is G11.

## 3. The findings

### G1 — Entering the garage is a ~1.9 second frozen frame

`ensure_scene` calls `hangar_scene_mesh()` **synchronously inside the render frame**
(`garage_render.rs:129`). The app is born in `SceneKind::Battle` (`app/mod.rs:789`), so the first
garage entry pays the whole bake.

The comment two lines above it reads *"The hangar mesh stays cheap enough to bake on entry"*.
Measured: 1.9 s in release on a desktop CPU. The same comment records that the battlefield got its
app-lifetime cache for exactly this reason — *"rebaking the full 1000 m battlefield inside the
transition frame froze it for hundreds of ms on iGPU laptops"*. The hangar is an order worse and
got nothing. The mechanism is five lines above in the same function: `poll_map_prebake()` already
prebakes a map speculatively on the garage's spare cores.

The guard test `the_hangar_bakes_cheap` counts VERTICES, and its own comment (*"The hall re-bakes
on every garage entry"*) is false — the `OnceLock` bakes once. **The budget measures a proxy, and
the proxy is not the cost.** This is the third recorded instance of measuring a proxy instead of
the resolution path.

### G2 — The whole hall runs with no material treatment at all

All 17 771 vertices carry `surface = 0.0`, so `surface_treatment()` **never executes** in the
garage. What runs instead is the interior arm of `material_detail` (`scene.wgsl:140`):

```wgsl
let p = world.xz * 1.3 + vec2<f32>(world.y * 0.7, world.y * 0.4);
return 0.955 + value_noise(p) * 0.07;      // ±3.5%, ONE octave, ~0.77 m period
```

and `detail_normal()` early-outs for interiors (`scene.wgsl:255`), so there is **no normal
perturbation whatsoever**. Concrete, painted sheet steel, workbench timber, rubber, tarpaulin,
crates and machined deck plate all resolve to one flat albedo times a 3.5% wash.

`docs/art-direction-policy.md` rule 5 asks for exactly two octaves on every surface — a 2–5 m macro
and a 0.3–0.6 m micro. The garage carries one octave at 0.77 m and 3.5% amplitude. The world has
PLASTER, PLANK, SLATE, BARK, dressed stone and natural rock roles built and shipping. **The garage
uses none of them.** Judgement, stated as such: this is the largest single reason the hall reads as
plastic, and it is not the geometry's fault.

### G3 — Seven tenths of the hall answers light with no specular at all

`SceneVertex::new` sets `gloss = 0.0`, and `slab()` / `push_cylinder()` use it. `slab_finished`
(the gloss-carrying variant) reached the floor (0.08), the lower walls (0.12), the girt rail (0.3),
the turntable deck (0.35) and a handful of rails. Everything else — roof, upper wall band, ribs,
trusses, the bay gate frame, the gantry crane, the workbench, crates, the stores rack, timber,
cable trays, signage — is gloss 0, and `scene.wgsl:346` (`if (gloss > 0.001)`) skips the entire
specular block *and* the environment reflection for them. In a workshop lit by lamps, that is
precisely the dead look.

### G4 — Metal in the garage mirrors a sky that is not there

`env_sky()` interpolates `sky_horizon_rgb` → `sky_zenith_rgb`. For `garage_hero` those are
**0.17/0.175/0.20 → 0.12/0.125/0.14**. What the roof openings actually show is
`INTERIOR_BACKGROUND = 1.30/1.38/1.55`.

A **7–10× mismatch**. The turntable deck (gloss 0.35), the rails, and every painted surface on the
hero (`vehicle.wgsl:450`) reflect a dim overcast night while standing under a daylit opening. The
comment beside those fields says they exist so *"the uniform stays well-formed"* — but they are
load-bearing for every reflection in the room.

### G5 — The room has GI; the hero does not

`SceneVertex` carries the `bounce` lane; `VehicleVertex` does not (`renderer_api/src/vehicle.rs:45`).
`scene.wgsl:343` adds `lit += input.bounce`; `vehicle.wgsl` has nothing to add.

So 44% of the hall's vertices carry baked indirect light peaking at 3.28 HDR, and the one object
the frame exists to sell receives `hemi_ambient + key + fill + local_pools` and nothing else. The
seam between the two lighting models runs along the silhouette of the subject. The policy says
*"The hero is the brightest, most contrasted, most detailed thing in frame"*; §2 measures that it
is not.

The 2026-08-08 audit already recorded the arithmetic that makes the fix viable: vertex-baked
occlusion **fails on world geometry and would work on vehicles**, because 21 508 triangles on a 6 m
hull puts vertices centimetres apart. The same argument transfers to a bounce lane.

### G6 — The GI bake resolves at 2.2 metres

`MAX_EDGE_M = 2.2` (`hangar_bake.rs:32`), `RAY_COUNT = 16`. The skylight shaft on the floor is about
6.4 m wide and is resolved by roughly three vertices across it. This is the same physics that
killed vertex AO on the world and is recorded as a negative result — *"vertex-baked shading has
nowhere to live on walls whose vertices are metres apart"*. Here the result is not zero, because
the emitters are bright; but the gradient is low-frequency by construction, and 56% of vertices
gathered nothing at all.

Separately: the bake hard-codes one profile (`hangar_bake.rs:360`, `SceneLighting::garage_hero()`)
into a process-wide `OnceLock`, so `garage_workshop` and `garage_studio` render a room whose
indirect light was computed for a different rig. Harmless today — only the hero preset ships — and
a silent trap the moment a second garage look does.

### G7 — Two laws the fleet obeys and the hall has never heard of

**The bevel law.** `solid::chamfer` (`kernels/solid/src/lib.rs:25`) states the widths — MACHINED
1 mm, ROLLED_PLATE 3 mm, FLAME_CUT 6 mm, CAST 20 mm — on the argument that *"a perfectly sharp
edge, having no such face, is the most reliable tell that a shape came out of a computer"*.
`push_oriented_box` (`scene_build/src/tank_mesh.rs:30`) is six quads with no chamfer. **Every one
of the hall's ~500 boxes has a mathematically sharp edge.** No arris, no bright line under a
raking key.

**The roundness law.** `game_core::roundness::segments_for_radius`, tolerance 2.8 mm.
`scene_build` never imports it, although it already depends on `game_core`. The hall types its
segment counts by hand:

| part | r [m] | segments given | the law asks | silhouette error |
|---|---:|---:|---:|---:|
| turntable hub | 0.90 | **24** | 40 | **7.7 mm** |
| prop contact ring | 0.485 | 18 | 29 | 7.4 mm |
| spare road wheel | 0.405 | 20 | 27 | 5.0 mm |
| 200 l drum | 0.29 | 18 | 23 | 4.4 mm |
| wheel hub | 0.15 | 14 | 17 | 3.8 mm |
| extinguisher | 0.11 | 14 | 14 | 2.7 mm ✓ |
| turntable deck / rim | 5.2 / 5.55 | 48 | 48 (the cap) | 11.1 / 11.9 mm |

And the sharper half: that 2.8 mm tolerance is calibrated as **a quarter of a pixel at 25 m**. The
garage looks from 14 m at rest and 5 m at the close boom, where a pixel covers ~2.2 mm. The hub
under the tank, dead centre of frame, misses by 3.5 px. **The garage needs a tighter tolerance than
the fleet and is given a looser one.**

This is the fifth recorded instance of the same pattern: a good decision applied once instead of
becoming a rule.

### G8 — The shadow test measures a resolution the game does not ship

`the_near_shadow_box_contains_the_whole_hall` (`hangar.rs:703`) builds its params from
`SunShadowParams::default()`, whose resolution is **4096** → 14.6 mm per texel, and asserts
`texel_world_size() < 0.03`.

The game ships `LightingQuality::canonical().shadow_resolution = 2048` → **29.3 mm per texel**,
sitting exactly on that bound. The test carries 2× of headroom the game does not have; drop the
shipped resolution to 1024 and the test stays green at 4096 while the player gets 58 mm.

Second, and bigger: **in the garage the far cascade is pure waste.** near = 30 m, far = 30 × 4.5 =
135 m at 1024 → a 270 m box at 264 mm per texel thrown around a 36 m room, drawn as a full pass
every frame to contribute nothing. Meanwhile the near box was stretched to 30 m *to contain the
hall*, which costs the hero the resolution it exists for. Swapping the roles — near box on the
turntable, hall on the far cascade — is available at no cost and worth roughly 4× on the contact
shadow. `PCF_WIDE` is also absent from the canonical mask, so this runs 2×2 PCF over 29 mm texels.

### G9 — The instrument multisamples; the game does not

`review_sample_count() = 4`, `shipped_sample_count(...) = 1` (`renderer_wgpu/src/msaa.rs`, locked by
`everyone_ships_no_msaa_and_only_the_dev_rich_profile_keeps_the_request`). The garage goldens and
both garage probes render through `SceneRenderer::for_offscreen` at **4× MSAA**; the live garage
runs `WindowRenderer` at **1× with FXAA as its only anti-aliasing**.

The hall is made of thin bars: railing posts at 0.03 m half-width, crane rails 0.08, cable trays
0.02–0.03, skylight mullions 0.08 — 524 triangles under a 3 cm equivalent edge. The reviewed frame
smooths them with four samples; the player gets one and a post-encode filter. **The locked picture
is not the played picture**, which is the one thing the whole review harness exists to prevent.

### G10 — The garage floors were never ratcheted, and the register overstates the frame

```rust
const GARAGE_BRIGHT_FLOOR: f32       = 0.0025;   // measured today: 0.010  (4× of slack)
const GARAGE_DARK_CEILING_FLOOR: f32 = 0.905;    // measured today: 0.805  (10 pp of slack)
```
(`client/tests/look_goldens.rs:391,396`). The garage may get four times darker and the gate stays
green. The policy is explicit: *"A wave is finished when its FLOOR has been raised to meet its
TARGET."* Neither floor has been raised once.

`docs/art-direction-program.md` carries **two different wrong numbers for the same frame**:

| source | dark | bright | spread | p05 |
|---|---:|---:|---:|---:|
| D20 row, marked CLOSED | 68.7% | 8.4% | 0.599 | 0.035 |
| the baseline table | 90.0% | 0.3% | 0.275 | 0.0036 |
| **measured 2026-08-09** | **80.5%** | **1.0%** | **0.567** | **0.019** |

The history explains it: `d8cfaaf` ("readable light") deleted the glowing frosted panes and cut
`bloom_weight` 0.07 → 0.04, re-recorded the goldens, and left the register alone. The art decision
was deliberate and argued in the commit body; **the bookkeeping did not follow it**, so D20 stands
marked closed at half its own 2% target.

### G11 — The studio has no subject measurement and no performance measurement

- `SUBJECT_BOUNDS` (`look_goldens.rs:643`) holds two entries, both Prokhorovka. **The garage —
  whose entire job is to sell the vehicle — has no subject crop.** The instrument already exists
  (`frame_stats_of`), the battlefield uses it, the studio does not.
- `perf_capture.rs` (713 lines) never mentions the garage or the hangar. **The garage's frame time
  has never been measured.** The one-look policy (MX330 @ 60 FPS) has no coverage here at all.
- The byte-exact half of the harness asserts per view inside one loop and renders the garage LAST.
  It fails on `prokhorovka_clear_afternoon` and **never reaches the garage views**. The garage's
  byte lock stands behind twenty assertions belonging to other maps.

### G12 — The light rig's slots are full

`local_pools` iterates six slots (`lighting_common.wgsl:28`); `garage_hero` uses five plus one
`OFF`. Under the readable-light doctrine — every light has a visible source — there is no room for
another lamp without resizing the uniform array.

## 4. What is good, and is not to be touched

- **Honest sources.** The skylights are real openings cut through the roof slab, the key genuinely
  reaches the turntable through one, and that is locked by a raycast against the LIVE lighting
  profile (`the_workshop_sun_reaches_the_turntable_through_a_real_opening`). No faked shadow disc.
  Every lamp pool has a housing hanging at it. That is rare discipline and it is the reason the
  room's light reads as light at all.
- **The bake is a pure function** — fixed spiral, no clock, no RNG, bit-identical across builds and
  test-locked as such. That is what licenses the golden harness to lock frames rendered from it.
- **One display transform.** The garage and the battlefield form their picture in the same
  `post.wgsl`; no pass tone-maps itself. Rule 7 holds.
- **The framing is single-sourced.** `hero_orbit_eye()`, `HERO_FOV_DEGREES` and `HERO_PARK_YAW` are
  read by the game, the goldens and both probes — D23 is closed structurally, not by convention.
- **The `bounce` lane was appended correctly**: zeros everywhere else, zero pixels of difference
  outside the hall.
- **`vehicle.wgsl:386`** reconciling four estimates of one occlusion by `min` instead of
  multiplying them is measured, argued work and reads correctly on the hero.

## 5. The order

1. **G1** — prebake the hall off the render thread; the mechanism is already in the same function.
   Biggest felt improvement, no risk to the picture.
2. **G2 + G3** — surface roles and a finish for the hall. This is the plastic. Cheapest large move
   on the picture in the whole garage.
3. **G8** — swap the cascades and make the test read the shipped resolution. ~4× on the contact
   shadow, free.
4. **G4** — point the garage profiles' sky colours at what the roof openings actually show.
5. **G10 + G11** — raise the floors to what the frame measures, add the hero crop, correct D20, put
   the garage in `perf_capture`. Without this every later change to the garage picture is
   unverifiable.
6. **G7** — the bevel law and the roundness law reach `scene_build`. Both laws exist; the hall does
   not import either.
7. **G5** — a bounce (or contact) lane on the vehicle. The largest piece of work, and the
   2026-08-08 audit already argued it would land.
8. **G6, G9, G12** — the structural debt: bake resolution, the MSAA divergence, the light-slot
   ceiling.
