# Panzerkampfwagen VI Tiger

## Implemented Variant

- `Panzerkampfwagen VI Tiger Ausf. E`

This is the Tiger I, not the later Tiger II. The game spec represents a late Tiger I Ausf. E
with the Maybach HL230-class power output and the 8.8 cm KwK 36 L/56 gun.

## The variant, pinned

**Late Ausf. E — post-February 1944, Fgst.Nr. 250822 and up.** Every number and every feature
below is read against THAT tank, because "Tiger I" alone is not a specification: the vehicle
changed enough across production that mixing eras produces a tank that never existed. The
blueprint already committed to this era through `wheel_face: SteelDish`; this dossier makes the
commitment explicit so the rest of the vehicle can be derived from it rather than guessed.

What the pin decides (sources in *Late-production identity* below):

| Feature | Late Ausf. E (what we model) | Early (what we must NOT mix in) |
| --- | --- | --- |
| Road wheels | 16 per side, steel-rimmed, 2 per arm | 24 per side, rubber-tyred, 3 per arm |
| Cupola | cast periscope drum (from July 1943) | vision-slit drum |
| Feifel air cleaners | absent (factory-dropped Oct 1943) | present |
| S-mine dischargers | absent (dropped Oct–Nov 1943) | present |
| Headlights | single | twin on the glacis |
| Turret roof | 40 mm (from ~March 1944) | 25 mm |
| Turret stowage | simplified open bins on the ring | bowed "wide bin" |

## Reference anatomy (W1 2026-07-17 + research pass 2026-08-06)

Per the data-first protocol (`docs/vehicles/_template.md`): anchors verified against external
sources **before** any shape work, with conflicts recorded and resolved rather than silently
picked. `Locked` rows gate the bake; `Target` rows are documented values the model has not
reached yet and are reported as debt every run until their geometry PR flips them.

