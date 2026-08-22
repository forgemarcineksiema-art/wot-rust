# Art Direction 3.0 — Pulling The Picture Up To The Policy

[art-direction-policy.md](art-direction-policy.md) states the target look and carries the locks.
**This document states why the shipped picture does not obey it, and in what order that gets
fixed.** The policy is the bible; this is the campaign. When the register below is empty and every
`FLOOR` has met its `TARGET`, this document becomes history and the policy stands alone.

> Sibling program: [world-scale-program.md](world-scale-program.md) (2026-08-03) — the measured
> register of the WORLD being 25–75% too small and too uniform around a correctly-scaled tank
> (trees, landmarks, horizon, instance variance, camera FOV). This program fixes the light;
> that one fixes the metres.

## Why this program exists

The engine has the whole apparatus: cascaded sun shadows, SSAO, HDR + ACES-lite, height/distance
fog with aerial perspective, a four-layer terrain splat with a field quilt, domain-warped FBM
clouds, an imported CC0 flora pipeline, PBR vehicles with baked cavity AO. It also has a written
target. **The picture that reaches the screen is two classes below both.**

The proof is in the repository, not in an opinion. The committed look goldens
(`crates/apps/client/tests/goldens/look/`, recorded 2026-07-22) are the frames the engine holds up
as correct, and they break the policy they exist to lock:

- `prokhorovka_golden_evening.png` is not golden — pale lavender sky over a yellow-green field,
  no dark mass anywhere, no warmth surviving to the pixels.
- `prokhorovka_grass_midfield.png` has near-white ground, canopies reading as black lumps on
  orange sticks, and canopies visibly detached from their trunks.
- `prokhorovka_overcast.png` is the same milk a player sees in a live battle.

### Root cause: the loop between the policy and the pixels was never closed

Three failures compound, and none of them is a tuning mistake.

1. **The locks measure the profile, not the photograph.**
   `crates/render/renderer_api/tests/look_locks.rs` computes shade/field/sky luma **analytically
   from `SceneLighting` numbers**. A profile can sit perfectly inside policy while the rendered
   frame is a flat wash, and the gate stays green — because the gate never looks at the frame.
2. **The one pixel-side lock is a rubber stamp.** `crates/apps/client/tests/look_goldens.rs:218`
   floors the dark plane at `0.001` — one pixel in a thousand — for every view not named
   "evening", with a comment conceding the floor is "symbolic for now".
3. **The review set measures the wrong frames.** `crates/world/scene_build/src/review_views.rs`
   holds five views: all on Prokhorovka, **none containing a vehicle**, and three shot from an eye
   **14 m** above the ground. The game has four maps, its subject is a tank, and the player's
   camera sits at hull height. Nothing about the shipped experience is under lock.

A fourth failure was the direct consequence of (3) and is worth stating on its own: because the
review example and the golden harness each hand-rolled ~50 lines of identical scene setup, they
drifted, and **both forgot to bind the foliage atlas**. The locked reference frames rendered
imported flora as untextured white (D13). The document's own promise — "the frame a human reviews
is exactly the frame the harness locks" — was a convention, and conventions rot.

W0 answered all four: the FLOOR/TARGET pairs (`look_goldens.rs:273-277`) replaced the symbolic
floor, `review_views_for` + `hangar_review_views` rebuilt the review set, and the shared
`look_harness` killed the setup duplication — the register below records each closure.

## The decisions this program is built on

Taken 2026-07-26. Each is a deliberate commitment, not a default.

| Decision | Choice |
|---|---|
| **Course** | **Pull everything up to the painterly target.** No tier of content is exempt: flora is procedural-only (Świat 2.0), rocks get a generator, `TreeLine` / `RailCover` get real geometry (`Wreck` stays a box — out of Świat 2.0), the sky's visible band is rebuilt, vehicles get full surface narrative. |
| **First program** | **Calibrate before content.** Build the instrument, then move the numbers. Tuning four maps and twelve looks against a broken baseline is work thrown away. |
| **Weather variants** | **The roll stays; every look comes up.** `apps/server/src/match_info.rs::pick_weather` keeps choosing at random, so no variant may be a "worse day" — three looks across four maps each hold the bar on their own. This is the policy's "equally authored days" taken literally. |
| **Per-map identity** | **Four identities, not one look reused.** Each map earns its own ground palette, its own times of day, its own review views and its own goldens. |

## Defect register

Every entry is reproducible from the repository or from a probe render. The wave column says where
it closes.

