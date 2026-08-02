# Model Idealny T-54 — program document

The T-54-3 obr. 1951 rebuilt to **zero deviations from the documented vehicle** — dimensions,
armor, and per-part mechanical construction — plus the workshop technology able to PROVE it.
When both registers below are empty and every `Target` anchor has flipped to `Locked`, this
document becomes history.

## STATUS — **COMPLETE** (2026-07-29)

Both registers are empty and every anchor is **Locked**. By this document's own terms it is now
history; what follows is the record of how it got here.

- **Register M (dimensions): 14 rows, 14 closed or dispositioned.** M1 hull 6.235 (PR-14),
  M2 dome 2.40 (PR-15), M3 turret taper (PR-09), M4 embrasure + internal mantlet (PR-17; face rebuilt as the two-step WINDOW after the player's verdict — see below),
  M5 cupola ⌀624 (PR-16), M6 gun arc −5/+18 (PR-10), M7 fender asymmetry + SG-43 + MDSh (PR-19),
  M8 spider-web wheel (PR-12, dampers struck as a mis-reading), M9 belt 580 / gauge 2640 / pitch
  137 / 13 teeth (PR-18), M10 travel lock deleted (PR-19), M11 one D-10 tube (PR-07),
  M12 slope drift (PR-06), M13 the workshop's own lies (PR-01..03). **M14 (hitbox width) is the
  one open item and it is the USER's** — narrowing the box changes ramming, terrain contact and
  spotting together, so it is not a modelling call. It is measured every run and on the record.
- **Register K (construction): 13 rows, 13 closed.** K1 muzzle (PR-25), K2 mantlet shell (PR-17; cover rebuilt as panel+frame in the window pass),
  K3 OMSh link (PR-22), K4 road wheel (PR-12/23), K5/K6 sprocket + idler (PR-24), K7 swing arm
  (PR-27), K8 hatch mechanics + headlight (PR-26a/b), K9 hooks, cables, beam (PR-26b),
  K10 DShK (PR-26b), K11 louvres, bolts, casting seam (PR-26a), K12 gear budget + LOD (PR-21),
  K13 hollow tests (rewritten per PR).
- **Every `Target` anchor has flipped to `Locked`** — 14 of 14. The last one, GroundClearance, was
  never a geometry debt: the belly has been at the documented 0.425 since PR-14 and what was wrong
  was the INSTRUMENT (PR-20).
- **The anchor test**: `t54_reference_spec` walks dossier → blueprint → mount chain → mesh in one
  place, plus the form rules no dimension can express. A vehicle can pass every other test and
  still be wrong at the joins.

### The gun window (2026-07-29, the player's verdict on the first internal mount)

PR-17 moved the mantlet inside and the instruments all agreed — and the player looked at the
front and rejected it: *"Jarzmo wyszło źle, beznadziejne. To nie jest ambrazura, którą ma T-54.
Tutaj jarzmo praktycznie pozostało te same, zmniejszone i bardziej wchłonięte."* The diagnosis
held up: a round pocket, a round tapered boot and a visible round mantlet face are three round
signals stacked, and three round signals read as the old ball shrunk and swallowed — the
documented ~0.40 m *armour aperture* had been modelled as the thing the EYE sees, when on the
real vehicle it is invisible under fabric. What the eye reads on a T-54 obr. 1949/1951 is a
**wide rectangular window** cut between the cheeks with a **rectangular canvas panel** fastened
into it.

The rebuilt face, judged against Blender renders across six iterations before the golden was
blessed:

- **The casting is two-step.** A wide shallow window (~0.85 × 0.44 m rebate, `window_*` plateau
  bump) drops from the face to a shelf; the narrow deep aperture (~0.40 m, the documented
  number) drops through that shelf. Locked by the extended
  `the_turret_face_carries_a_narrow_gun_aperture`: shelf step 0.08–0.15 below the face band,
  window width 0.76–1.05, aperture ~0.40 at half depth — measured by walking the mesh, not by
  reading the bump function back.
- **The cover is a mattress, not a funnel.** Rounded-rectangle panel (0.80 × 0.37 m) with the
  hem rooted at the window shelf, a swell over the fastening strip, a rounded border and a FLAT
  capped front face the sleeve pierces; the first rebuild tapered the sweep to 0.30 scale and
  the front read as a pyramid of diagonals. Sleeve radii keep the ten-to-twelve-gon INRADIUS
  proud of the 0.098 m tube under the sag — at vertex-radius 0.100 the flats dipped under the
  steel and the barrel showed through as a torn zigzag.
- **The fastening frame is its own steel part** (`gun_mantlet_frame`, a closed loop swept in
  the window), locked to the panel and the window by `the_cover_frame_matches_the_window`.
- **Fold ridges are deliberately absent.** Tried twice — buried behind the face plane they
  rendered nothing (the K3 defect in miniature); surfaced they read as stamped chevrons, then
  as claw-mark slashes. Broad low undulations are beyond a tube sweep; clean canvas is the
  honest render.
- **The occluded pays for the visible.** The internal mantlet drops to 12 segments (only a
  breach remesh ever shows it); the panel, frame and window refinement land LOD0 at 21,988 of
  22,000.
- The visibility rule is inverted from the ball days:
  `the_visible_gun_mount_is_no_wider_than_its_canvas_cover` now asserts fabric ahead of the
  face out to the panel's half-diagonal (0.30–0.47) and **steel no wider than the tube** —
  anything wider is the ball creeping back.

### The no-compromise pass (2026-07-29, after S3/S4)

The Blender verification's findings were ordered fixed rather than recorded. What changed:

- **The flank waist is gone.** The S1 master's vertical 2.25 m band (ring seat → 2.00) is now
  carried BY THE STATIONS (half-width 1.125 across the band), and the separate cheek bumps are
  retired at zero: S1's exponent-2.8 superellipse fit measured the whole outline, so bolting
  cheek lobes onto a narrowed base DOUBLE-COUNTED the front mass — the documented width appeared
  only at the bump's own height and the casting waisted everywhere else (−112/−124 mm at
  1.58/2.00, measured). The dossier's own form rule was the tiebreaker: a full hemispherical
  dome, one continuous surface.
