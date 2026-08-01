# Visual review — 2026-08-01

The first time anyone in this audit looked at what the game actually renders. Sources: the 18 look
goldens (`client/tests/goldens/look/`), the studio goldens (`tools/tests/goldens/studio/`), the
design-review captures (`output/design-review/`), a fresh
`cargo run -p client --release --example screenshot`, and four frames from the scripted battle.

## The UI: good information architecture, placeholder production

**Correction to an earlier draft of this document, which called the garage and HUD "finished work".
That was wrong.** It confused two different things.

**What is genuinely good is the information architecture.** The panel layout makes sense, the
reticle communicates the right things (dispersion ring, reload arc, range, effective mm), the module
row is the right idea, the vehicle carousel reads clearly. Someone thought carefully about *what*
the player should see. That thinking is worth keeping.

**The production value is a mockup.** Panels are flat grey rounded rectangles — no depth, no
material, no layering. Typography has one weight and two sizes: there is no hierarchy, only
"bigger/smaller". Icons are simple line glyphs. The hangar is grey boxes, a yellow floor line and a
flat disc, with the tank standing in it **flat-lit** — no key light, no rim, no reflection. The
BATTLE button is a single salmon rectangle. The minimap is a grey square with dots. Nothing has
animation, weight, or a response to being touched.

**The UI and the world are at the same level of finish.** The UI simply has better-considered
content.

## Defects, in order of how much they hurt

### 1. The gun barrel is far too thin
A D-10T reads as an aerial, not a 100 mm gun. Visible in **every** frame of **every** vehicle; in
the garage front view it is the first thing the eye lands on. One number in the blueprint.

### 2. The T-54 turret front reads as a mushroom
The side silhouette is good. The front is one smooth continuous lump — no casting character, no
cheek definition. Surprising after Model Idealny closed with 14 anchors locked and six iterations on
the mantlet alone, and it says the program's instruments measured the profile better than the face.

### 3. The grass ring boundary is visible
`prokhorovka_golden_evening`, right-hand slope: a hard diagonal band where the grass population
ends. It ruins the best-looking frame in the set. Needs a fade, not a cut.

### 4. The grass tiles
One tuft, one rotation, one grid step, plainly repeating across the foreground and midfield of
`battle_hud_third_person`. Needs per-instance rotation and scale variance.

### 5. Terrain reads as a playing field
In the third-person battle frame there is **nothing** between the player and an enemy 214 m away —
no fold, no dip, no reason to steer. This is `cell_m = 5.0` seen with the eye, and the effect is
stronger than the audit of the code suggested.

### 6. Ostrogorsk's trees are broken
Orange trunks **as wide as the canopy is tall** — they read as termite mounds — standing in a
perfectly straight receding line with no jitter. The generator is visible; a treeline is not.

### 7. Bystra's river is a flat ribbon with a hard bank
No shoreline transition: a straight blue cut across the meadow, plus a white smear along the far
bank that reads as an artifact.

### 8. Trees are origami with ink-stain shadows
Large flat-shaded polygons with a near-black unlit underside, dropping hard-edged polygonal
shadows.

### 9. Distant buildings are untextured slabs
Two large **black** rectangles on the horizon in `target/scene.png`; a flat green slab on
Prokhorovka. They read as holes in the scene, not structures.

### 10. Team tints read as toys
Mint green against maroon. Legible, but not military.

### 11. Shadows are offset soft blobs
The enemy tank's shadow is a large soft ellipse displaced to one side, unrelated to the hull shape.

---

## Cheap wins

Items 1, 3, 4, 6, 7 are hours of work each and visible in every frame afterwards. They belong in the
first wave that touches presentation, ahead of anything structural.

Items 2 and 9 are real modelling and material work — later, but they are what stands between "reads
as a prototype" and "reads as a game".

---

# The systemic problem: this is not a style, it is the absence of one

The individual defects above are symptoms. "Flat-shaded low-poly under soft light" is not an
artistic decision anyone made — it is what remains when the geometry is procedural and the materials
were never written. Five specifics:

**1. Nothing has a material.** Tank, ground, trees, buildings all read as the same matte plastic. A
tank should read as **steel**: a hard specular response, dirt in the recesses, wear on the edges,
weld seams. Nothing in any frame says "metal".

