# Art Direction 3.0 — Pulling The Picture Up To The Policy

[art-direction-policy.md](art-direction-policy.md) states the target look and carries the locks.
**This document states why the shipped picture does not obey it, and in what order that gets
fixed.** The policy is the bible; this is the campaign. When the register below is empty and every
`FLOOR` has met its `TARGET`, this document becomes history and the policy stands alone.

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

A fourth failure is the direct consequence of (3) and is worth stating on its own: because the
review example and the golden harness each hand-roll ~50 lines of identical scene setup, they
drifted, and **both forgot to bind the foliage atlas**. The locked reference frames render
imported flora as untextured white (D13). The document's own promise — "the frame a human reviews
is exactly the frame the harness locks" — was a convention, and conventions rot.

## The decisions this program is built on

Taken 2026-07-26. Each is a deliberate commitment, not a default.

| Decision | Choice |
|---|---|
| **Course** | **Pull everything up to the painterly target.** No tier of content is exempt: flora moves to imported CC0, rocks get a generator, `TreeLine` / `Wreck` / `RailCover` get real geometry, the sky's visible band is rebuilt, vehicles get full surface narrative. Procedural trees step back to the backdrop ring they are actually good at. |
| **First program** | **Calibrate before content.** Build the instrument, then move the numbers. Tuning four maps and twelve looks against a broken baseline is work thrown away. |
| **Weather variants** | **The roll stays; every look comes up.** `apps/server/src/match_info.rs::pick_weather` keeps choosing at random, so no variant may be a "worse day" — three looks across four maps each hold the bar on their own. This is the policy's "equally authored days" taken literally. |
| **Per-map identity** | **Four identities, not one look reused.** Each map earns its own ground palette, its own times of day, its own review views and its own goldens. |

## Defect register

Every entry is reproducible from the repository or from a probe render. The wave column says where
it closes.