| # | Defect | Evidence | Wave |
|---|---|---|---|
| ~~D1~~ | ~~Locks computed from the profile, not the picture; dark-plane floor is 0.1% of pixels~~ — **CLOSED (W0)**: the FLOOR/TARGET mechanism below replaced the symbolic floor — `look_goldens.rs:273-277` asserts `OUTDOOR_DARK_FLOOR = 0.008`, reports `OUTDOOR_DARK_TARGET = 0.08`, and holds `OUTDOOR_SPREAD_FLOOR = 0.34`, all measured on the golden pixels | `look_goldens.rs:273-277` | W0 |
| ~~D2~~ | ~~Review set covers 1 map of 4, contains no vehicle, shoots from 14 m, and has no garage entry~~ — **CLOSED (W0)**: `review_views_for` covers all four `REVIEWED_MAPS`, `hangar_review_views()` adds the garage, the vantage is locked by `no_review_camera_sits_above_the_players_own_eye` and a vehicle-in-frame range assert | `review_views.rs:371,399-403` | W0 |
| ~~D3~~ | ~~The milky sky is **structural**, not a tuning error… the visible band is the fog colour, which must be pale by construction~~ — **CLOSED (2026-08-21, three PRs)**. The cloud belt had already been reopened to `smoothstep(0.005, 0.16, dir.y)`; what remained was the fog colour and the sky pass, and both fell. (1) The fog colour stopped being pale-by-construction: the sun-haze blend is energy-conserving (`horizon*0.55 + key*0.45`, was `0.4/0.8` = 1.2× energy grading BRIGHTER than the sky), locked by `the_sun_haze_warms_the_air_but_never_outshines_it` via the new CPU mirror `fog_sun_haze_reference()`; the horizons darkened (Bystra `[0.78,0.72,0.62]→[0.64,0.60,0.51]`); the fog reaches the backdrop hills (falloff 0.02→0.007/0.008, locked by `aerial_perspective_reaches_the_hills`). (2) The sky pass stopped adding its own milk: the `*1.06` super-horizon lift is gone (the band is the fog colour, exactly), the disc multiplier fell 6.0→3.0 (the core still tops ACES but the flat-white plateau shrank and the skirt keeps the key's colour — the sun reads golden), and the pow-9 halo that washed ~26° of sky at a fifth of key strength tightened to pow-14 at half that. The pixel-side regression stop is the new `near_white` ceiling: ≤ 1.5% of any outdoor golden at ≥ 0.97 linear luma | `sky.wgsl`, `lighting_common.wgsl`, `look_goldens.rs` | W1 |
| D4 | No dark mass: the field is uniformly sunlit, and cloud shadows run at 0.25–0.3 strength over a very large scale | `lighting.rs` profiles | W1 |
| ~~D5~~ | ~~**Every** battle tree is pinned to `TreeLod::Mid`~~ — **CLOSED (Świat 2.0 F0)**: the battlefield oak draws through the instanced LOD ladder (`scene_build::tree_lod`) with Near/Mid/Impostor rungs from `world_forge::tree`; height locks stay in `tree.rs`. Other species still bake Mid into statics — their counts are small enough not to need the ladder | `tree_lod.rs`, `tree.rs` | W2 |
| ~~D6~~ | ~~Three content kinds still render as a bare cuboid in Cover 2.0 scope: `TreeLine`, `RailCover`, and `SceneryKind::Rock`~~ — **CLOSED (Świat 2.0, 2026-08-07)**: `TreeLine` wears a szpaler and `RailCover` a revetment (PR 5 + 7, `b064287`); `SceneryKind::Rock` is now baked by `world_forge::rock` (PR 8a) — a displaced, cut-and-sunk erratic with its frost spall, wearing the new `surface_role::ROCK_FACE` and **the stone of the map it stands on** instead of one hardcoded grey on all four. `Wreck` stays a footprint box by decision (out of Świat 2.0, 2026-08-07), which is what this row's "three kinds" already excluded | `battlefield.rs`, `rock.rs`, `clutter.rs` | W2 |
| ~~D7~~ | ~~Grass has no clumping term: 28 candidates per 8 m cell at uniformly random positions~~ — **CLOSED (teren A4)**: both grass systems share one clump rule in `crate::grass` — 2–3 hash centres per 8 m cell pull every candidate by a convex 0.55 lerp (in-cell by construction, so the per-cell determinism/phase contracts hold untouched), and a low-frequency baldness field (sampled at the FOLDED z, so the mirrored card meadow and the true-position near ring agree across the axis) refuses ~10 % of ground outright. Redistribution, not addition: 28 candidates, 4.5 cards and every budget unchanged. Locked in the Clark–Evans direction (mean NN < 0.85 × uniform) in BOTH systems + the self-locating bald-cell lock; the cache-sweep floor renegotiated 3 800 → 3 300 in-diff (visible bald patches are the feature) | `grass.rs::clump_centres`, `grass_cards.rs` | W2 |
| D8 | Baked contact AO exists for the **T-54 only**; there is **no curvature/edge-wear term anywhere in the repository**; dust is confined to the running gear | `surface_bake.rs`, `vehicle.wgsl:310` | W3 |
| D9 | `VehicleVariation` carries `dirt` / `snow` / `camo` lanes that are **never populated in battle** | `variation.rs:105-116` | W3 |
| ~~D10~~ | ~~Team colour keys on `tank.id == player_tank` instead of `TeamId`~~ — **CLOSED (PR #317)**: both paths now read `PresentationTank::team` through one shared rule; identity still decides the gun. In a 7v7 this had six of the thirteen other tanks wearing enemy paint | `render_frame.rs` | W3 |
| ~~D11~~ | ~~The garage has **no golden and no review view**~~ — **CLOSED (W0/W4)**: `goldens/look/garage_hero.png` and `garage_screen.png` are both locked (D24 records the screen golden landing) and `hangar_review_views()` puts the hangar in the review set. `garage_workshop` itself stays a test-only profile (`lighting.rs:517`) — the locked frames light through the hero preset | `review_views.rs:95`, `goldens/look/` | W0/W4 |
| ~~D12~~ | ~~Two vegetation languages share a frame~~ — **CLOSED (Świat 2.0 F0, 2026-08-06)**: flora is procedural-only; imported CC0 meshes/`assets/flora`/`import-flora` are gone; maps author procedural `Oak` (instanced LOD + trunk cover); `FloraTree`/`FloraPine`/`FloraBush` are retired wire-only variants | `docs/map-forge-policy.md` #10 | W2 |
| ~~D13~~ | ~~**The locked goldens render imported flora as WHITE.**~~ — **CLOSED (W0)**; then **OBSOLETE (Świat 2.0 F0)**: there is no foliage atlas to bind — trees are vertex-coloured procedural meshes | — | W0 |
| ~~D14~~ | ~~The imported `stylized-tree` orange-red trunk~~ — **CLOSED (Świat 2.0 F0)**: the asset is gone with the import pipeline | — | W2 |
| D15 | Outside the T-54 the fleet offers nothing to look at up close: unbroken plates, no weld seams, grab handles, tow cable, spare track or vision blocks; hull and turret read as two different paints (cast vs rolled split too far); running gear is a black void with no contact | `target/closeup_probe/centurion_flank.png` | W3 |
| ~~D16~~ | ~~The garage room's content sits entirely outside the hero framing; the hero does not separate in value from its background~~ — **CLOSED (Hala 3.0)**. First half by the reframing: `HERO_ORBIT_PITCH` 0.28 → 0.13 brought the gallery band, the bay gate and the frosted panes into frame, and A3 (#538) gave every slot frame a composed background. Second half by the relight chain (#539 → #547): the hero-over-room separation is now a LOCKED ratio — `HERO_OVER_ROOM` re-derived explicitly 2.0 → 1.7 when the shafts landed (the derivation history lives in the lock's comment), measured 1.83x at E3 — so the hero separates from its background by contract, not by luck | `garage_render.rs:142`, `hangar_gallery.rs`, `hangar_props.rs` | W4 |
| D22 | **The hero was parked at the camera's own bearing.** `HERO_ORBIT_YAW` and the parked `yaw_rad` were both 0.6, so the "three-quarter" the comments claimed was a head-on elevation with the barrel bisecting the hull — the one angle at which the whole fleet looks alike. **CLOSED**: `hangar::HERO_PARK_YAW = HERO_ORBIT_YAW + 0.65`, read by the live garage, the golden and the example | `hangar.rs`, `garage_render.rs`, `review_views.rs` | W4 |
| D23 | **The human-review example hard-coded the framing** — `(0.60, 0.28, 14.0)` and a 32° lens, copied beside the constants `ecc0777` had just centralised so that "a reframing moves the played picture and the locked picture together". A reframing would have moved the golden and left the reviewed frame behind, which is D13's disease exactly. **CLOSED**: the example reads `hero_orbit_eye()` / `HERO_FOV_DEGREES` | `examples/garage_hangar_review.rs:70` | W4 |
| D24 | **The garage UI had no picture lock of any kind.** `garage_hero` is the room only; the overlay was covered solely by unit tests over rect arithmetic, which cannot see a control drawn off its plate. It had shipped for months with the top-bar plate ending at y=0.86 while both screen tabs hit-tested 0.785–0.845 — GARAGE and TECH TREE rendered as dim text on the hangar wall, and GARAGE answered no click at all. **CLOSED**: `garage_screen` golden + the plate reaches its own tab row | `review_views.rs`, `panels/topbar.rs` | W4 |
| D25 | **The VEHICLE column did not carry the game's own promise.** Six rows — HP, kW, km/h, °/s, penetration, reload — with no dispersion, no aim time and no armour: the "no ±25% roll, the gun groups where it is pointed" pitch was absent from the screen where the player picks a gun and presses Battle. **CLOSED**: nine rows, plate derived from `STAT_ROWS` | `panels/stats.rs` | W4 |
| D17 | The fleet showcase renders vehicles in pastels (powder blue, lavender, pink, cream) — the canonical "no clones" render does not show paint | `target/vehicle_lineup.png` | W3 |
| D18 | **Orliny Pereval has no light of its own.** Its blueprint's `ClearAfternoon` preset resolves to `bystra_clear_afternoon` — the mountain pass wears the river valley's afternoon. The borrowed look is now locked, so the day it gets its own is visible in the diff | `blueprints/orliny-pereval.map.ron:114-119`, `weather.rs::preset_lighting` | W5 |
| ~~D20~~ | ~~The garage has almost no bright plane~~ — **CLOSED (W4/Hala 2.0 + the garage pipeline audit)**, and the closing numbers below are the third set this row has carried, which is the row's own lesson. The hall-light PRs (#450–#461) took the hero frame from 0.3% to 6.4% bright; Hala 2.0 T1a took it to 8.4%. Then `d8cfaaf` ("readable light") deleted the frosted panes and cut `bloom_weight` 0.07 → 0.04 for a doctrine this register agrees with, re-recorded the goldens, and **left this row claiming 8.4% / 68.7% while the frame it pointed at measured 1.0% / 80.5%**. The decision was right and the bookkeeping did not follow it — recorded here because that is the failure, not the tuning. Closed for real at **2.3% bright against the 2% target, spread 0.572, p05 0.021**, carried there by the room's own reflection (`sky_zenith_rgb` derived from the skylights' area share of the daylight behind them) and by the hall finally being made of concrete and painted steel instead of one untreated fill. `GARAGE_BRIGHT_FLOOR` is raised to the target, so it can no longer drift back | `hangar.rs`, `lighting.rs`, `scene.wgsl`, `look_goldens.rs` | W4 |
| D20a | **The garage has almost no bright plane** (original record, kept for the numbers) — 0.3% of the hero frame sits above the bright threshold, against a 2% target. Was 0.00%; the reframing (D16) brought the frosted panes into shot and they are the entire gain, being the only emissive surface the hero lens contains. **The reframing did not close this, and the percentiles say why**: p50 0.119, p95 0.276 — the whole picture is a narrow band pressed against the 0.25 dark/mid boundary, and the floor a player reads as light grey measures 0.238, a hair on the dark side. Where the lens points decides what is IN the picture; the light rig and the grade decide how far apart its values are. **This closes with light in the room, not with a camera** | `goldens/look/garage_hero.png`, `look_goldens.rs` `GARAGE_BRIGHT_TARGET` | W4 |
| D21 | **Cloud shadows never run in the shipped game.** `LightingQuality::canonical()` set `cloud_shadows: false`; only the dev-only `rich()` enabled them. The procedural evaluation was measured and refused at +4.8 ms (see below) — then made cheap instead of argued: the coverage field is now baked once into a seamlessly tiling R8 texture at renderer init (`cloud_map.rs`, bindings 5–6 of the environment group), so the per-fragment cost fell from ~6 lattice evaluations to one repeat-sampled tap. **CLOSED**: `canonical()` ships `cloud_shadows: true`; the `WOT_CLOUD_SHADOWS` knob stays so the cost keeps being measurable as one variable. Locked by the one-look profile test and the tile's seam/determinism/span tests | `lighting_quality.rs`, `cloud_map.rs`, `shadow_common.wgsl::cloud_shadow` | W1 |
| D19 | **Two thirds CLOSED (teren A2)**, facades stay open. ~~Grass scatters onto the city street~~ — the card meadow now re-samples vegetation per CARD (the 8 m cell-centre gate let cards straddle streets its centre missed) and BOTH grass systems exclude authored cover footprints, locked by `the_city_grows_no_cards_on_streets_or_through_floors` + `a_cover_footprint_grows_no_grass`. ~~`RoadSurface::Cobble` reads as a dirt path~~ — `weights_from` routes Ballast/Cobble to the ROCK lane (in the splat and under the tracks: a paved road stops being the slowest ground), Ostrogorsk's stone layer repainted granite `(0.36, 0.37, 0.40)`; locked by `a_paved_road_is_never_the_slowest_ground` + the splat goldens. ~~Tenement facades are flat boxes with painted window rectangles over a hard black plinth~~ — **CLOSED (Świat 2.0 PR 3)**: true pierced openings with recessed glass, stone frames/sills/lintel bands, lesenes and cornice; plinth takes a palette stone (`stone_palette`) with the new `DRESSED_STONE` role and its own ashlar shader pole; FactoryHall got pilaster bays, the wagon portal and the clerestory. ~~Rural styles keep painted windows~~ — **CLOSED (Świat 2.0 PR 4)**: cottage/townhouse/church get pierced leaves with recessed panes and doorways cut through the leaf; the barn takes shuttered slits and true wagon portals in both gables; the church bell stage is four corner piers with real openings. **D19 CLOSED in full** | `goldens/look/ostrogorsk_canyon.png`, `building.rs::bake_tenement` | W2 |
| D26 | **The ground grain rendered as hard ~0.3–0.6 m square plates** (most visible under grazing raking light and near the eye) — the ground twin of the square-sky artefact, with the same three roots: the `fract(sin(dot))` lattice hash collapsed to correlated corners once world-metre coordinates left the GPU sin's accurate range; every detail octave sampled one axis-aligned square lattice; and the light-catching normal "gradient" was a finite difference stepped at over half a lattice cell, faceting the field into tiles. **CLOSED**: `noise_common.wgsl` — an integer-domain hash, rotated octave frames, and the analytic gradient (`value_noise_grad`, also ~4 fewer noise evaluations per terrain fragment); one implementation shared by terrain and statics, de-squaring locked by `ground_grain_is_lattice_decorrelated` | `noise_common.wgsl`, `terrain.wgsl`, `scene.wgsl` | W1 |
| D27 | **The display grade carried a second, undeclared crush.** The contrast step was `(x - 0.5) * contrast + 0.5` — a straight line of slope `contrast` — so everything below `0.5 - 0.5/contrast` went negative and clamped to pure black: a dead band reaching 0.054 at the shipped 1.12. A hull's shaded flank lives inside it, and the backlit review frame's median vehicle pixel entered that step at 0.068 and left at 0.016, with its darkest twentieth at exactly 0.000. This is separate from the black point, which is *supposed* to make deep shade black and is untouched. **CLOSED**: the step is a `smoothstep` blend with a real toe, keeping `contrast`'s meaning (the slope at mid grey, `1 + k/2`) while compressing the darks instead of deleting them. Subject median 0.016 → 0.056 from this change alone; the lit end does not move. Locked by `only_the_black_point_may_produce_pure_black` | `lighting_common.wgsl::display_grade`, `lighting.rs::grade_reference` | W1 |
| D28 | **Screen AO was multiplied into the three bakes it was supposed to join.** `vehicle.wgsl` reconciles its material AO, cavity and contact bakes with `min` — for the stated reason that they are three estimates of one quantity and multiplying them compounds it — then took the product of that result and SSAO anyway. On a shaded flank both sit near 0.7, so the indirect light the dark side lives on was halved rather than dimmed once. The sky reflection took the same pair twice over. **CLOSED**: one `indirect_occlusion = min(surface_occlusion, screen)` for the indirect terms; the direct sun keeps the baked occlusion, and the rim stays out of both as `lighting_common` already promised. Subject median 0.056 → 0.070, p05 off the floor at last | `vehicle.wgsl` | W1 |
| D29 | **Matte service paint reflected like semi-gloss.** Specular amplitude fell LINEARLY with smoothness, `(1 - roughness) * 0.4`, leaving rolled armour at roughness 0.58 with 0.168 — about four times a dielectric's ~0.04 — so big smooth cast shapes carried broad hot sheets of sky. A large part of what "the light is too strong" actually was. **CLOSED**: the amplitude falls with the cube, `pow(1 - roughness, 3) * 0.6` — paint drops to ~0.044 while headlight glass RISES from 0.36 to 0.44, so the one role meant to catch the sun still does | `vehicle.wgsl` | W1 |
| D30 | **Cloud shade fell on the world but not on the vehicles in it.** `terrain.wgsl` and `scene.wgsl` multiplied the sun by `cloud_shadow`; `vehicle.wgsl` never did — the word "cloud" did not appear in the file. A bank of shade swept the field and every tank standing in it stayed at full key: a bright cut-out pasted on darkened ground. Nothing in this policy asked for that; it was simply missed, which is what a per-pass list catches and a screenshot does not. **CLOSED**: locked by `every_sunlit_pass_takes_the_cloud_shade` | `vehicle.wgsl`, `cloud_map.rs` | W1 |
| D32 | **Every penumbra wore the same woven-cloth print.** The widened reduced PCF kernel (D-adjacent to the softening work) placed its four taps on a FIXED ±1-texel diagonal lattice — sparse, with holes — and the lattice printed into every soft edge as diagonal weave and flat plateaus, identical everywhere, "a bit like fabric" (user screenshots, 2026-08-04: stowage-box penumbrae, the turntable's floor shadow, the turret's cast edge). **CLOSED**: the four taps sit on a cross of radius 1 texel rotated per pixel by interleaved gradient noise — same tap count, structure decorrelates into grain that FXAA and the post dither integrate away, contact roots kept (bilinear support still touches the fragment's texel at every angle). Locked by the flat-plateau share of the penumbra: 42.0% woven vs 8.4% rotated, bound 20% (`shadow_render_frame.rs`) | `shadow_common.wgsl::pcf_rotation`, screenshots | W1 |
| D31 | **The vehicle-readability bound was measuring the paint, not the readability.** `subject void <= 0.45` counted pixels under 0.25 linear luma — a threshold a dark-green hull sits below almost everywhere it is not in direct sun. Proved by giving the golden frame (`prokhorovka_evening_contact`, the one this program calls the first frame that looks like the policy) a subject box of its own: it scores **89.4% void against the broken frame's 72.1%**. The metric ranked the good frame worse than the bad one. **CLOSED**: dark share demotes to a per-view regression ceiling, and the readability TARGET moves to local contrast — which ranks them the way an eye does, 0.0145 golden against 0.0061 broken — set at two thirds of the reference frame's, derived rather than invented | `look_goldens.rs::SUBJECT_BOUNDS`, `review_views.rs` | W1 |

## What the instrument found first

Recorded here because it changes how W1 should be read.

**The reference look was not as broken as the reference frames said — it was being judged from
a vantage that destroyed it.** Dropping the panoramas from 14 m to the player's own 4.9 m and
putting a T-54 in frame produced `prokhorovka_evening_contact`, and that frame *is* golden: warm
raking light, a hull that grounds on a real cast shadow and separates from the field, a ridge
raking into shade. It is the first frame in this program that looks like the policy.

Two consequences:

1. **W1's job is smaller than the goldens implied, and differently shaped.** The evening profile
   largely works at hull height with a subject in it. What fails is the empty long-range
   panorama, where the visible sky band is fog-coloured milk (D3) and nothing casts (D4). Tune
   for the frame the player occupies, not for the vantage that flattered nothing.
2. **A review vantage is an art-direction decision, not a convenience.** The panoramas had to
   move sideways as well as down: at 4.9 m on the map's axis the camera sits on the road crown
   and a third of the frame becomes embankment. Height alone was not the fix.

## The FLOOR / TARGET mechanism

The reason the dark-plane floor sat at `0.001` behind an apologetic comment is that there was
nowhere to *record* the distance between what the picture is and what it must become. So the gap
hid in prose, and prose does not fail a build.

From this program on, every value-structure bound is a pair:

- **`FLOOR`** — what today's picture actually achieves. Asserted, so it can never regress.
- **`TARGET`** — what the policy demands. Not asserted yet; emitted as a `LOOK DEBT` line with
  the remaining distance.

Both live in the test as named constants, so the gap is a value in code rather than a sentence in
a comment. The debt lines surface under `cargo test -- --nocapture` (cargo swallows a passing
test's stdout, so they are not on every `verify` run); the standing record is the debt table in
this document, refreshed whenever a wave moves a bound. A wave is done when its `FLOOR` has been
raised to meet its `TARGET`. A PR that moves a bound re-blesses it in the same diff and says, in
its description, **what changed about the PICTURE** — not only what changed about the code.

## The baseline

Every recorded frame, measured. Produced by
`cargo test -p client --test look_goldens -- --nocapture measured_baseline`; refresh it whenever
a wave moves a number. Luminance is display-linear, so `dark < 0.25` is roughly "below mid-grey
on screen". `band` is the top 15% of rows minus the bottom 40% — sky-band minus near-field on an
outdoor frame, and meaningless indoors.

Re-measured in full 2026-08-21, three times in one day: the standalone re-bless after 28
frames of content drift (#610 — the opt-in GPU gate had been red on master since the
T-54/tension/ammo waves), then the W1 air pass (energy-conserving sun-haze, darker horizon,
fog that reaches the hills), then the W1 light pass (warm keys R/B ≥ 1.5, trimmed flat-light
stack, exposure 1.1 → 1.05/1.0). Together they moved the clear days from milk toward the
policy: `bystra_clear_afternoon` bright 44.7% → 33.3% and **dark 3.6% → 17.0%** (first
outdoor clear-day frame AT the 0.08 dark target), `orliny_clear_afternoon` dark 5.2% → 13.0%,
`orliny_pine_belt` bright 46.2% → 27.1%, `bystra_town_lane` bright 40.9% → 26.8%. The light
pass was tuned against the backlit-subject locks: a harder fill/rim cut and exposure 1.0
were each measured and REFUSED (subject median 0.049 / form 0.0069 vs floors 0.060 / 0.0070).

Re-blessed 2026-08-22 (17 frames) after two user-merged look changes landed without their
PNGs: the SSAO diet #578 (all 8 garage frames, 52-73% of pixels at max delta 34-35/255 — the
room's corners lighten a touch) and the landmark rise #596 (Bystra x4 / Ostrogorsk x5 skyline
silhouettes at 0.2-0.5%, plus `bystra_town_lane` at 14.1% — the 27 m church now anchors the
lane and adds real shade mass: dark 20.4% -> 25.3%, toward the W1 target, not away). All value
locks held without relaxation; Prokhorovka and Orliny frames byte-identical.

| frame | dark | mid | bright | p05 | p50 | p95 | spread | sat | local | band |
|---|---|---|---|---|---|---|---|---|---|---|
| `prokhorovka_clear_afternoon` | 1.0% | 62.4% | 36.7% | 0.345 | 0.516 | 0.849 | 0.504 | 0.355 | 0.0083 | +0.255 |
| `prokhorovka_golden_evening` | 40.9% | 25.1% | 34.0% | 0.121 | 0.415 | 0.701 | 0.580 | 0.364 | 0.0087 | +0.393 |
| `prokhorovka_overcast` | 0.7% | 50.3% | 49.0% | 0.353 | 0.510 | 0.720 | 0.367 | 0.183 | 0.0063 | +0.283 |
| `prokhorovka_evening_midfield` | 40.1% | 25.0% | 34.9% | 0.093 | 0.313 | 0.715 | 0.621 | 0.387 | 0.0089 | +0.410 |
| `prokhorovka_grass_midfield` | 2.1% | 56.7% | 41.1% | 0.355 | 0.530 | 0.841 | 0.487 | 0.341 | 0.0077 | +0.231 |
| `prokhorovka_contact_backlit` | 21.8% | 57.7% | 20.5% | 0.040 | 0.440 | 0.810 | 0.770 | 0.424 | 0.0074 | +0.351 |
| `prokhorovka_evening_contact` | 48.7% | 26.4% | 24.8% | 0.073 | 0.255 | 0.700 | 0.628 | 0.448 | 0.0090 | +0.393 |
| `bystra_clear_afternoon` | 17.1% | 49.7% | 33.2% | 0.200 | 0.381 | 0.726 | 0.526 | 0.344 | 0.0102 | +0.386 |
| `bystra_rain` | 1.4% | 57.2% | 41.3% | 0.292 | 0.420 | 0.689 | 0.397 | 0.198 | 0.0064 | +0.329 |
| `bystra_dawn_fog` | 27.8% | 29.8% | 42.4% | 0.177 | 0.334 | 0.763 | 0.586 | 0.217 | 0.0093 | +0.451 |
| `bystra_town_lane` | 25.3% | 49.9% | 24.8% | 0.067 | 0.330 | 0.710 | 0.643 | 0.366 | 0.0084 | +0.326 |
| `orliny_clear_afternoon` | 13.0% | 54.3% | 32.7% | 0.206 | 0.342 | 0.781 | 0.575 | 0.344 | 0.0104 | +0.453 |
| `orliny_golden_evening` | 47.8% | 17.2% | 35.0% | 0.123 | 0.259 | 0.806 | 0.683 | 0.329 | 0.0112 | +0.562 |
| `orliny_overcast` | 1.0% | 55.7% | 43.3% | 0.331 | 0.417 | 0.721 | 0.389 | 0.189 | 0.0063 | +0.314 |
| `orliny_pine_belt` | 14.1% | 58.8% | 27.1% | 0.205 | 0.384 | 0.718 | 0.513 | 0.342 | 0.0094 | +0.324 |
| `ostrogorsk_clear_afternoon` | 2.4% | 62.4% | 35.2% | 0.316 | 0.505 | 0.833 | 0.517 | 0.281 | 0.0103 | +0.298 |
| `ostrogorsk_golden_evening` | 38.8% | 24.7% | 36.5% | 0.113 | 0.297 | 0.799 | 0.686 | 0.325 | 0.0102 | +0.530 |
| `ostrogorsk_overcast` | 4.7% | 51.9% | 43.4% | 0.271 | 0.444 | 0.721 | 0.450 | 0.129 | 0.0073 | +0.288 |
| `ostrogorsk_rain` | 4.3% | 52.8% | 43.0% | 0.275 | 0.411 | 0.683 | 0.408 | 0.134 | 0.0065 | +0.294 |
| `ostrogorsk_canyon` | 7.4% | 69.3% | 23.3% | 0.210 | 0.435 | 0.842 | 0.632 | 0.242 | 0.0081 | +0.255 |
| `garage_hero` | 72.8% | 23.1% | 4.1% | 0.030 | 0.162 | 0.573 | 0.543 | 0.229 | 0.0076 | −0.134 |
| `garage_screen` | 75.7% | 20.4% | 3.9% | 0.063 | 0.140 | 0.565 | 0.502 | 0.190 | 0.0105 | −0.089 |
| `garage_tech_tree` | 78.9% | 19.3% | 1.8% | 0.063 | 0.127 | 0.494 | 0.431 | 0.191 | 0.0064 | −0.101 |
| `garage_option_list` | 77.0% | 19.1% | 3.9% | 0.063 | 0.138 | 0.566 | 0.503 | 0.191 | 0.0107 | −0.075 |
| `garage_susp_close` | 86.0% | 14.0% | 0.0% | 0.050 | 0.123 | 0.317 | 0.267 | 0.405 | 0.0059 | +0.069 |
| `garage_hero_tiger2` | 67.6% | 29.8% | 2.6% | 0.028 | 0.175 | 0.522 | 0.494 | 0.239 | 0.0070 | −0.182 |
| `garage_hero_jagdtiger` | 64.3% | 33.2% | 2.4% | 0.027 | 0.184 | 0.518 | 0.491 | 0.238 | 0.0073 | −0.239 |
| `garage_inspector` | 68.4% | 26.0% | 5.6% | 0.030 | 0.171 | 0.607 | 0.577 | 0.256 | 0.0073 | −0.160 |

### What the baseline says

*(Reading of 2026-08-09. The specific figures cited below predate the 2026-08-14 re-measure
above — the diagnosis and its direction are unchanged, so the reading stands as the record of
why W1 exists; re-derive it only when a wave actually moves the milk.)*

**The evening looks are not the problem.** `prokhorovka_evening_contact` (34.6% dark, spread
0.617), `orliny_golden_evening` (49.2%, 0.728) and `ostrogorsk_golden_evening` (43.2%, 0.741) all
carry a real dark mass and a wide range. The reference look works.

**The clear/overcast days are the problem, and the numbers name it exactly.**
`prokhorovka_clear_afternoon` holds **1.0% dark against 58.9% bright**, with p05 at **0.452** —
even the darkest twentieth of the frame is mid-grey. `prokhorovka_overcast` is the flattest
picture in the set at spread 0.348. That is the milk, quantified: not a colour problem, a
*missing shade mass* problem. It is W1's whole job (D3, D4).

**Two looks are thin in the middle.** `orliny_golden_evening` (15.2% mid) and
`ostrogorsk_golden_evening` (19.4%) read bimodal — lit or black, with little in between. Watch
this when W1 retunes exposure; deepening the shade further would make it worse.

**The garage's value debt is paid.** Rewritten 2026-08-09 against measured frames, because the
row above it had been wrong for months in two different directions at once (see D20); rewritten
again 2026-08-14, because the 2026-08-09 sentence ("80.4% dark against a 75% bound is what
remains") outlived its own fix by four days. Hala 3.0 E3 (#547, the gate curtain and the chain
hoist foreground) took the hero's dark plane under the bound for the first time, and the #554
relight held it there: the table above measures `garage_hero` at **72.3% dark against the 75%
bound**, 4.2% bright against the 2% target, spread 0.544. What the garage still owes is not a
value — it is the frame budget (~3 ms over on the MX330, which gates 4x MSAA and the deck
reflection), owned by `docs/hala-4-program/plan.md`.

Its lowest-local-contrast standing is WITHDRAWN as a finding, because local contrast cannot
measure what it was being read as measuring. It is the mean step between horizontally adjacent
pixels — edge density — and a room of large flat planes legitimately has fewer edges than a
hedgerow. Measured directly: giving the whole hall two-octave material treatments moved 81% of
the frame's pixels and moved local contrast from 0.0049 to 0.0050, and restricting the measure to
lit pixels only moved it 0.01133 to 0.01136. A smooth octave whose features are ten pixels wide
has a small per-pixel gradient however much tonal variation it carries. Judge material by
looking; judge edges by this number.

The reframing that closed D16 is the proof of that sentence rather than a counter-example to it.
It changed what the picture CONTAINS — a real three-quarter hero, the gallery band, the bay gate,
daylight over it — and moved bright from 0.000 to 0.003 and dark from 0.899 to 0.900. **The band
did not move, because the band is not a property of the framing.** The floor a player reads as
light grey measures 0.238 against a 0.25 threshold: this frame is not dark, it is *narrow*, and
the whole of it happens to sit on the dark side of one boundary. Nothing about where the camera
stands can widen it.

## The debt

Emitted by `cargo test -p client --test look_goldens -- --nocapture recorded_goldens`. Each line
is a frame that clears its FLOOR (so it cannot get worse) but has not reached its TARGET. **This
list is W1's and W4's work order.**

```
(refreshed 2026-08-21 after the W1 air + light passes)
prokhorovka_clear_afternoon: dark plane 0.010, target 0.080 (short by 0.070, W1)
prokhorovka_overcast:        dark plane 0.007, target 0.080 (short by 0.073, W1)
prokhorovka_overcast:        spread     0.368, target 0.450 (short by 0.082, W1)
prokhorovka_grass_midfield:  dark plane 0.021, target 0.080 (short by 0.059, W1)
bystra_rain:                 dark plane 0.014, target 0.080 (short by 0.066, W1)
bystra_rain:                 spread     0.397, target 0.450 (short by 0.053, W1)
orliny_overcast:             dark plane 0.010, target 0.080 (short by 0.070, W1)
orliny_overcast:             spread     0.389, target 0.450 (short by 0.061, W1)
ostrogorsk_clear_afternoon:  dark plane 0.024, target 0.080 (short by 0.056, W1)
ostrogorsk_overcast:         dark plane 0.047, target 0.080 (short by 0.033, W1)
ostrogorsk_overcast:         spread     0.449, target 0.450 (short by 0.001, W1)
ostrogorsk_rain:             dark plane 0.042, target 0.080 (short by 0.038, W1)
ostrogorsk_rain:             spread     0.408, target 0.450 (short by 0.042, W1)
ostrogorsk_canyon:           dark plane 0.073, target 0.080 (short by 0.007, W1)
```

The 2026-08-21 light pass retired four whole frames from this list — `bystra_clear_afternoon`
(dark 0.170 — the first clear day AT target), `orliny_clear_afternoon` (0.130),
`orliny_pine_belt` (0.141) and `bystra_town_lane` — and moved every remaining clear-day line
closer. What is left is concentrated where the CONTENT is flat: Prokhorovka's empty steppe
(no shade-casting mass in frame) and the overcast/rain lids, whose flat light is authored.
The steppe's missing dark mass is a content question (a cloud-shade pattern only reads where
something anchors it), not another exposure notch — a harder cut was measured and refused
against the backlit-subject readability locks.

**Which frames are absent is the point.** Every golden-evening frame, the tank-at-contact frame,
the grass band, the dawn fog and the town lane are already at target on every metric. The debt is
concentrated in the clear and overcast days — and it is a *shade* debt, not a colour one: eleven
of the sixteen lines are the dark plane.

The two worst offenders are the two flattest pictures in the set, and they are the same two the
baseline named: `prokhorovka_overcast` (short 0.071 dark and 0.102 spread) and
`prokhorovka_clear_afternoon` (short 0.070 dark). Fixing those two moves the whole programme.

## The cloud-shadow buy-back: measured, and REFUSED at this cost

D21 says cloud shadows never run in the shipped tier, and the arithmetic says D4 cannot close
without them. So the buy-back was measured rather than argued, one variable at a time via the new
`WOT_CLOUD_SHADOWS=on|off` knob, on the stated min spec (**NVIDIA GeForce MX330**, Vulkan):

| probe | shadows off | shadows on | delta |
|---|---|---|---|
| `detail_cost_probe`, empty midfield, 1080p | 7.785 ms | 8.660 ms | +0.875 ms |
| `flora_frame_probe`, Ostrogorsk full scatter, 1080p | **11.279 ms** | **16.116 ms** | **+4.837 ms** |

The release-gate probe is the one that decides, and it lands at **96.7% of the 16.667 ms frame**.
Its own gate prints PASS — and PASS is misleading here, because that probe draws **no vehicles, no
FX, no HUD**. Roughly 0.55 ms is left for everything a real battle adds on top.

> **2026-08-22 correction (Drzewa 3.0 PR1).** The table above was captured while the instrument
> still rendered at 4× MSAA (`render_sample_count.rs` documents the divergence); the probe now
> measures the shipped 1× picture, and the fiscal picture is different. Re-measured baseline on
> the same MX330, Vulkan, 1080p @ 1× — `flora_frame_probe` now runs TWO gated views, and each
> leaves `target/flora_frame_<view>.png` so the number stands behind a reviewable frame:
>
> | view | baseline (no oaks) | full flora | oak delta |
> |---|---|---|---|
> | lineup (scatter at range) | 9.050 ms | **10.036 ms** | +0.986 ms |
> | under-crown (Near canopy fills the frame) | 6.829 ms | **9.481 ms** | +2.652 ms |
>
> Worst view median **10.036 ms** — ~6.6 ms of real headroom under the 16.667 ms line, not
> 0.55 ms. This is THE baseline every Drzewa 3.0 PR quotes; the acceptance bar for the program
> is ≤ 16.0 ms in both views.

**Refused at this cost — and then bought back by making it cheap.** The knob and these numbers
shipped so the next person asking "why is the steppe flat?" would find the measurement instead
of repeating it; the next person did. The section's own opening was the answer:

The cost was concentrated where it would be: `cloud_shadow()` ran a domain warp plus five
`value_noise` evaluations **per terrain fragment**, which is why an open map with heavy scatter
paid 5 ms while an empty midfield paid 0.9. The same coverage is now baked once at renderer init
into a seamlessly tiling R8 texture (`cloud_map.rs` — the lattice wrapped modulo an integer
period so the tile is seamless by construction, not by blending) and sampled through one
repeat-addressed tap at group-2 bindings 5–6. The per-fragment ALU fell to the sample, the
offset arithmetic and the threshold, and `canonical()` ships the shade in the one look (D21,
closed). The min-spec `perf_capture` number for THIS build is taken by re-running the capture:
`cargo run -p client --release --example probe -- perf_capture`.

## Why the vehicle's shaded half is dark: four levers, measured, all insufficient

`prokhorovka_contact_backlit` locks the failure — subject median **0.002**, void **71.5%** — and
the obvious explanations were each tested in isolation rather than argued. Every row is one
variable changed against the shipped build, measured on the same frame:

| lever tried | subject median | verdict |
|---|---|---|
| shipped build | 0.002 | the failure |
| `ground_ambient_rgb` doubled | 0.010 | **rejected by `scene_lighting`'s hemispheric invariant** — "a grounded look needs the ground darker than the sky". The lock is right. |
| screen-space AO forced fully off | 0.019 | biggest single contributor, still less than half the 0.045 target |
| AO split so it occludes the sky half only, not the ground bounce | 0.002 | no measurable change: the split can only matter once the ground bounce is meaningful, and the invariant caps it |
| baked cavity `contact` forced to 1.0 | 0.005 | not the cause either |

**There is no single lighting lever that reaches this.** Even removing *all* screen-space
occlusion leaves the median at less than half of target. The base signal is simply small: on a
face the sun never touches the key contributes nothing by construction (and the shadow map
correctly reports the hull occluding its own flank, so a wrap term on the key cannot rescue it
either), leaving ambient and fill — small numbers — landing on running-gear materials that are
very dark to begin with.

That reframes the problem. It is not a world-lighting bug to be tuned in W1; it is that **dark
materials under ambient-only light have nothing to show**, which is a vehicle-surface question:

- **D9 is the strongest candidate.** `VehicleVariation`'s dirt lane exists and is never populated
  in battle. Dusty running gear is far lighter than clean track steel and would read.
- Edge/rim treatment describing the silhouette, and the curvature term D15 wants, are the others.

So the readability debt moves to **W3**, and W1 keeps the lock that measures it. The lock stays
where it is precisely so a W3 change has to answer a number.

## The cast shadow is not soft because of the shadow map

`examples/shadow_probe` measures the thing directly: it collapses an authored ground box to a
column profile (grass cards are alpha-cutout, so their edges are the hardest thing in any outdoor
frame and a raw max-gradient just reports a blade — measured, and it did) and reports the 10-90%
transition width of the lit-to-shaded crossing. `WOT_SHADOW_FOCUS` was added beside the existing
`WOT_SHADOW_RES` so the two knobs that both change `texel_world_size = 2 * focus / resolution`
can be moved one at a time.

Swept on `prokhorovka_contact_backlit`:

| near-cascade resolution | focus half-box | texel | edge width |
|---|---|---|---|
| 1024 | 64 m | 12.5 cm | 168 px |
| 2048 | 64 m | 6.25 cm | 164 px |
| 8192 | 64 m | 1.56 cm | 155 px |
| 2048 | 16 m | 1.56 cm | **155 px** |

Two things fall out.

**The model is confirmed and my earlier reading of it was wrong.** Settings with equal texel size
give identical numbers — 2048/16 m matches 8192/64 m exactly. Resolution and box size are fully
interchangeable, exactly as `texel_world_size` says. An earlier attempt concluded "box size does
not matter" by comparing two renders *by eye*; that conclusion was an artefact of looking, not a
property of the renderer.

**And the shadow map is not what makes the edge soft.** An **eightfold** finer texel narrows the
transition by **8%**. Raising shadow resolution — the expensive option — buys almost nothing here.
The ~155 px transition is dominated by the grazing camera angle: a shadow boundary on the ground
seen nearly edge-on is stretched across many pixels by perspective, which is geometry, not blur.

So "the shadow cast on the terrain is poor quality" is not a resolution problem and must not be
answered with one. What is left to investigate is the shadow's SHAPE fidelity and its contrast
against the lit ground, not its sharpness — and any future attempt now has an instrument and a
baseline to argue against instead of an impression.

## Wave plan

**W0 — Instrument** (the approved scope of the first program) — **DELIVERED**: one shared
review-render path so the reviewed frame and the locked frame cannot drift again (D13 died with
the duplication); a review set covering four maps, the garage, a vehicle in frame and the
player's own eye height (D2, D11); a pixel-side meter with a recorded baseline; locks that bite
(D1 — the FLOOR/TARGET pairs).

**W1 — Image.** The sky's visible band (D3), the missing dark mass (D4), exposure and grade per
look — driven by measured percentiles, not by feel. All three looks on all four maps clear the
bar; the "roll stays" decision means none of them gets a pass.

**W2 — World content.** Unpin the tree LOD (D5), finish the four placeholder content kinds (D6),
clump the grass (D7), correct the imported trunk tint (D14). The honesty doctrine binds here: a
`TreeLine` that gains a shelterbelt's geometry keeps its blocking volume **bit for bit**, and each
PR carries the before/after AABB comparison that proves it.

**W3 — Vehicles.** Baked contact AO across the whole fleet (D8), a curvature/edge-wear term (D15),
the dirt lane wired into battle (D9), the team-colour key fixed (D10), the showcase showing paint
(D17).

**W4 — Garage.** The room's own content brought into frame (D16 — done, by lowering the hero
pitch), the light pools made visible, the hero separated from its background. D20 was a range
problem and the range is fixed: spread 0.275 → 0.572, p95 0.276 → 0.593, bright 0.3% → 2.3%. The
hero's separation from its background is no longer an opinion either — the garage has a subject
crop now, and it measures **median 0.161 against the room's 0.089, 1.81x**, floored at 1.4x by
`the_vehicle_stays_readable_on_the_side_the_sun_never_touches`.

What is LEFT for this wave is the **dark plane: 80.4% against a 75% bound**, and it is the one
number none of the above could move — a specular term and an albedo treatment cannot lift shade,
only light can. The full picture of what the garage still owes, measured end to end, is
[garage-pipeline-audit-2026-08-09.md](garage-pipeline-audit-2026-08-09.md). Also outstanding: the
`SIGNAL` red of the Battle button falling outside the palette.

**The claim that the garage UI "is the strongest work in the game and is not to be touched" was
made without a measurement and did not survive one.** It rested on a review render from
2026-07-11 that predated the map picker, and the UI had no picture lock of any kind (D24): the
top bar's plate ended above its own tab row, so both screen tabs rendered on the hangar wall, and
one of them answered no click. There is now a `garage_screen` golden, and opinions about this
screen answer to it.

**W5 — Per-map identity.** Four ground palettes, four sets of times of day, four sets of goldens.
D12 (dual vegetation languages) closed under Świat 2.0 F0 — flora is procedural-only; the bush
is `TreeSpecies::Bush`, not a sourced CC0 asset.

## Verification

The merge gate is `./scripts/verify.ps1` locally; CI billing is blocked. A cold full run exceeds
ten minutes, so stage fmt / clippy / test separately.

Two traps this program must not fall into, both already paid for once:

- **A pipe eats the exit code.** `cargo test ... | grep ...` reports grep's status, not cargo's,
  and `| head -N` silently truncates the result summary. Capture the real exit code and count the
  binaries, or the green is imaginary.
- **A look change is not verified until it is looked at.** Re-record with
  `WOT_UPDATE_GOLDENS=1 cargo test -p client --test look_goldens`, then read the new PNGs against
  the old ones in the diff. The always-on CPU statistics run without a GPU and catch structure,
  never taste.

Human review: `cargo run -p client --example probe -- {prokhorovka,bystra,orliny,ostrogorsk}_views`,
`--example probe -- garage_hangar_review`, `--example probe -- sky_probe`, `--example probe -- closeup_probe`.
Perf: `--release --example probe -- perf_capture`, `--example probe -- flora_frame_probe` (1080p against the
16.667 ms budget), `--example probe -- detail_cost_probe`. Every raised geometry budget lands with a
min-spec measurement in the PR description — one look, and a dropped frame is a game bug.