This stings because **the PBR pipeline is built and unused** — `vehicle.wgsl` carries per-material
albedo and roughness, the Forge artifact carries normal/AO/roughness maps, twelve material roles are
defined. Three of the twelve **do not even reach the shader** (see register B3).

**2. The lighting is flat.** Cascaded shadows, SSAO, HDR and bloom are all present, and the result is
pale and low-contrast. Shadows are soft blobs; the sun does not model form. In the contact frame the
T-54's side is a **black void** — that is not dramatic lighting, it is clipping to zero. There is no
key/fill relationship, just one even bath.

**3. The palette takes no position.** Pale green, pale blue, mint tank, maroon tank — it reads as a
default engine scene. WoT is desaturated with a warm/cool split; War Thunder plays naturalism;
BeamNG plays neutral realism. The tools exist here (exposure, black point, grade from the profile);
the **decision** does not.

**4. Nothing is dirty.** No mud, no dust build-up, no wear, no soot. A tank that crossed five hundred
metres of field should be carrying that field. This is the cheapest authenticity in vehicle art and
it is entirely absent.

**5. The silhouettes are good and the surfaces are empty.** That is exactly the profile of procedural
geometry: shape comes free, surface never does. The T-54's side profile is correct and carries no
history at all.

**And the ground is 60 % of every frame** — a flat green plane with tiling grass, cut by a hard river
edge. Whatever is done to the tanks, this is what the player looks at most.

---

# Feel — what can and cannot be judged from here

**Not judged: tuning.** The battle was driven by a script, not played. Whether the camera spring has
the right frequency, whether the FOV opens too fast, whether the recoil stroke is the right length —
unknown, and not guessed at here. Tuning is most of feel.

**The mechanisms are all present**, and that is not nothing: a 0.3 s fire buffer, a critically damped
follow spring, speed-driven FOV, barrel recoil, hull rock, camera kick, 343 m/s sound delay, a
per-shell flyby crack, tracers, and impact FX that distinguish penetration from ricochet from
non-penetration from water from stone. Someone knew the full list of things a tank game needs.

**One measured fact qualifies them.** An earlier draft claimed the battle was empty and the ricochet
never fired; both came from a harness that filtered out every zero-damage event. Corrected: **79
impacts and 9 of 14 tanks destroyed** across a full battle. There is plenty happening — the feel
mechanisms get used.

- **The bounce happens, and it is under-sold.** ~10 % of impacts ricochet (an earlier draft of this
  document claimed zero; that was instrument error). **The bounce is the most satisfying moment in an
  armoured game** — the second where the player knows they won the positioning duel: angled the hull,
  took the shell on the slope, heard the clang. The mechanic fires; what is thin is its
  *presentation*: there is no near-glance signature, no transition band, and the directed spark fan
  is the only thing distinguishing it at range.

---

# The strategic risk

> **The gap between the quality of the systems and the quality of the presentation is the single
> largest risk to this product.**

The engineering here is far above what a one-person project usually reaches: a deterministic
simulation, real convex armour volumes, honest collision, a versioned protocol with reconnect. And a
player will **never reach any of it**, because judgement happens in ten seconds and ten seconds shows
a flat green field, plastic tanks, an aerial where a 100 mm gun should be, and no consequence to
being hit.

Nobody discovers an honest armour model after bouncing off the first screenshot.

---

# Order of return

| # | work | why here |
|---|---|---|
| 1 | **Steel as a material** — roughness variance, edge wear, dirt in recesses, specular response | the PBR pipeline is built and unused; this is the difference between "plastic" and "tank" |
| 2 | **Decide the palette and the contrast** | the tools exist (exposure, black point, grade); the decision does not |
| 3 | **Dirt and wear** | the cheapest authenticity there is |
| 4 | **The ground** — material layers, no tiling, a real shoreline transition | 60 % of every frame |
| 5 | **The ricochet** | fixes a mechanic and restores the genre's best moment at the same time |
| 6 | **Proportions** — barrel, turret front | one proportion error spoils all the detail work around it |
| 7 | **UI: depth, material, type hierarchy, response to touch** | the content is right; the execution is a mockup |

Items 1–3 are perhaps two weeks and **will change the first impression more than anything else in
this program**.