| # | Defect | Evidence | Wave |
|---|---|---|---|
| D1 | Locks computed from the profile, not the picture; dark-plane floor is 0.1% of pixels | `look_locks.rs`, `look_goldens.rs:218` | W0 |
| D2 | Review set covers 1 map of 4, contains no vehicle, shoots from 14 m, and has no garage entry | `review_views.rs:22-64` | W0 |
| D3 | The milky sky is **structural**, not a tuning error. Clouds live in `smoothstep(0.04, 0.32, dir.y)`; the bottom is forced to `sky_horizon_rgb * 1.06`. A hull-height camera sees `dir.y ≈ 0..0.2`, so the authored zenith `[0.15, 0.32, 0.62]` **never appears in play** and the visible band is the fog colour, which must be pale by construction | `sky.wgsl:150,188` | W1 |
| D4 | No dark mass: the field is uniformly sunlit, and cloud shadows run at 0.25–0.3 strength over a very large scale | `lighting.rs` profiles | W1 |
| D5 | **Every** battle tree is pinned to `TreeLod::Mid`, where lobes are raw 20-triangle icosahedra, `trunk_sides = 5`, and limbs are skipped entirely. The limbed, subdivided `TreeLod::Close` (180–1200 tris) has **no shipping caller at all** | `foliage.rs:97`, `tree.rs:220-235` | W2 |
| D6 | Four content kinds still render as a bare cuboid: `TreeLine` (on Prokhorovka, solids of **44 × 10 × 6 m**), `RailCover`, `Wreck` (a "knocked-out tank" as a brown 3.4 × 1.6 × 6.2 m box), and `SceneryKind::Rock` — **there is no rock generator** | `battlefield.rs:630-659`, `foliage.rs:268` | W2 |
| D7 | Grass has no clumping term: 28 candidates per 8 m cell at uniformly random positions, accepted by splat weight | `grass.rs:108-203` | W2 |
| D8 | Baked contact AO exists for the **T-54 only**; there is **no curvature/edge-wear term anywhere in the repository**; dust is confined to the running gear | `surface_bake.rs`, `vehicle.wgsl:310` | W3 |
| D9 | `VehicleVariation` carries `dirt` / `snow` / `camo` lanes that are **never populated in battle** | `variation.rs:105-116` | W3 |
| ~~D10~~ | ~~Team colour keys on `tank.id == player_tank` instead of `TeamId`~~ — **CLOSED (PR #317)**: both paths now read `PresentationTank::team` through one shared rule; identity still decides the gun. In a 7v7 this had six of the thirteen other tanks wearing enemy paint | `render_frame.rs` | W3 |
| D11 | The garage has **no golden and no review view**; `garage_workshop` is a dead look with no caller | `review_views.rs`, `lighting.rs:517` | W0/W4 |
| D12 | Two vegetation languages share a frame: imported CC0 `stylized-pine` beside a procedural distance LOD. `FloraBush` is look-gate rejected, so maps still scatter procedural `Bush` | `docs/urban-map-program.md:19-20` | W2/W5 |
| D13 | **The locked goldens render imported flora as WHITE.** `look_goldens.rs`, `prokhorovka_views`, `orliny_views`, `bystra_views` and `vehicle_lineup` never call `set_foliage_atlas`, so flora samples the 1×1 `[255,255,255,255]` default. The live client is correct (`app/render.rs:334`), as is `ostrogorsk_views`. Visible whole-frame in `target/orliny_pine_belt.png` | `foliage_atlas.rs:36` | W0 |
| D14 | The imported `stylized-tree` has a glaring orange-red trunk that falls outside the saturation window. **Not** a colour-space bug — the atlas uploads as `Rgba8UnormSrgb` and mips are alpha-weighted in linear. It is the asset's own colour, correctable by per-vertex tint without a re-import | `foliage.rs:75-79` | W2 |
| D15 | Outside the T-54 the fleet offers nothing to look at up close: unbroken plates, no weld seams, grab handles, tow cable, spare track or vision blocks; hull and turret read as two different paints (cast vs rolled split too far); running gear is a black void with no contact | `target/closeup_probe/centurion_flank.png` | W3 |
| D16 | The garage room's content — catwalk, crane, workbench, stores, six worklamps, skylights — is built and sits **entirely outside** the hero framing, which points at the emptiest wall. The hero does not separate in value from its background — **PARTLY CLOSED**: `HERO_ORBIT_PITCH` 0.28 → 0.13 brought the gallery band, the bay gate and the frosted panes into frame (at 0.28 the top of the frame sat exactly on the horizon through the pivot, so *everything* above the eye was out of shot). The value separation is still owed and rides with D20 | `garage_render.rs:142`, `hangar_gallery.rs`, `hangar_props.rs` | W4 |
| D22 | **The hero was parked at the camera's own bearing.** `HERO_ORBIT_YAW` and the parked `yaw_rad` were both 0.6, so the "three-quarter" the comments claimed was a head-on elevation with the barrel bisecting the hull — the one angle at which the whole fleet looks alike. **CLOSED**: `hangar::HERO_PARK_YAW = HERO_ORBIT_YAW + 0.65`, read by the live garage, the golden and the example | `hangar.rs`, `garage_render.rs`, `review_views.rs` | W4 |
| D23 | **The human-review example hard-coded the framing** — `(0.60, 0.28, 14.0)` and a 32° lens, copied beside the constants `ecc0777` had just centralised so that "a reframing moves the played picture and the locked picture together". A reframing would have moved the golden and left the reviewed frame behind, which is D13's disease exactly. **CLOSED**: the example reads `hero_orbit_eye()` / `HERO_FOV_DEGREES` | `examples/garage_hangar_review.rs:70` | W4 |
| D24 | **The garage UI had no picture lock of any kind.** `garage_hero` is the room only; the overlay was covered solely by unit tests over rect arithmetic, which cannot see a control drawn off its plate. It had shipped for months with the top-bar plate ending at y=0.86 while both screen tabs hit-tested 0.785–0.845 — GARAGE and TECH TREE rendered as dim text on the hangar wall, and GARAGE answered no click at all. **CLOSED**: `garage_screen` golden + the plate reaches its own tab row | `review_views.rs`, `panels/topbar.rs` | W4 |
| D25 | **The VEHICLE column did not carry the game's own promise.** Six rows — HP, kW, km/h, °/s, penetration, reload — with no dispersion, no aim time and no armour: the "no ±25% roll, the gun groups where it is pointed" pitch was absent from the screen where the player picks a gun and presses Battle. **CLOSED**: nine rows, plate derived from `STAT_ROWS` | `panels/stats.rs` | W4 |
| D17 | The fleet showcase renders vehicles in pastels (powder blue, lavender, pink, cream) — the canonical "no clones" render does not show paint | `target/vehicle_lineup.png` | W3 |
| D18 | **Orliny Pereval has no light of its own.** Its blueprint's `ClearAfternoon` preset resolves to `bystra_clear_afternoon` — the mountain pass wears the river valley's afternoon. The borrowed look is now locked, so the day it gets its own is visible in the diff | `blueprints/orliny-pereval.map.ron:114-119`, `weather.rs::preset_lighting` | W5 |
| D20 | **The garage has almost no bright plane** — 0.3% of the hero frame sits above the bright threshold, against a 2% target. Was 0.00%; the reframing (D16) brought the frosted panes into shot and they are the entire gain, being the only emissive surface the hero lens contains. **The reframing did not close this, and the percentiles say why**: p50 0.119, p95 0.276 — the whole picture is a narrow band pressed against the 0.25 dark/mid boundary, and the floor a player reads as light grey measures 0.238, a hair on the dark side. Where the lens points decides what is IN the picture; the light rig and the grade decide how far apart its values are. **This closes with light in the room, not with a camera** | `goldens/look/garage_hero.png`, `look_goldens.rs` `GARAGE_BRIGHT_TARGET` | W4 |
| D21 | **Cloud shadows never run in the shipped game.** `LightingQuality::canonical()` sets `cloud_shadows: false`; only the dev-only `rich()` enables them, and `gpu_layout` zeroes `sky_params.x` when the tier says no. Every profile's `cloud_shadow_strength` is therefore dead data in the configuration players get — and the arithmetic says D4 **cannot be closed without it**: with ambient+fill+rim carrying ~0.36 of a flat ground's radiance, only near-total key occlusion pushes sunlit steppe below the dark threshold. Enabling it is a per-item buy-back against the one-look budget and needs a min-spec measurement | `lighting_quality.rs:81`, `gpu_layout.rs:290` | W1 |
| D19 | Grass scatters **onto the city street**: the Ostrogorsk canyon reads as a meadow between tenements, and `RoadSurface::Cobble` reads as a dirt path rather than granite setts. Tenement facades are flat boxes with painted window rectangles over a hard black plinth | `goldens/look/ostrogorsk_canyon.png`, `grass.rs::vegetation_weight` | W2 |

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

| frame | dark | mid | bright | p05 | p50 | p95 | spread | sat | local | band |
|---|---|---|---|---|---|---|---|---|---|---|
| `prokhorovka_clear_afternoon` | 1.0% | 40.1% | 58.9% | 0.452 | 0.623 | 0.889 | 0.437 | 0.300 | 0.0077 | +0.160 |
| `prokhorovka_golden_evening` | 18.1% | 47.3% | 34.6% | 0.143 | 0.497 | 0.702 | 0.559 | 0.426 | 0.0104 | +0.294 |
| `prokhorovka_overcast` | 0.9% | 49.4% | 49.7% | 0.365 | 0.570 | 0.713 | 0.348 | 0.176 | 0.0049 | +0.205 |
| `prokhorovka_evening_midfield` | 26.6% | 38.1% | 35.3% | 0.110 | 0.358 | 0.697 | 0.587 | 0.451 | 0.0105 | +0.361 |
| `prokhorovka_grass_midfield` | 8.1% | 49.1% | 42.8% | 0.070 | 0.574 | 0.873 | 0.803 | 0.315 | 0.0087 | +0.213 |
| `prokhorovka_evening_contact` | 34.6% | 40.6% | 24.8% | 0.064 | 0.302 | 0.681 | 0.617 | 0.516 | 0.0122 | +0.347 |
| `bystra_clear_afternoon` | 4.6% | 50.9% | 44.5% | 0.267 | 0.494 | 0.761 | 0.494 | 0.303 | 0.0086 | +0.311 |
| `bystra_rain` | 7.4% | 58.8% | 33.8% | 0.147 | 0.433 | 0.676 | 0.529 | 0.221 | 0.0052 | +0.276 |
| `bystra_dawn_fog` | 25.1% | 28.8% | 46.1% | 0.100 | 0.377 | 0.779 | 0.679 | 0.234 | 0.0075 | +0.463 |
| `bystra_town_lane` | 10.6% | 44.6% | 44.8% | 0.079 | 0.473 | 0.762 | 0.683 | 0.302 | 0.0076 | +0.274 |
| `orliny_clear_afternoon` | 3.6% | 55.2% | 41.1% | 0.272 | 0.417 | 0.827 | 0.556 | 0.296 | 0.0108 | +0.435 |
| `orliny_golden_evening` | 49.2% | 15.2% | 35.6% | 0.087 | 0.254 | 0.815 | 0.728 | 0.407 | 0.0130 | +0.591 |
| `orliny_overcast` | 2.1% | 54.2% | 43.7% | 0.327 | 0.425 | 0.720 | 0.393 | 0.209 | 0.0062 | +0.328 |
| `orliny_pine_belt` | 6.2% | 48.3% | 45.5% | 0.156 | 0.487 | 0.777 | 0.621 | 0.292 | 0.0107 | +0.283 |
| `ostrogorsk_clear_afternoon` | 4.4% | 50.9% | 44.7% | 0.286 | 0.534 | 0.847 | 0.561 | 0.270 | 0.0106 | +0.301 |
| `ostrogorsk_golden_evening` | 43.2% | 19.4% | 37.4% | 0.059 | 0.276 | 0.800 | 0.741 | 0.379 | 0.0110 | +0.555 |
| `ostrogorsk_overcast` | 7.7% | 48.8% | 43.5% | 0.190 | 0.430 | 0.713 | 0.523 | 0.159 | 0.0065 | +0.306 |
| `ostrogorsk_rain` | 7.5% | 57.7% | 34.8% | 0.181 | 0.397 | 0.676 | 0.495 | 0.173 | 0.0061 | +0.302 |
| `ostrogorsk_canyon` | 4.8% | 62.8% | 32.4% | 0.256 | 0.526 | 0.880 | 0.624 | 0.259 | 0.0079 | +0.228 |
| `garage_hero` | 90.0% | 9.7% | 0.3% | 0.001 | 0.119 | 0.276 | 0.275 | 0.251 | 0.0036 | −0.141 |
| `garage_screen` | 89.1% | 10.5% | 0.4% | 0.020 | 0.106 | 0.281 | 0.261 | 0.180 | 0.0071 | −0.089 |

### What the baseline says

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

**The garage is the outlier on every axis.** 90.0% dark, **0.3% bright**, the narrowest spread in
the set (0.275) and the lowest local contrast (0.0036). Its dark share sits **at the 90% "one
plane swallowed the picture" bound** — a second debt beside D20, and the reason W4 is about light
in the room rather than paint on the hero.

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
prokhorovka_clear_afternoon: dark plane 0.010, target 0.080 (short by 0.070, W1)
prokhorovka_clear_afternoon: spread     0.437, target 0.450 (short by 0.013, W1)
prokhorovka_overcast:        dark plane 0.009, target 0.080 (short by 0.071, W1)
prokhorovka_overcast:        spread     0.348, target 0.450 (short by 0.102, W1)
bystra_clear_afternoon:      dark plane 0.046, target 0.080 (short by 0.034, W1)
bystra_rain:                 dark plane 0.074, target 0.080 (short by 0.006, W1)
orliny_clear_afternoon:      dark plane 0.036, target 0.080 (short by 0.044, W1)
orliny_overcast:             dark plane 0.021, target 0.080 (short by 0.059, W1)
orliny_overcast:             spread     0.393, target 0.450 (short by 0.057, W1)
orliny_pine_belt:            dark plane 0.062, target 0.080 (short by 0.018, W1)
ostrogorsk_clear_afternoon:  dark plane 0.044, target 0.080 (short by 0.036, W1)
ostrogorsk_overcast:         dark plane 0.077, target 0.080 (short by 0.003, W1)
ostrogorsk_rain:             dark plane 0.075, target 0.080 (short by 0.005, W1)
ostrogorsk_canyon:           dark plane 0.048, target 0.080 (short by 0.032, W1)
garage_hero:                 bright     0.003, target 0.020 (short by 0.017, D20, W4)
garage_hero:                 dark plane 0.900, target ≤ 0.750 (over by 0.150, D20, W4)
```

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

**Refused at this cost.** Under a policy that calls a dropped frame a game bug, this is not a
buy-back; it is a loan. The knob and these numbers ship anyway, because the next person to ask
"why is the steppe flat?" should find the measurement instead of repeating it.

The cost is concentrated where it would be: `cloud_shadow()` in `terrain.wgsl` runs a domain warp
plus five `value_noise` evaluations **per terrain fragment**, which is why an open map with heavy
scatter pays 5 ms while an empty midfield pays 0.9. That shape is the opening: the same coverage
baked into a small scrolling R8 texture is one sample instead of eight ALU-heavy taps. Making it
cheap is the work, not arguing the budget — tracked as W1's next item.

Until then, D4's dark mass must come from levers that are already free: the grade (black point,
contrast), the ambient/key balance, and the shadow-casting content W2 adds.

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

**W0 — Instrument** (the approved scope of the first program). One shared review-render path so
the reviewed frame and the locked frame cannot drift again (and D13 dies with the duplication); a
review set covering four maps, the garage, a vehicle in frame and the player's own eye height; a
pixel-side meter with a recorded baseline; locks that bite.

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
pitch), the light pools made visible, the hero separated from its background. What is LEFT is the
part a camera cannot do: **D20 is a range problem.** The garage frame is a narrow band pressed
against the dark/mid boundary — p95 0.276, spread 0.275, the narrowest in the set — so the room
needs light, not a different angle. The frosted panes are the only emissive surface the hero lens
reaches; the six worklamps, the second-bay strip and the skylights are all still outside it or
above it. Also outstanding: the `SIGNAL` red of the Battle button falling outside the palette.

**The claim that the garage UI "is the strongest work in the game and is not to be touched" was
made without a measurement and did not survive one.** It rested on a review render from
2026-07-11 that predated the map picker, and the UI had no picture lock of any kind (D24): the
top bar's plate ended above its own tab row, so both screen tabs rendered on the hangar wall, and
one of them answered no click. There is now a `garage_screen` golden, and opinions about this
screen answer to it.

**W5 — Per-map identity.** Four ground palettes, four sets of times of day, four sets of goldens.
Closing D12 needs a sourced CC0 bush to replace the rejected `FloraBush` — the only item in this
program with an external dependency, so it starts early and runs in parallel.

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
