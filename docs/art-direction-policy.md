# Art Direction Policy — "Steel Under an Evening Sky"

The target look of the whole game, in words, with numbers, and with locks. Every visual
system — terrain, sky, vehicles, FX, post — serves ONE picture:

> **A late-afternoon Eastern-Front oil painting that stays militarily readable.**
> Repin-school landscape values under a low raking sun. Not a photoreal sim screenshot,
> not a stylized cartoon. The `prokhorovka_golden_evening` profile is the reference frame
> of the engine; the art-direction program exists to pull every other system and profile
> up to it — the other looks (hazy noon, lead overcast, valley dawn, rain squall) are
> equally *authored* days, never merely dimmer ones.

This document is policy, not commentary: each rule below carries a locking test. Widening
any bound is a deliberate art-direction diff with a justification in the PR, never a tuning
accident. The lock set:

- **CPU, always-on** — `crates/render/renderer_api/tests/look_locks.rs` (statistical locks
  on the profiles themselves) and `recorded_goldens_hold_the_value_structure` in
  `crates/apps/client/tests/look_goldens.rs` (statistical locks on the committed golden
  pixels; no GPU needed).
- **GPU, opt-in** — `look_goldens_match_their_recordings` in the same file, gated by
  `WOT_LOOK_GOLDENS=1` (byte-exact per machine, like the Forge studio goldens); re-record
  with `WOT_UPDATE_GOLDENS=1`. The rendered views are the canonical review set
  (`client::prokhorovka_review_views`), shared with the `prokhorovka_views` example so the
  frame a human reviews is exactly the frame the harness locks.

## The seven rules

### 1. Value structure first

The image reads in three separated value planes: a dark shade mass, a mid field, a bright
sky. Measured on graded display luma with the reference mid albedo `[0.28, 0.27, 0.24]`:

- deep shade (ambient + fill only) stays **at or below 0.25** — and may legitimately reach
  true black on the golden looks (that is what the black point is for; anti-crush is locked
  separately by mid-grey survival),
- the sunlit field sits **at least 0.06 above the shade**, and lands in **[0.28, 0.58]** on
  every clear-sky profile,
- the sky horizon stays **at least 0.10 above the field**, and lands in **[0.60, 0.95]** on
  every clear-sky profile. An overcast lid compresses the range but never re-orders it.

On the golden frames: every canonical view keeps all three planes alive and no plane
swallows more than 90% of the picture.

The dark plane is where this policy sits furthest from the shipped picture, and that
distance is now recorded rather than excused. Each value bound is a **`FLOOR` / `TARGET`
pair**: `FLOOR` is what today's frames actually achieve and is asserted so it cannot
regress; `TARGET` is what this rule demands and is **printed on every run** as a remaining
distance. A wave is finished when its `FLOOR` has been raised to meet its `TARGET`. The
campaign that closes the gap — and the evidence that the gap is real — is
[art-direction-program.md](art-direction-program.md).

Silhouette readability at combat range is the second half of this rule and is already
locked map-wide: no weather look may fog away more than 35% of a spotted target's contrast
at the 400 m view range (`crates/apps/client/src/scene/weather.rs`). That bound is
inviolable — atmosphere is depth, never concealment.

### 2. The saturation window

The ground plane is muted; chroma lives in the sky and in the light. Grass is grey-green,
never lawn-green. The policy ground swatches (linear albedo, saturation ≤ 0.45):

| swatch | albedo |
|---|---|
| steppe grass | `[0.30, 0.33, 0.22]` |
| dry straw | `[0.45, 0.40, 0.26]` |
| tilled dirt | `[0.32, 0.27, 0.21]` |
| chalk break | `[0.55, 0.54, 0.50]` |
| wet mud | `[0.20, 0.17, 0.14]` |

Terrain material work binds its layer albedos to this discipline. On the grade side: no
outdoor profile lifts saturation past **1.30** (tighter than the generic display envelope),
and the display transform never invents chroma — grey in, grey out, exactly.

Deliberate accents are exempt by design: tracer, fire, team tint — the eye goes where the
battle is.

### 3. Warm key, cool fill — always

The colour axis of the whole direction. The sun key is warmer (R/B) than the sky fill in
every outdoor profile; an overcast lid compresses the split but never inverts it, and every
clear-sky look keeps it decisive: key warmth > 1.0, fill warmth < 1.0. Holistically locked
on pixels too: the golden-evening frame out-warms the overcast frame by ≥ 10% mean R/B.

### 4. Atmospheric depth is mandatory

