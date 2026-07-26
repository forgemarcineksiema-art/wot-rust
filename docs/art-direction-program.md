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
| D10 | Team colour keys on `tank.id == player_tank` instead of `TeamId`, so friendly AI renders in the enemy's paint. A readability bug, not only a look one | `render_frame.rs:28,73` | W3 |
| D11 | The garage has **no golden and no review view**; `garage_workshop` is a dead look with no caller | `review_views.rs`, `lighting.rs:517` | W0/W4 |
| D12 | Two vegetation languages share a frame: imported CC0 `stylized-pine` beside a procedural distance LOD. `FloraBush` is look-gate rejected, so maps still scatter procedural `Bush` | `docs/urban-map-program.md:19-20` | W2/W5 |
| D13 | **The locked goldens render imported flora as WHITE.** `look_goldens.rs`, `prokhorovka_views`, `orliny_views`, `bystra_views` and `vehicle_lineup` never call `set_foliage_atlas`, so flora samples the 1×1 `[255,255,255,255]` default. The live client is correct (`app/render.rs:334`), as is `ostrogorsk_views`. Visible whole-frame in `target/orliny_pine_belt.png` | `foliage_atlas.rs:36` | W0 |
| D14 | The imported `stylized-tree` has a glaring orange-red trunk that falls outside the saturation window. **Not** a colour-space bug — the atlas uploads as `Rgba8UnormSrgb` and mips are alpha-weighted in linear. It is the asset's own colour, correctable by per-vertex tint without a re-import | `foliage.rs:75-79` | W2 |
| D15 | Outside the T-54 the fleet offers nothing to look at up close: unbroken plates, no weld seams, grab handles, tow cable, spare track or vision blocks; hull and turret read as two different paints (cast vs rolled split too far); running gear is a black void with no contact | `target/closeup_probe/centurion_flank.png` | W3 |
| D16 | The garage room's content — catwalk, crane, workbench, stores, six worklamps, skylights — is built and sits **entirely outside** the hero framing, which points at the emptiest wall. The hero does not separate in value from its background | `garage_render.rs:142`, `hangar_gallery.rs`, `hangar_props.rs` | W4 |
| D17 | The fleet showcase renders vehicles in pastels (powder blue, lavender, pink, cream) — the canonical "no clones" render does not show paint | `target/vehicle_lineup.png` | W3 |
| D18 | **Orliny Pereval has no light of its own.** Its blueprint's `ClearAfternoon` preset resolves to `bystra_clear_afternoon` — the mountain pass wears the river valley's afternoon. The borrowed look is now locked, so the day it gets its own is visible in the diff | `blueprints/orliny-pereval.map.ron:114-119`, `weather.rs::preset_lighting` | W5 |
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
- **`TARGET`** — what the policy demands. Not asserted yet; **printed on every run** as a distance.

`verify` therefore states the debt out loud on every invocation instead of burying it in a
comment, and a wave is done when its `FLOOR` has been raised to meet its `TARGET`. A PR that moves
a bound re-blesses it in the same diff and says, in its description, **what changed about the
PICTURE** — not only what changed about the code.

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

**W4 — Garage.** The room's own content brought into frame, the light pools made visible, the hero
separated from its background (D16). The garage UI is the strongest work in the game and is not to
be touched, beyond the `SIGNAL` red of the Battle button falling outside the palette.

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

Human review: `cargo run -p client --example {prokhorovka,bystra,orliny,ostrogorsk}_views`,
`--example garage_hangar_review`, `--example sky_probe`, `--example closeup_probe`.
Perf: `--release --example perf_capture`, `--example flora_frame_probe` (1080p against the
16.667 ms budget), `--example detail_cost_probe`. Every raised geometry budget lands with a
min-spec measurement in the PR description — one look, and a dropped frame is a game bug.