- **The interior audit found the interior authored against a remembered hull, and it is now
  authored against the blueprint.** Side liners sat at a literal ±1.12 — painted walls 90 mm
  OUTSIDE the armour, a relic of the pre-narrow-box hull; ammo racks and engine-bay fuel tanks
  hugged the old 1.05 tub (rounds 40 mm inside the 80 mm side plates, tank faces 10 mm past the
  hull); the final drives sat 18 cm behind the sprocket axle PR-18 moved; torsion bars ran to a
  literal 1.12; the suspension capsules stood 80 mm inboard of the documented gauge. All derived
  now, and locked by `t54_interior_containment` — which also NAMES the two legitimate wall
  penetrations (final drives to their sprockets, torsion bars to their arm hubs) and limits each
  to the hub it exists to reach.
- **The recorded compromises are withdrawn.** The DShK stands at fighting height on a real
  pintle with a cradle, trunnion pin, elevation arc, spade grips and sight (the K10 close below
  had been OVERCLAIMED — the ring and calibre landed in PR-26b, the controls only now), with its
  documented 1070 mm barrel and 1625 mm envelope. The smoke canisters are BDSh-5 at their
  documented ⌀450 × 650 (they were ⌀220 — sized by the collision box, not by a source). The
  unditching log is `MaterialRole::Timber` (open decision #6, resolved). Each protrusion this
  buys is CATALOGUED and enforced: `hitbox_fit::HITBOX_EXCEPTIONS` lists the AA gun, the
  canisters and the grab rails, requires everything else to fit, and separately requires each
  exception to actually protrude — a quietly shrunk-back compromise fails the gate.

### S3 re-run after the no-compromise pass (same day)

The section-diff, repeated on the fixed bake against the same S1 master:

| h | W model | W wzorzec | ΔW | L model | L wzorzec | ΔL |
|---:|---:|---:|---:|---:|---:|---:|
| 1.58 | 2.250 | 2.250 | **0 mm** | 2.355 | 2.363 | −8 mm |
| 1.68 | 2.250 | 2.250 | **0 mm** | 2.351 | 2.363 | −12 mm |
| 1.78 | 2.250 | 2.250 | **0 mm** | 2.351 | 2.363 | −12 mm |
| 1.88 | 2.250 | 2.250 | **0 mm** | 2.351 | 2.363 | −12 mm |
| 2.00 | 2.250 | 2.250 | **0 mm** | 2.362 | 2.363 | −1 mm |
| 2.12 | 2.124 | 2.124 | **0 mm** | 2.323 | 2.323 | 0 mm |
| 2.22 | 1.796 | 1.796 | **0 mm** | 2.105 | 2.105 | 0 mm |

**Width deviation: zero at every trusted station.** The residual −8..−12 mm of length in the
1.58–1.88 band is the gun aperture itself: the pocket recesses the casting's forward-most point
at those heights, and the master's outline was extracted un-recessed. Distance to ideal collapsed
from 124 mm to 12 mm, and the 12 is a documented feature, not a drift. The interior cutaway
render confirms the audit fixes visually: torsion bars end at the wall, the liner hugs the
armour, brass reads as brass inside the casting.

The no-compromise pass also cost three instruments their innocence, each caught reading the new
catalogued exceptions as vehicle structure: overall-length read the BDSh-5 drums as 0.36 m of
tank; the silhouette-apex anchor and the reference-spec roof check both read the raised DShK as
the casting. All three now measure armour structure, which is what their documented numbers
describe — stowage and weapons never counted in the sources either.

### S3/S4 — the Blender verification (2026-07-29, session close)

The master-reference loop ran against the FINAL stack head, in Blender over the live S1 scene
(`master_dome` + calibrated drawings), on a fresh `tools export-mesh` bake (13 objects, 60,692
tris). Three instruments measured the same vehicle — the in-repo dimension gate, a plain-Python
OBJ parse, and Blender itself — and their numbers agree to the millimetre. Blender independently
confirms **0 non-manifold edges** on Hull, Turret and Gun.

Section diff against the S1 master (widths/lengths of the casting, registration-free):

| h | W model | W wzorzec | ΔW | L model | L wzorzec | ΔL |
|---:|---:|---:|---:|---:|---:|---:|
| 1.58 | 2.138 | 2.250 | **−112 mm** | 2.363 | 2.363 | −0 mm |
| 1.68 | 2.207 | 2.250 | −43 mm | 2.366 | 2.363 | +3 mm |
| 1.78 | 2.245 | 2.250 | −5 mm | 2.369 | 2.363 | +6 mm |
| 1.88 | 2.207 | 2.250 | −43 mm | 2.366 | 2.363 | +3 mm |
| 2.00 | 2.126 | 2.250 | **−124 mm** | 2.366 | 2.363 | +3 mm |
| 2.12 | 2.136 | 2.124 | +12 mm | 2.324 | 2.323 | +1 mm |
| 2.22 | 1.799 | 1.796 | +3 mm | 2.105 | 2.105 | +0 mm |
| 2.30* | 1.441 | 1.608 | −167 mm | 1.680 | 1.886 | −206 mm |
| 2.40* | 0.840 | 0.420 | +420 mm | 0.960 | 0.493 | +467 mm |

\* rows above 2.22 compare against the master's CLOSURE band, which S1 itself marks untrusted
(roof furniture in the extraction; crown from geometric closure). Our authored flat roof is a
deliberate divergence from that closure, not from the vehicle.

**Distance to ideal, trusted band (1.58–2.22): length within 6 mm everywhere; width within
12 mm at the cheeks and the neck.** The one remaining SHAPE deviation: the casting waists in up
to 112/124 mm of total width at the ring seat and at 2.00, where the S1 sheet claims a vertical
flank at full 2.25 — and PR-15 reads that 1.125 as the silhouette including the cheek swell. The
sheet's own ±4–7% inconsistency cannot arbitrate; recorded here as the open shape question, with
the front/rear registration dispute (ours ring-datum front-heavy, S1's widest-cut rear-heavy)
already documented in the PR-15 authoring comments. Renders: overlay + 7 review angles in the
verification job's tmp (`v_*.png`); reproduce with `tools export-mesh` + `verify_phase1/2.py`.

Visual review of the renders confirms every dossier form rule on the shipped mesh: the narrow
aperture with the canvas cover, no ball (this session's boot was later rebuilt into the
window-and-panel face — see the gun window section), the bore reading as a hole, spider-web wheels with twin
tyres and horns riding between them, sprocket teeth meshing the links, rear drive / front idler
with its crank, no return rollers (the top run lies on the wheels), the asymmetric fender line
(right: two X-lid tanks; left: three bins + exhaust), the headlight facing forward with a glass
lens and guard, thimbled and clamped cables, the banded log over two MDSh canisters, raked deck
louvres, bolted panels, the casting seam, and the DShK ring concentric with the loader's hatch.
Known, recorded compromises visible as intended: the DShK sits low on its ring (footprint
doctrine) and the MDSh drums are constraint-sized.

### What this programme kept finding

One defect class dominated, and it is worth naming because the next vehicle will have it too:
**the instrument and the thing it measured were the same mistake.** Hull length caught the stowed
beam; bare-roof height caught the hatch lid; cupola diameter caught the roof plate; ground
clearance looked in a window narrower than the floor it was measuring; the phantom-width lint read
the hitbox against a model that shared its error; a shading test passed because a cavity band
overshot onto the nose plate; three tests REQUIRED the wrong shape outright. Every one of them
surfaced only when the geometry became correct — which is the argument for fixing the vehicle and
the tape measure in the same commit.

The second class: **a number written down twice is a number about to disagree with itself.** The
tub width, the wheel stations, the fender centre, the top-run anchor with `0.96 − 0.32` hidden
inside a bare `0.66`, the DShK inheriting the tank gun's calibre — each was a copy, and each broke
the moment its source moved.

## The decisions this program is built on

| Decision | Choice (2026-07-28) |
| --- | --- |
| Scope | Full program, 5 waves (W0 workshop truth → W1 hardening → W2 tech → W3 dimensions ∥ W4 construction), ~28 PRs |
| Real combat values (turret side 160→65 taper, gun arc −5/+18) | **Enter the game immediately** with their tech PRs — honesty over balance; balance is tuned by roster/MM, never by faking armor |
| Interior / museum detail | Separate program (Honest Steel). Component mask is already u32 (v27) — NOT protocol-blocked |
| Dispersion (2.9 mrad card vs vision 0.1–0.3) | Out of scope; weapon-card semantics to be documented separately |
| Blender | Digital clay + master reference + inspection instrument, **never an asset source** — numbers flow Blender→RON/Rust, meshes never do |

## Blender collaboration (standing workflow)

Blender 5.2 + blender-mcp addon (port 9876), MCP server `blender` (user scope,
`uvx --with "mcp[cli]<2" blender-mcp`). Sessions: **S1** master dome + part references (station
superellipse fits → `t54_hybrid_turret.rs` table; OMSh link / road wheel / muzzle contour /
sprocket engagement dimensions from photos), **S2** camera-match to resolve height 2.40 vs 2.218
and audit fittings (travel lock has no citation — removal candidate), **S3** per-PR inspection
loop (`tools export-mesh` → overlay on master → cross-section diff = numeric distance-to-ideal),
**S4** final proof-shot. The game stays 100% blueprint-born (no-clones, procedural-only).

## Register M — dimensional deviations

| # | Deviation | Evidence | Wave |
| --- | --- | --- | --- |
| M1 | ~~Hull 6.00 m vs real 6.20–6.27~~ **CLOSED (PR-14)**: hull built at the working 6.235, belly at the documented 0.425, and the muzzle at the documented 2.73 m past the bow (it was an absolute, so growing the hull walked the Locked 9.00 m overall length past its tolerance). Every fitting on both ends is a setback from the hull's own ends now, not a literal | RON `half_len: 3.1175`, `belly_y: 0.425`, `muzzle_z: 5.8475`; `dimension_gate` HullLength **Locked** | W3/PR-14 DONE |
| M2 | ~~Dome roof 2.27 vs 2.40~~ **CLOSED (PR-15)**: the casting is built at the documented 2.40 m roof from the S1 station table, the cupola's 131 mm of exposure is authored instead of being whatever fitted under the hitbox apex, and S1's other finding lands with it — the widest cut is 43% from the front, so the turret reaches 1.016 m forward of the ring and 1.347 aft where the model had 1.05 both ways | RON `roof_y: 2.40`, `cupola_height: Some(0.131)`, `plan_half_length: 1.35`; `dimension_gate` HeightToTurretRoof(Bare) **Locked** | W3/PR-15 DONE |
| M3 | Turret side 90 mm vs 200/160→65 taper; turret roof 24 (formula) vs 30; mantlet ×1.18 rule vs authored | `catalog_soviet.rs:38`, `zone.rs:52-89` | W2/PR-09 |
| M4 | ~~External mantlet ball ⌀640 vs narrow embrasure~~ **CLOSED (PR-17)**: the aperture is cut through the casting as a plateau bump and measures **0.420 × 0.380 m** at half depth (dossier ~0.40); the mantlet is a closed cast body behind it, wider than the hole it sits in; a canvas boot seals the hole to the tube. Three tests that REQUIRED the ball are inverted | `t54_hybrid_turret.rs` embrasure block; `t54_gun_cover.rs`; `the_turret_face_carries_a_narrow_gun_aperture` | W3/PR-17 DONE |
| M5 | Cupola ⌀480 vs 624, exposed 131 mm, hatch 497×670; three copies in code, the rendered one untested | `t54_hybrid_turret.rs:82` | W3/PR-16 |
| M6 | Gun arc global −8/+20 vs real −5/+18 per vehicle | `aiming.rs:6-8` | W2/PR-10 |
| M7 | ~~Symmetric fenders; missing SG-43 port and 2× MDSh~~ **CLOSED (PR-19)**: right shelf two flat fuel tanks (was three — a later fit) with stowage fore and aft, left shelf three bins; the fixed course SG-43's cast boss in the glacis right of centre, opposite the driver, rooted through the plate; two MDSh canisters on the rear plate below the beam | `t54_fenders_are_asymmetric_the_way_the_references_are`, `t54_carries_a_course_machine_gun_port_right_of_centre`, `t54_carries_two_smoke_canisters_on_the_rear_plate` | W3/PR-19 DONE |
| M8 | Wheel disc pattern generic 6 spokes vs the documented **spider-web** stamping (12 ribs, 12+12 lightening holes) — S1b corrected the earlier "5-arm starfish" assumption: starfish is a later/rebuild wheel; doubled swing arms (F5). **Struck 2026-07-29: "no visible dampers st. 1+5" was wrong** — the T-54's hydraulic dampers are VANE type acting on the inboard end of the balance-arm shaft, inside the hull; nothing of them is visible from outside, so the honest exterior is bare hull side and no geometry is owed | RON `wheel_spokes: 6`; `t54_chassis.rs:44-64`; dossier "Part construction"; ru.wikipedia / armor.kiev.ua suspension description | W2/PR-12 (done) + W3/PR-18 |
| M9 | ~~Track 570/gauge 2690/pitch 142/14 teeth~~ **CLOSED (PR-18)**: belt 0.580 on the documented 2.640 gauge, base 3.840, and `end_z` solved (2.66 → 2.590) so 90 links come out at the documented 0.1370 m pitch. The tooth count fell to 13 on its own — it is the number of pitches around the wrap, and the wrap is now the sprocket's documented ⌀572.4 pitch circle. The tub narrowed to the 2.060 m the gauge leaves it | RON track fields; `dimension_gate` TrackWidth + TrackGauge **Locked** | W3/PR-18 DONE |
| M10 | ~~Deck travel lock authored with zero citation~~ **CLOSED (PR-19)**: deleted. It was the only part on this vehicle drawn without a citation of any kind, and the dossier rules it out. The test asserts its ABSENCE — no part key may contain `travel_lock` — because deleting it is easy and keeping it deleted is the point | `t54_has_no_external_gun_travel_lock` | W3/PR-19 DONE |
| M11 | `barrel_length_m` D-10T 5.0 vs D-10T2S 5.9 — same real tube L/53.5 ≈ 5.35; upgrade falsely stretches the silhouette | `catalog_soviet.rs:230,252` | W1/PR-07 |
| M12 | RON slope drift: T-54 rear 8° visual vs 5° armor; **Panther II turret 11° vs 20°** (fleet-wide defect class) | 2026-07-28 probe | W1/PR-06 |
| M13 | Workshop lies: anchors fit-to-model (hull 6.04±0.15 — a corrected model would FAIL), citations to a doc with no numbers, mirrored studio tiles, fast loop bakes the non-shipping mesh, wheel measurement is a tautology | Forge audit; `packs.rs:99-146` | W0/PR-01..03 |
| M14 | Hitbox wider than the outermost armor volume (1.75 vs 1.61) — phantom ram/movement width. **The number moved to 0.141 m in PR-18 and nothing got worse**: the hitbox did not move, the VEHICLE did, onto its documented 2.640 m gauge. The phantom width was always this; the lint had been reading it against a model that shared part of the error | `all_vehicles.rs:79-91`; `the_hitbox_does_not_grow_further_past_the_visible_vehicle` | open decision — **user owns it** (narrowing the box changes ramming, terrain contact and spotting together) |

## Register K — per-part construction deviations

| # | Defect | Evidence | PR |
| --- | --- | --- | --- |
| K1 | Muzzle reads bore-less: face is 76% flat steel (wall 49 mm vs real 10–15 — tube ~40% too fat), bore has no distinct material/AO (the legacy path had a dark funnel — lost), rim smoothed into a dimple; 20-gon faceting | `gun_parts.rs:15-34`; no muzzle band in `surface_bake.rs` | PR-25 |
| K2 | ~~Mantlet is an OPEN-ended sleeve~~ **CLOSED (PR-17)**: the profile reaches r=0 at both ends, so the mesh has zero boundary edges — asserted directly rather than left to `OPEN_OR_CLOSED`, which is the contract that cannot see this | `the_mantlet_is_a_closed_body_not_an_open_sleeve`, `the_mantlet_profile_closes_at_both_ends` | PR-17 DONE |
| K3 | OMSh link: NO guide horn, no hinge knuckles/pins, no cleats; 4 of 7 detail boxes fully buried inside the backing slab (11,520 dead tris/tank); "pin bars" on the wrong face | `running_gear_geom.rs:17-91` | PR-22 |
| K4 | Road wheel: one body with a groove fakes twin tires; "dish" is a thinner flat coin; hub is a 19 cm peg; ZERO bolts (bolt circle exists only on the German dished path). **Partly closed by PR-12**: the T-54 face is now the spider-web frame (two bands + twelve webs + two rings of real holes, ray-measured), so what remains here is the twin tyres with their 53 mm axial gap, the dished disc, and the 10-bolt hub circle | `running_gear_wheels.rs:28-57` | PR-23 |
| K5 | Sprocket teeth stop 3.2 cm SHORT of the belt line (nothing meshes) yet intersect the backing on the wrap; carrier "rings" are solid coins; tooth is a flat wedge. Documented truth (S1b): **2 × 13 teeth**, ring ⌀682 × 120 mm on a ⌀572.4 pitch circle, 40 bolts, and the tooth bears on the link's **hinge-eye barrel**, not the horn | `running_gear_end_wheels.rs:65-136`; dossier "Part construction" | PR-24 |
| K6 | Idler: flat cylinder (same coin as the sprocket drum), no dish, no tension crank; open revolve hides the hollow | `running_gear_end_wheels.rs:25-59` | PR-24 |
| K7 | Swing arm: reach 0.26 / rise 0.13 HARDCODED fleet-wide (not blueprint), flat slab, no torsion-bar hub. **Duplication closed by PR-12** — the static hull boxes are deleted; the animated arm is the single source | `running_gear_arms.rs:18-22` | PR-27 |
| K8 | ~~Hatches are bare drums; ZERO hinges/handles/latches in the whole repo; headlight lens faces UP~~ **CLOSED (PR-26a + PR-26b)**: the headlight points FORWARD with a glass lens, bracket and guard; the detail kernel gained generic `hinge` / `grab_handle` / `coaming` generators (the fleet had none — `grep hinge` returned nothing) and all three hatches carry a collar, a hinge behind the lid and a handle on it | `the_headlight_faces_forward_and_shows_glass`, `every_hatch_carries_a_coaming_a_hinge_and_a_handle` | PR-26a/b DONE |
| K9 | ~~Tow hooks are 240x220x200 bricks; cables levitate on a standoff; beam unbanded~~ **CLOSED (PR-26b)**: each hook is a bracket, a curved throat a shackle drops into and a catch across its mouth; each cable is thimbled at both ends and clamped along its run; the log is strapped by two steel bands. The wood MATERIAL stays open decision #6 — it belongs with the material families, not smuggled in here | `the_tow_hooks_have_a_throat_and_a_catch`, `the_tow_cables_are_thimbled_and_clamped`, `the_unditching_beam_is_banded_to_its_brackets` | PR-26b DONE |
| K10 | ~~DShK on a pedestal beside the hatch; bore 2.2x too big~~ **CLOSED (PR-26b ring/calibre + no-compromise pass controls)** — the first close was overclaimed: PR-26b delivered the ring and the calibre while the cradle, arc, grips and sight were still absent, and the gun lay flat to fit the box. It turns on the LOADER'S HATCH RING — he stands in his own hatch and swings the gun round himself; on a pedestal beside it, it is a gun he cannot reach from inside. And the bore was worse than recorded: inheriting `..v.gun` gave a 12.7 mm gun the D-10T's 100 mm, a bore WIDER THAN ITS OWN TUBE, so the muzzle turned itself inside out | `the_dshk_turns_on_the_loaders_hatch_ring`, `the_dshk_has_its_own_calibre` | PR-26b DONE |
| K11 | ~~Deck boards not raked; `louvre_slats`, `bolt_head`, `casting_seam` DEAD~~ **CLOSED (PR-26a)**: the deck louvres lean (the rake went into the KERNEL too — the primitive named for real louvres made square boxes), the panels carry the bolts that hold them, and the turret has the line where its mould parted. `tube_along` could not make a closed seam at all (`closed: false`, both ends capped → 7 non-manifold edges), so `casting_seam_loop` was added: part of WHY they were dead is that they could not be used for what they were written for | `t54_exterior_mechanics.rs`; `a_raked_louvre_leans_across_its_opening` | PR-26a DONE |
| K12 | Running gear has NO LOD and NO budget: ~35k tris outside every limit, 204 instances/tank, whole-vehicle culling only; blueprint `segments` knob dead (`.max()` floors) | `frame_scene.rs:95-119`; `budgets.rs:16-25` | PR-21 |
| K13 | Lying tests: `..._has_omsh_plate_horns_and_pin_cues` is satisfied by the backing slab; `..._reads_as_a_double_wheel_pair` passes on a single tire | `tests/running_gear.rs:338,459` | each W4 PR |

## Wave plan (~28 PRs; 1 branch = 1 PR from master; every PR lands with a locking test)

- **W0 Workshop truth (PR-00..04):** this document + dossier (PR-00); `ReferenceSpec` with new
  `DimensionKind`s, `Locked`/`Target` anchor status, mesh-slice measurements, corrected T-54
  numbers, docs-provenance test (PR-01); authoritative fast loop for the T-54 hybrid + live
  TrackShape (PR-02); mirror fix + full golden re-bless + unconditional hash-based gate +
  chirality lock + hybrid production golden (PR-03); `tools export-mesh` OBJ exporter (PR-04).
- **W1 Hardening (PR-05..08, parallel):** kernel contracts (revolve min-radius, signed-volume
  outwardness, hard-edges weld fix, chamfer-zero, cast_loft bump validation) (PR-05); blueprint
  SSOT (fleet slope lint incl. Panther II reconciliation, `turret_loft` in the SSOT test, glacis
  fold rederivation, legacy arms → `unreachable!`, metaball cleanup) (PR-06); module honesty nits
  (both D-10 barrels 5.35 m, forge-report on the authoritative bake, LOD path unification,
  sprocket material, stale comments) (PR-07); hybrid under the fleet mesh-quality gate + hitbox
  tightness decision (PR-08).
- **W2 Tech extensions (PR-09..13):** armor thickness-per-plane + turret side taper + HullDeck
  zone split + authored mantlet/roof [PROTOCOL bump, replay re-pin] (PR-09); per-vehicle gun arc
  (PR-10); cast_loft sharp bump + sweep per-station taper (PR-11); consistent lathe winding in
  the revolve kernel — one winding, one orientation vote, fleet gear winding debt deleted
  (PR-12a); `WheelFace::SpiderWeb` + T-54 on the stamped disc + F5 dedup (PR-12); the armour dome
  becomes the CASTING (support function of the loft, not a swept circle) + the T-54 mesh↔volume
  lock in both directions (PR-13, closes W2).
- **W3 Dimensions (PR-14..19, sequential, numbers from S1/S2):** hull 6.2X (PR-14); dome 2.40
  (PR-15); cupola ⌀624 (PR-16); embrasure + internal mantlet + canvas + closed shell (PR-17);
  track 580 + gauge decision + link pitch (PR-18; the dampers once planned here are struck — see
  M8); fender asymmetry + SG-43 + MDSh + travel-lock removal (PR-19).
- **W4 Mechanical logic (PR-21..27, parallel with W3, PR-21 first):** gear budget + LOD enabler
  (PR-21 — DONE: `GEAR_BUDGETS` per tier + `GearDetail::Far` saving 47-61% past 60 m, and
  the blueprint's dead `segments` knob revived); OMSh link anatomy (PR-22); road wheel construction (PR-23); sprocket/idler engagement
  (PR-24); muzzle truth + cradle (PR-25); exterior mechanics — hinges, handles, headlight
  forward, hooks, thimbles, louvres, bolts, casting seam, DShK on the hatch ring (PR-26);
  blueprint-driven swing arm (PR-27).
- **Final: PR-20** — every anchor `Locked`, the `t54_reference_spec` cross-representation anchor
  test (Tiger-I pattern), S4 proof-shot, protocol changelog entry, STATUS flipped to COMPLETE.

## Open decisions

1. M14 hitbox 1.75 vs 1.63 (gameplay: ram/movement) — owner: user.
2. ~~Track gauge 2690 vs 2640~~ **RESOLVED (PR-18), and by the dossier rather than by a
   drawing**: gauge 2.640 + track 0.580 = 3.220 over the tracks, and the fender shelf already
   sat where 25 mm of overhang each side makes the documented 3.270 over the fenders. The gauge
   was right and the TUB was wrong — it is 1.03, the half of the 2.060 m the gauge leaves.
3. Height 2400 vs 2218 — S2 camera-match; fallback 2400.
4. Panther II turret 11° vs 20° — that vehicle's dossier decides in PR-06.
5. Protocol bump strategy for purely-visual W3 PRs — single collective entry at PR-20 (PR-09
   bumps regardless).
6. Wood material for the beam (new `MaterialRole` + texture family layer) vs steel-banded
   compromise — decided at PR-26.
7. Inter-link daylight vs anti-strobe overlap — PR-22 ships a sculpted knuckle-line underside;
   full gaps only if S3 shows it insufficient.

## Risks

1. Anatomy tests currently lock the deviations (pancake ratios, mantlet caps, roof-furniture
   bands) — re-bless only onto dossier/S1 numbers, never "to pass".
2. Hull cascade: the absolute-z band (hooks/headlight/beam/cables/stowage/AO weld) moves by
   hand — PR-06's fold rederivation first; `closeup_probe` after.
3. Real combat values shift 7v7 balance (stronger turret sides, weaker hull-down) — watch bots
   after W2; compensate via roster, never via armor.
4. Golden/replay churn — bless in deliberate commits only; hash goldens with a diff dir from
   PR-03.
5. Gear perf on MX330: 204 instances × new detail — PR-21 (budget+LOD) is the wave enabler;
   PR-22's −11.5k buried tris pays for horns and pins; `perf_capture` after every W4 PR; a frame
   regression stops the wave (one-look policy).

## Verification

- Merge gate: `./scripts/verify.ps1` (fmt → clippy `-D warnings` → workspace tests; stage the
  three on cold runs; in a worktree run `cargo fmt` per crate — os error 206 pitfall).
- Shape PRs: `cargo run -p tools -- studio --vehicle t54-1951` (authoritative after PR-02;
  `--blueprint-file` loop ≈1.8 s), `tools export-mesh` → Blender S3 section diff, human review:
  `cargo run -p client --example probe -- garage_hangar_review` / `t54_studio` / `t54_views` /
  `closeup_probe`.
- Perf: `cargo run -p client --release --example probe -- perf_capture` + `combat_hot_path` bench after
  PR-09, PR-15, PR-18 and every W4 PR (min spec MX330 @ 60 FPS; LOD0 budget 22k; gear under the
  new PR-21 budget).
- Armor: `dimension_gate` + `t54_reference_spec` (PR-20) + deliberate replay re-pins.