| Dimension | Value | Source | Confidence | Encoded as |
| --- | ---: | --- | --- | --- |
| Hull length | 6.316 m | Wikipedia + Panzerworld agree | high | `HullLength` (Locked) |
| Width (combat tracks) | 3.705 m | Panzerworld (German records); see conflict C1 | high | `HullWidth` (Locked) |
| Height (cupola apex) | 3.00 m | Wikipedia | high | `HeightToTurretRoof` (Locked) |
| Height (bare turret roof) | 2.885 m | German records; see conflict C2 | high | `HeightToTurretRoofBare` (Locked) |
| Overall with gun | 8.450 m | Wikipedia + Panzerworld agree | high | `OverallLengthWithGun` (Locked) |
| Road wheel | ⌀0.800 m | Tank Museum — **unchanged** across the steel-rim swap | high | `RoadWheelDiameter` (Locked) |
| Turret ring (in the clear) | 1836 mm | tiger1.info, factory-drawing derived | medium | `TurretRingDiameter` (Locked) |
| Turret ring bearing (OD) | 2100 mm | tiger1.info (traces to Jentz/Doyle) | high | context for the above |
| Combat track width | 725 mm | Wikipedia, tiger1.info, Tank Museum | high | `TrackWidth` (Locked) |
| Transport track width | 520 mm | same three | high | not modelled (other config) |
| Track pitch | 130 mm | panzerbasics, Alan Hamby; matches the Kgs 63/725/**130** name | high | implied by link count |
| Links per side | 96 | panzerbasics + Alan Hamby (independent) | high | `TrackLinkCountPerSide` (**Target**) |
| Ground clearance | 0.47 m | Wikipedia | high | `GroundClearance` (Locked) |
| Fire line (bore axis, gun level) | 2.195 m | Panzerworld **and** Alan Hamby | high | `FireLineHeight` (**Target**) |
| Road wheels per side | 16 | Tank Museum (with Fgst.Nr.), Alan Hamby ×2 | high | `RoadWheelCount` (**Target**) |
| Drive sprocket | ⌀914.4 mm, 20 teeth, front | Alan Hamby | medium | cage |
| Idler | ⌀685.8 mm | Alan Hamby | medium | cage |
| Return rollers | none | universal, photographically self-evident | high | cage |
| Hull width over sponsons | 3.56 m (Tank Museum: 3547 mm) | Wikipedia, Tank Museum | high | `half_width 1.78` |
| Gun bore length | 4928 mm (56 × 88) | Wikipedia 8.8 cm KwK 36 | high | context (not trunnion-relative) |

### Armour (late Ausf. E)

| Plate | Thickness | Angle from vertical | Confidence |
| --- | ---: | ---: | --- |
| Hull front, driver's plate | 100 mm | 9° | medium (a looser source rounds to 10°) |
| Hull front, nose plate | 100 mm | 25° | medium — **not yet modelled separately** |
| Hull side, upper | 80 mm | 0° | high |
| Hull side, lower (behind tracks) | 60 mm | 0° | high |
| Hull rear | 80 mm | 8–9° | medium-high (see conflict C4) |
| Hull roof / floor | 25 mm | — | high |
| Turret front | 100 mm | 8° | medium — genuine 5°/8°/10° spread (C3) |
| Turret side / rear | 80 mm | 0° | high |
| Turret roof | 40 mm (late; 25 mm early) | — | high |
| Walzenblende mantlet | 110–200 mm (curvature) | — | high |

### Conflicts, recorded with their resolution

- **C1 — width over combat tracks: 3.705 m vs 3.72 m.** The Tank Museum's own article and Alan
  Hamby both give 3.72 m; Panzerworld (German records) gives 3.705 m. **Resolved: keep 3.705 m.**
  **Reopened 2026-09-05 by the STT 1944 front view:** its own dimensions put 12 ft 3 in = 3.734 m
  over the widest point (the track guards) and 6 ft 10.625 in + 2 x 2 ft 4.5 in = 3.548 m over the
  tracks - so the 3.705-3.72 figures are the width over the GUARDS, and the blueprint's
  `track.outer_x 1.8525` puts the belts 8 cm too far out per side (K20).
  It is the more precisely and consistently repeated figure, and tiger1.info explicitly warns
  that measurements of surviving Tigers differ by several millimetres — 15 mm sits inside the
  build tolerance of a hand-assembled vehicle. Recorded, not chased into the model.
- **C2 — height: 2.885 m vs tiger1.info's "2625 mm total height". OPEN - leaning to tiger1.info since 2026-09-05: the STT side view traces the turret roof at ~2.55 and the cupola apex at 2.96 (K19).** tiger1.info's dedicated
  roof-height page quotes a German drawing at 2625 mm (2655 late), which would collide with our
  anchor by ~26 cm — exactly the variant-contamination class that bit the Tiger II (its "3.27 m"
  turned out to be transport tracks). It was NOT passed through silently: an independent
  decomposition (hull-only 1.78 m + turret-with-cupola 1.20 m ≈ 2.98 m apex, implying a roof at
  ≈2.865 m) supports keeping 2.885 m. **Resolution: anchor unchanged, conflict left OPEN** — the
  2625 mm figure's baseline could not be disambiguated because the source's own dimensioned
  diagram is an unreadable embedded image. This is the most important unclosed question on this
  vehicle.
- **C3 — turret front angle: 5° / 8° / 10°** across three sources. Our 8° is the midpoint of a
  genuine spread, not a consensus. Recorded so nobody later mistakes it for a settled number.
- **C4 — hull rear: 80 mm @ 8–9° vs one table's "60 mm @ 0°".** The outlier comes from a source
  with a non-standard hull/superstructure split; treated as a mislabelled row, not grounds for
  a change.

### Deliberately NOT anchored (and why)

Anchoring a number we cannot source is how a model gets graded against itself. Two are left out
on purpose:

- **Cupola external diameter.** The model carries ⌀0.78 m, whose only citation was the repo's own
  cast-cupola helper — a self-calibrated anchor. The 2026-08-06 research pass could not source
  the late periscope cupola's diameter from open material: tiger1.info's dimensioned drawings
  exist only as un-OCR'able images. The single web figure found (780 mm) comes from an
  AI-generated wiki whose own citation could not be checked, and its closeness to our unsourced
  0.78 is coincidence, **not corroboration**. Closing this needs Jentz/Doyle Vol. 1 or a photo
  camera-match against the sourced 850 mm roof cutout / 460 mm hatch hole.
- **Track gauge.** Nowhere directly documented; derivable only as 3.705 − 0.725 ≈ 2.98 m, which
  is exactly what the blueprint already builds. An anchor on it would pass by construction and
  measure nothing.

Also unsourced, and therefore uncaged: trunnion-to-muzzle barrel length, muzzle-brake external
dimensions, wheel-station spacing / ground-contact run, and cupola exposed height (our 0.115 m is
arithmetic between two other anchors, not an independent third fact).

### Open debts (measured every run by `dimension_gate`)

| Debt | Model | Documented | Δ | State |
| --- | ---: | ---: | ---: | --- |
| Fire line | 2.170 m | 2.195 m | −0.025 | open |
| Road wheels per side | 8 | 16 | −8 | open |
| ~~Track links per side~~ | 96 | 96 | 0 | **closed** — anchor Locked |

The wheel and link debts were the same defect wearing two faces: **the running gear was drawn at
roughly half the real tank's part count.** One authored axle produces one visible wheel
(`running_gear_place.rs`), where the late E carries two per arm; and `link_count: None` dropped
the belt onto the fleet's 0.22 m fallback spacing against a real 130 mm pitch.

The belt half is closed: the blueprint now authors `link_count: Some(96)` and the anchor is
Locked. **It cost nearly all the running-gear headroom** — the Tiger draws 38,736 of the 40,500
allowed near-tier gear triangles and 228 of 260 instances. Closing the wheel half on top of that
does not fit inside today's budget, so it has to arrive with the waste it pays for, not as a
budget raise (`GEAR_BUDGETS`; the one-look rule in `CLAUDE.md` — a budget rises per item WITH a
measurement).

### Previously closed (model-logic pass, 2026-07-26)

- Turret roof plane: was 2.72 m vs 2.885 m. The 16.5 cm deficit was invisible to the cage
  because the drum cupola's height was DERIVED as "whatever reaches the hitbox apex" — the
  roof error simply grew the drum (0.29 m against the 0.115 m the records imply) and the
  3.00 m apex lock still passed. Roof and drum are now authored separately
  (`roof_y 2.885`, `cupola_height Some(0.115)`) and locked separately.
- Width over tracks: was 3.68 m, and it was the HULL that measured 3.70 — the sponsons carried
  the beam anchor while the belts hid 1 cm inside them, which is also why the tank had no
  fender line at all. The two documented widths now sit on the two parts that own them:
  3.56 m over the sponsons, 3.705 m over the combat tracks, with the guards in between.
- Cupola diameter: was ⌀0.66 m against the ⌀0.78 m the cast-cupola helper documents as its
  own reference (audit #3's number). Now ⌀0.78, pulled slightly inboard so the honest drum
  still lands inside the bent rear wall. **Superseded 2026-08-06:** that ⌀0.78 cited our own
  helper, which makes it a self-calibrated number rather than a closed one — see *Deliberately
  NOT anchored* above. The geometry change was still an improvement; the citation was not.

## Blueprint Migration (2026-07)

The Tiger I is blueprint-born: `game_core::vehicle_blueprint::tiger_i` is the single shape
source for the hitbox, the mount frames, the armor facet slopes, the convex armor volumes, and
(via `vehicle_geometry::recipes::tiger_i`) the visible mesh. The legacy hand-authored body —
hitbox fractions plus a magic-number turret box — is gone, and with it a hitbox that was 7.2 m
long and only 2.92 m tall. The migrated body is the researched tank, a conscious gameplay
correction documented here and locked by `tiger_i_benchmark.rs`.

### Anchor dimensions (1:1)

| Anchor | Value | In the blueprint |
| --- | --- | --- |
| Hull length | 6.316 m | `half_len 3.16` |
| Width over combat tracks | 3.705 m | `track.outer_x 1.8525` |
| Width over sponsons | 3.56 m | `half_width 1.78` (the belts stand 7.25 cm proud per side) |
| Height to turret roof | 2.885 m | `roof_y 2.885` |
| Height to cupola top | 3.00 m | `roof_y 2.885` + `cupola_height 0.115` |
| Ground clearance | 0.47 m | `belly_y 0.47` |
| Road wheels | 8 × ⌀0.80 m interleaved | `wheel_count 8`, `overlap_inner_dx 0.22` |
| Track width | 725 mm | `inner_x 1.1275 .. outer_x 1.8525` |
| Contact run | ~3.6 m | `wheel_first_z/last_z ±1.80` |
| Overall with gun | 8.45 m | `muzzle_z 5.29` |
| Fire line | ~2.17 m | `trunnion_y 2.17` |

### The slab, honestly

The Tiger's character is the ABSENCE of slope, and the armor model now says exactly that:

- Driver's plate ~9° from vertical (`hull_front (9.0, 0.9)` — was a rounded 10° before).
- Sides genuinely vertical (`hull_side (0.0, 1.0)`), rear at its real 8°.
- The turret walls stand straight up; only the front plate leans its 8°
  (`turret_front (8.0, 0.92)` keeps the mantlet weakspot).

Because the vehicle carries armor VOLUMES (a `WeldedBox` plate prism for the turret instead of
the cast-dome sectors), the plate normals a shell meets are the flat plates you see: angling
the hull is what changes the presented angle, exactly like the real crews were taught. The
visible hull front/rear plates are authored on the same plane equations the volumes bake
(`tiger_slab_hull`), locked by `the_visible_drivers_plate_is_the_armor_glacis_plane`.

### Recognition features carried by the recipe

- Horseshoe turret: flat front plate, vertical bent side wall, faceted rear.
- Stowage bin closing the turret's REAR armor plane — the plate a shell into the bustle actually
  meets. **Era flag (2026-08-06):** this is authored as the bowed "Rommelkiste", which is the
  EARLY-production shape; late production simplified turret stowage to open bins mounted on the
  ring (~November 1943). "Rommelkiste" is also modelling-community shorthand associated with the
  Afrika Korps rather than a standard late-Tiger fitting. With the pin set to post-February 1944,
  the bin and the running gear currently come from different tanks.
- Drum (not domed) commander's cupola on the left rear roof, topping out the 3.0 m silhouette.
  The late E's is the CAST PERISCOPE type (from July 1943), not the vision-slit drum.
- Interleaved Schachtellaufwerk: odd wheels 0.22 m inboard, no return rollers — the top run rests
  on the wheels. **Currently one wheel per authored axle (8 per side) against the late E's 16 on
  8 arms** — see the debt table above. Note this is genuinely *interleaved*, a different mechanism
  from the Tiger II / Panther II *overlapped* gear the same code path serves.
- 8.8 cm KwK 36 with its double-baffle muzzle brake and no bore evacuator.
- Twin exhaust stacks standing proud on the rear plate; driver's visor and bow-MG ball on the
  near-vertical front.

### What deliberately changed for gameplay (re-recorded consciously)

- Hitbox: 7.20 × 3.90 × 2.92 m → 6.44 × 3.74 × 3.01 m (shorter, narrower, taller — the real
  proportions; the Tiger is now honestly harder to hide hull-down and easier to hit tall).
- Fire line raised 2.02 → 2.17 m; muzzle reach 5.85 → 5.29 m (the L/56 is not an L/71).
- Armor resolution moved from facet BANDS to blueprint volumes: track boxes act as spaced
  armor, the mantlet is a true patch on the front plate, roof shots resolve on a real roof
  plane.
- Contact footprint: eight real wheel stations (was a five-station hitbox estimate), so trench
  bridging and crest behavior follow the actual running gear.

## Authored visual parts (fleet slot W4 F5, #422)

The Tiger I is the first — and so far only — vehicle in the fleet to author a visual file:
`game_core/blueprints/tiger_i_ausf_e.visual.ron`. Only the GUN GROUP is authored; every other
part is `None` and the recipe keeps covering the rest — a partial file improves the look without
claiming cut-truth. What the file states:

- **Bore-honest KwK 36**: bore 88 mm (`bore_radius 0.044`) recessed in the muzzle face, tube
  radius 0.1 — the gameplay gun's, one truth.
- **Double-baffle muzzle brake** as real chambers with a waist (`revolve::muzzle_brake`,
  2 baffles); no bore evacuator, no canvas — the external mantlet has no window.
- **The Walzenblende** as a wide rolled casting spanning exactly the armour's mantlet patch band
  (trunnion-relative −0.23 … +0.07, widest radius 0.34 = the gameplay `mantlet_radius`) — the
  visible body IS the volume the armour quotes.

The golden bake hash was re-recorded Tiger-I-only (`vehicle_recipes/src/budgets.rs`), which is
itself the proof that the visual dispatch reads data, not vehicle identity.

## Reference drawings and the K0 outline (2026-09-05)

The School of Tank Technology's *Report on PzKw VI (Tiger) Model E, Part I* (January 1944) —
dimensioned technical drawings of a captured Tiger on wide tracks, UK Government work, public
domain — is on Commons as [Tiger_Side_View_Left.png](https://commons.wikimedia.org/wiki/File:Tiger_Side_View_Left.png),
[Tiger_Front_View.png](https://commons.wikimedia.org/wiki/File:Tiger_Front_View.png),
[Tiger_Top_View.png](https://commons.wikimedia.org/wiki/File:Tiger_Top_View.png),
[Tiger_Rear_View.png](https://commons.wikimedia.org/wiki/File:Tiger_Rear_View.png) and
[Tiger_Side_Cut_Right.png](https://commons.wikimedia.org/wiki/File:Tiger_Side_Cut_Right.png)
(local copies `output/refs/tiger_i_ausf_e/`, git-ignored, licences in `output/refs/SOURCES.md`).
**This is the highest-trust source this dossier has** — a measured vehicle, drawn to one scale,
with its own dimensions on the sheet: 27 ft 9 in (8.458 m) overall, 12 ft 3 in (3.734 m) over the
widest point, 12 ft 8 in (3.861 m) to the aerial, 5 ft 10 in (1.778 m) to the hull side top,
2 ft 4.5 in (0.724 m) track width, 6 ft 10.625 in (2.100 m) between the tracks, 1 ft 5 in
(0.432 m) clearance, 11 ft 10.125 in (3.61 m) contact run.

All three views are traced with `scripts/refs/trace_silhouette.py` into
`crates/vehicle/vehicle_forge/outlines/tiger_i_ausf_e.outline.ron` (side and front calibrated on
the sheet's own 8.458 / 3.734, the plan on 3.705; the three scales agree within 3 %). Against them
the sketch reads **side 0.831 / front 0.862 / plan 0.922** (floors 0.83 / 0.86 / 0.92, `Target`
until K3 builds the vehicle). What the overlay measured, in metres (drawing = grey + blue, bake =
grey + red on the 1 cm raster):

| Where | STT drawing | Bake | Register |
| --- | --- | --- | --- |
| Turret roof (side, the turret top behind the cupola) | ~2.55 | 2.885 (`roof_y`) | K19: the turret is ~30 cm too tall; C2 resolves toward tiger1.info's 2625 mm |
| Apex (cupola top) | 2.96 | 3.02 | consistent with 3.00 |
| Turret + bin at y 1.95 (side, z) | -1.55 ... +1.60 | -1.89 ... +1.20 | K19: the bake's turret sits 34 cm too far back and ends 40 cm short at the front |
| Turret width at y 1.95 / 2.5 (front) | 2.34 / 2.52 | 2.01 / 2.01 | K19: 30-50 cm too narrow |
| Widest point (front, y 1.2) | 3.63-3.73 (the track guards, 12 ft 3 in) | 3.57 | K20 |
| Tracks (front, y 0.2-0.9) | 3.56 (= 2.100 + 2 x 0.724) | 3.71 | K20: the blueprint's 3.705 is over the GUARDS, not the tracks |
| Upper hull above the guards (front, y 1.5-1.8) | 3.17-3.32 | 3.57 | K20: the upper hull box is ~25 cm too wide |
| Rear deck top (side, z -3.0 ... -2.2) | 1.96 | 2.10 | K20: 14 cm too high (the sheet's 5 ft 10 in = 1.778 hull side top) |
| Belt at y 0.30 (side, z) | wraps further at both ends | -2.90 ... +2.84 | the end wraps, as on the T-54 (K18) |

The plan's 5 cm strips along both sides are the drawing's convention (tracks drawn only at
their ends), not a defect.

## Data Sources And Gameplay Translation

The implemented values are practical gameplay specs grounded in public historical data.

Reference points:

- [Panzerworld Tiger Ausf. E](https://panzerworld.com/pz-kpfw-tiger-ausf-e): Maybach HL230 P45,
  700 net hp, 8.8 cm KwK L/56, ammunition and fuel data.
- [Tiger I overview](https://en.wikipedia.org/wiki/Tiger_I): 100 mm frontal hull and turret
  armor, dimensions (6.316 m hull, 3.705 m width, 3.00 m height), interleaved running gear.
- [OnWar Tiger I data](https://www.onwar.com/wwii/tanks/germany/ge067tiger1.html):
  Panzerkampfwagen VI Ausf. E designation, 57,000 kg combat weight, 88 mm KwK 36 L/56,
  38 km/h speed, armor table.
- Wikimedia Commons photo galleries for the horseshoe turret, turret stowage, cupola position,
  exhaust stacks, and wheel interleave used by the Forge reference pack ratio gates.

Added by the 2026-08-06 research pass:

- [Tank Museum — Tiger Wheels](https://tankmuseum.org/article/tiger-wheels): the 24→16 wheel
  change, the February 1944 / Fgst.Nr. 250822 changeover, and the 800 mm diameter holding
  constant across it. Also a photo calibration reference (the ⌀800 mm wheel in frame).
- [Tank Museum — Two Widths of Track](https://tankmuseum.org/article/two-widths-track): combat
  vs transport belts (725 / 520 mm), the front mudguard being wider than the hull, and the Berne
  loading-gauge reason the narrow track exists.
- [tiger1.info — Turret ring bearings](https://tiger1.info/EN/Turret-ring-bearings.html) and
  [mid/late bearing ring](https://tiger1.info/EN/Turret-bearing-mid-late.html): the 2100 mm race
  and the 1836 mm ring in the clear.
- [tiger1.info — Height of the roof](https://tiger1.info/EN/Roof-height.html): the 2625/2655 mm
  figure behind conflict C2. Its dimensioned diagram is an image and could not be read.
- [Alan Hamby — Tiger I Information Center: Suspension](https://www.alanhamby.com/suspension.shtml)
  and [Major Model Changes](http://www.alanhamby.com/changes.shtml): sprocket/idler diameters,
  link count, and the dated production-change timeline used by the variant pin.
- [panzerbasics — Driving the Tiger E](https://www.panzerbasics.com/panzer/01_basics/01_Tiger_E/driving.htm):
  track pitch and links per side, agreeing independently with Alan Hamby.

### Late-production identity, with dates

Reconstructed from Alan Hamby's changes timeline and the Tank Museum articles, cross-checked
where a second source existed:

- **Feifel air cleaners**: fitted Nov 1942, dropped at the factory **October 1943** (two sources
  agree). Our omission is correct for this pin — it is not a missing detail.
- **Cupola**: cast periscope type (7 periscopes, side-pivoting hatch, AA ring) replaces the
  vision-slit drum from **July 1943**.
- **Steel-rimmed wheels**: **February 1944, Fgst.Nr. 250822**; 24 → 16 per side, outer row
  deleted, diameter unchanged.
- **Headlights**: single light replaces the twin glacis pair from **August 1943**.
- **S-mine dischargers**: dropped **Oct–Nov 1943**; replaced from March 1944 (Fgst.Nr. 250991)
  by the internal Nahverteidigungswaffe, alongside the 40 mm turret roof.
- **Zimmerit**: factory-applied late Aug 1943 to 9 Sep 1944. Not modelled as a surface at all;
  recorded for completeness, not as a dimensional gap.

## Current Gameplay Spec

- Name: `Panzerkampfwagen VI Tiger Ausf. E`
- Mass: 57,000 kg
- Engine: 515 kW Maybach HL230-class
- Max forward speed: 10.56 m/s
- Gun: 8.8 cm KwK 36 L/56 (92 rounds)
- Armor model: 100 mm hull front @9°, 80 mm vertical hull side, 80 mm hull rear @8°,
  100 mm turret front @8° (mantlet weakspot 0.92), vertical turret sides/rear — thicknesses
  from the installed modules, geometry from the blueprint.

## Asset

Generated vehicle asset:

```text
assets/vehicles/tiger_i_ausf_e.vehicle.json
```

Regenerate it with:

```powershell
cargo run -p tools -- generate-vehicle --vehicle tiger-i-ausf-e --output assets/vehicles/tiger_i_ausf_e.vehicle.json
```