Every outdoor frame shows at least three distinguishable depth planes — near ground, mid
field, horizon haze. Structurally: every outdoor profile carries real fog
(`fog_density > 0`), and the fog colour IS the horizon colour (one atmosphere, by
construction — distant ground and sky meet in the same haze). The horizon always out-lumes
the zenith. The 400 m / 0.35 fairness bound from rule 1 caps how much air the look may put
between the player and a spotted target.

### 5. Nothing is clean, nothing is noisy

Every surface carries exactly two detail octaves: a macro variation around 2–5 m and a
micro grain around 0.3–0.6 m (the existing `material_detail` discipline of the scene and
vehicle shaders). No surface is flat — that reads as cheap; no surface shimmers — that
reads as noise. This is the anti-low-poly and the anti-cheap-trick rule in one sentence,
and the standing answer to "should we add TAA": with disciplined octaves there is no
subpixel noise to hide.

### 6. Light is the only bling

Post-processing exists to serve light, not to dress the lens: bloom (when it lands) is
threshold-free and energy-conserving — only genuinely HDR sources glow (sun, tracers,
fires, specular glints); vignette stays at or under 0.15 and is profile-owned. **Never:**
chromatic aberration, film grain, lens dirt, motion blur. The battle must stay readable at
all times; the camera is an eye, not a camera.

### 7. One picture

Every pass grades through the single shared display transform (exposure → ACES-lite →
black point → saturation → contrast, `lighting_common.wgsl`, CPU-mirrored by
`SceneLighting::grade_reference`). No pass may tone-map independently. The grade is profile
data with envelope locks (`every_profile_grades_within_the_sane_display_envelope`); no look
constant hides in a shader.

## The canonical review set

`client::prokhorovka_review_views` — the hill panorama under the hazy noon, the golden
evening and the dry overcast, plus a golden-evening mid-field vantage. Render them with
`cargo run -p client --example prokhorovka_views`; the goldens live in
`crates/apps/client/tests/goldens/look/`. Every view added is a view locked.

What the set must cover, because a look this policy cannot see is a look it cannot govern:

- **every shipped map**, not one of them. A per-map identity that no frame locks is a wish.
- **every weather variant the map declares.** `pick_weather` rolls at random, so a variant
  the review set skips is a variant the player meets unreviewed.
- **a vehicle in frame.** The subject of this game is a tank; a landscape-only review set
  cannot catch a hero that fails to separate from its ground.
- **the player's own eye height.** A panorama shot from above the chase camera judges a
  frame nobody plays.
- **the garage.** It is the first thirty seconds of contact with the game and grades through
  the same display transform as rule 7 demands; it belongs under the same locks.

**One render path.** The reviewed frame and the locked frame must be produced by the same
code, not by two hand-rolled copies of the same setup. They were once two copies, they
drifted, and both silently stopped binding the foliage atlas — so the committed goldens
render imported flora as untextured white. A shared harness is the structural form of the
promise; the convention was not enough.

## The garage is the same picture

Rule 7 says every pass grades through one display transform, and the garage obeys it — but
the garage is not merely "the battle look indoors". It is a **studio**, and the studio's job
is the opposite of the battlefield's: the field hides a tank, the hangar sells one.

- **The hero is the brightest, most contrasted, most detailed thing in frame.** If the room
  out-reads the vehicle, the shot has failed no matter how well lit the room is. The parked
  hull must separate in value from the wall behind it.
- **Light that is authored must be light that is seen.** The room hangs its own lamps and
  skylights, and the profile's local pools are placed to coincide with those housings. Pools
  the camera never frames are pools that do not exist.
- **The framing is part of the look.** A hero framing that points at the emptiest wall makes
  a furnished workshop read as a grey box. The camera is authored with the same care as the
  palette.
- **The UI is instrument, not decoration.** `hud/theme.rs` — flat graphite, chamfered
  corners, hairline rules, one amber signal accent, no gradients or glow. It grades through
  nothing: HUD is drawn after the display transform and answers to the theme locks alone.

Garage profiles obey the same envelope ceilings as the outdoor looks (rule 6), and the
hangar view belongs in the canonical review set above.

## How a look change lands

1. Change the profile / shader / content with its own tests.
2. `cargo test -p renderer_api --test look_locks` — the bible still holds.
3. `WOT_UPDATE_GOLDENS=1 cargo test -p client --test look_goldens` — re-record, then eyeball
   the new PNGs against the old ones in the diff.
4. The always-on golden statistics re-run in verify — if the new picture lost its value
   structure, the gate fails regardless of what the diff author thought of it.
5. Say in the PR what changed about the PICTURE, not just the code.
