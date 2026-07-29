# Model Idealny T-54 — program document

The T-54-3 obr. 1951 rebuilt to **zero deviations from the documented vehicle** — dimensions,
armor, and per-part mechanical construction — plus the workshop technology able to PROVE it.
When both registers below are empty and every `Target` anchor has flipped to `Locked`, this
document becomes history.

## STATUS (2026-07-28) — read this first if you are picking the work up

- Program approved. Wave W0 in flight: **PR-00 (this document + the t-54 dossier) is the first
  landing**. Nothing else has merged yet.
- Baseline at program start: 0 open PRs, `trial3` == master, LOD0 = 15,096 / 22,000 tris
  (the remembered "t54_hybrid budget debt" is stale — there is none).
- Foundation: five audits dated 2026-07-28 (geometry kernels; T-54 authoring stack;
  blueprint schema + armor; Forge workshop; gun/turret + running-gear construction) and a
  web-sourced dossier now recorded in `docs/vehicles/t-54.md` (Reference anatomy table +
  Part construction table from session S1b).
- Blender sessions S1 (dome master: station table banked in the plan; height 2.40 confirmed,
  2.218 ruled out) and S1b (part construction: 13-tooth sprocket rings, hinge-eye drive,
  the spider-web wheel correction) are done; S2/S3/S4 belong to W3.
- **Update 2026-07-29 — W0/W1/W2 built as a review stack** (PR-01 reference spec → PR-02 studio
  on the authoritative bake → PR-03 tile goldens + mirror fix → PR-04 OBJ export → PR-05 kernel
  contracts → PR-06 blueprint SSOT → PR-07 module honesty → PR-08 fleet gates → PR-09 armor
  taper [PROTOCOL v40] → PR-10 gun arc → PR-11 loft/sweep shape → PR-12a revolve winding →
  PR-12 spider-web wheel -> bot water slope -> PR-13 armour dome). Each is its own branch and
  draft PR; merge order is the stack order. **W2 closes at PR-13.**
- **Two findings worth carrying forward.** (1) The T-54's hydraulic dampers are internal (vane
  type on the balance-arm shaft) — the register's "no visible dampers" line was a mis-reading and
  is struck; nothing is owed on the hull side, and the `GearPart::Damper` drafted for PR-12 was
  deleted rather than shipped, because inventing an external cylinder is the exact failure this
  program exists to remove. (2) The revolve kernel wound every closed lathe profile
  inconsistently — 22 broken edges per ring, on every wheel, tyre, roller and drum in the fleet.
  It surfaced only because a NEW construction pushed a recorded debt ceiling past its limit,
  which is what the FLOOR/TARGET pattern is for. (3) The T-54's cheeks — the vehicle's entire
  armour argument — stood 0.34 m OUTSIDE their own armour volume, because the dome was swept as a
  circle of 1.12 m while the casting bulges to 1.37 at the shoulder. Shells went through them.
  Fixed in PR-13 by placing every sector plane on the casting's support function.
- **A defect found by deterministic divergence.** Changing the armour volume changed every
  subsequent shell resolution, so the seeded Bystra soak grew a different battle — and that
  battle drove a bot into the river. The bot brain's escape budget scaled braking by surface grip
  but not by SLOPE, and a channel is a hole, so every approach to one runs downhill. Fixed in its
  own PR before PR-13. The defect was always there; the two recorded seeds never produced that
  approach.

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
| M1 | Hull 6.00 m vs real 6.20–6.27 (barrel overhang optically long: 2.93 vs 2.58–2.73); belly clearance bakes 440 mm vs documented 425 (`belly_y 0.43`) — found by the PR-01 instrument | RON `half_len: 3.0`, `belly_y: 0.43` | W3/PR-14 |
| M2 | Dome too shallow: roof 2.27 vs 2.40 (fire line 1.78 is the anchor and stays) | RON `roof_y`; 3 "pancake" tests lock the deviation | W3/PR-15 |
| M3 | Turret side 90 mm vs 200/160→65 taper; turret roof 24 (formula) vs 30; mantlet ×1.18 rule vs authored | `catalog_soviet.rs:38`, `zone.rs:52-89` | W2/PR-09 |
| M4 | External mantlet ball ⌀640 vs narrow ~400 mm embrasure + internal mantlet + canvas | `t54_hybrid.rs:92-107` | W3/PR-17 |
| M5 | Cupola ⌀480 vs 624, exposed 131 mm, hatch 497×670; three copies in code, the rendered one untested | `t54_hybrid_turret.rs:82` | W3/PR-16 |
| M6 | Gun arc global −8/+20 vs real −5/+18 per vehicle | `aiming.rs:6-8` | W2/PR-10 |
| M7 | Symmetric fenders vs real asymmetry; missing SG-43 port and 2× MDSh | `t54_kit.rs:40-49` | W3/PR-19 |
| M8 | Wheel disc pattern generic 6 spokes vs the documented **spider-web** stamping (12 ribs, 12+12 lightening holes) — S1b corrected the earlier "5-arm starfish" assumption: starfish is a later/rebuild wheel; doubled swing arms (F5). **Struck 2026-07-29: "no visible dampers st. 1+5" was wrong** — the T-54's hydraulic dampers are VANE type acting on the inboard end of the balance-arm shaft, inside the hull; nothing of them is visible from outside, so the honest exterior is bare hull side and no geometry is owed | RON `wheel_spokes: 6`; `t54_chassis.rs:44-64`; dossier "Part construction"; ru.wikipedia / armor.kiev.ua suspension description | W2/PR-12 (done) + W3/PR-18 |
| M9 | Track 570 vs 580 mm; gauge 2690 vs 2640 (side-clearance arithmetic — open decision); **link pitch 142 vs 137 mm and wrap radius 0.32 vs a 0.286 m pitch circle**, which together derive 14 sprocket teeth per ring where the T-54 has 13 (14 is the modernised T-55 / Obj. 167 wheel) — the count follows the belt, so it lands with the track dimensions | RON track fields; `the_t54_tooth_count_records_the_belt_it_has_to_mesh_with` | W3/PR-18 |
| M10 | Deck travel lock authored with zero reference citation (real obr. 1951: likely none) | `t54_kit_lines.rs:47-72` | W3/PR-19 after S2 |
| M11 | `barrel_length_m` D-10T 5.0 vs D-10T2S 5.9 — same real tube L/53.5 ≈ 5.35; upgrade falsely stretches the silhouette | `catalog_soviet.rs:230,252` | W1/PR-07 |
| M12 | RON slope drift: T-54 rear 8° visual vs 5° armor; **Panther II turret 11° vs 20°** (fleet-wide defect class) | 2026-07-28 probe | W1/PR-06 |
| M13 | Workshop lies: anchors fit-to-model (hull 6.04±0.15 — a corrected model would FAIL), citations to a doc with no numbers, mirrored studio tiles, fast loop bakes the non-shipping mesh, wheel measurement is a tautology | Forge audit; `packs.rs:99-146` | W0/PR-01..03 |
| M14 | Hitbox +12% wider than the outermost armor volume (1.75 vs 1.63) — phantom ram/movement width | `all_vehicles.rs:79-91` | open decision |

## Register K — per-part construction deviations

| # | Defect | Evidence | PR |
| --- | --- | --- | --- |
| K1 | Muzzle reads bore-less: face is 76% flat steel (wall 49 mm vs real 10–15 — tube ~40% too fat), bore has no distinct material/AO (the legacy path had a dark funnel — lost), rim smoothed into a dimple; 20-gon faceting | `gun_parts.rs:15-34`; no muzzle band in `surface_bake.rs` | PR-25 |
| K2 | Mantlet is an OPEN-ended sleeve (no station at r=0); rims stand outside the barrel over ±31° crescents looking into the hollow; `OPEN_OR_CLOSED` contract allows it | `gun_parts.rs:62-83`; `quality.rs:214-219` | PR-17 |
| K3 | OMSh link: NO guide horn, no hinge knuckles/pins, no cleats; 4 of 7 detail boxes fully buried inside the backing slab (11,520 dead tris/tank); "pin bars" on the wrong face | `running_gear_geom.rs:17-91` | PR-22 |
| K4 | Road wheel: one body with a groove fakes twin tires; "dish" is a thinner flat coin; hub is a 19 cm peg; ZERO bolts (bolt circle exists only on the German dished path). **Partly closed by PR-12**: the T-54 face is now the spider-web frame (two bands + twelve webs + two rings of real holes, ray-measured), so what remains here is the twin tyres with their 53 mm axial gap, the dished disc, and the 10-bolt hub circle | `running_gear_wheels.rs:28-57` | PR-23 |
| K5 | Sprocket teeth stop 3.2 cm SHORT of the belt line (nothing meshes) yet intersect the backing on the wrap; carrier "rings" are solid coins; tooth is a flat wedge. Documented truth (S1b): **2 × 13 teeth**, ring ⌀682 × 120 mm on a ⌀572.4 pitch circle, 40 bolts, and the tooth bears on the link's **hinge-eye barrel**, not the horn | `running_gear_end_wheels.rs:65-136`; dossier "Part construction" | PR-24 |
| K6 | Idler: flat cylinder (same coin as the sprocket drum), no dish, no tension crank; open revolve hides the hollow | `running_gear_end_wheels.rs:25-59` | PR-24 |
| K7 | Swing arm: reach 0.26 / rise 0.13 HARDCODED fleet-wide (not blueprint), flat slab, no torsion-bar hub. **Duplication closed by PR-12** — the static hull boxes are deleted; the animated arm is the single source | `running_gear_arms.rs:18-22` | PR-27 |
| K8 | Cupola/3 hatches/headlight = bare `revolve::drum` pucks (8 real parts from one primitive); ZERO hinges/handles/latches in the whole repo; headlight "lens" faces UP | `parts.rs:12-23`; `t54_details.rs:15-72` | PR-26 (+PR-16) |
| K9 | Tow hooks are bricks; cables float without thimbles/clamps; rails have no welded feet; beam has no steel bands and is `TrackMetal`, not wood; splash board is a ⌀70 sausage, not an angle plate | `t54.rs:141-152`; `t54_kit_lines.rs` | PR-26 |
| K10 | DShK planted NEXT TO the loader hatch instead of on its ring; no cradle/arc/grips/sight; bore 2.2× oversize | `t54_dshk.rs:13-77` | PR-26 |
| K11 | Grille slats are flat unraked slabs (`louvre_slats` is DEAD code), "louvered" exhaust is a chamfered brick, deck panels have no bolts (`bolt_head` DEAD), no casting seam (`casting_seam` DEAD) | `solid/t54.rs:124-150`; `detail/` | PR-26 |
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
2. Track gauge 2690 vs 2640: at 580 mm the inboard edge (1.035) collides with the 1.05 tub —
   keep gauge or narrow the lower tub; resolved by S1/S2 cross-section drawings.
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
  `cargo run -p client --example garage_hangar_review` / `t54_studio` / `t54_views` /
  `closeup_probe`.
- Perf: `cargo run -p client --release --example perf_capture` + `combat_hot_path` bench after
  PR-09, PR-15, PR-18 and every W4 PR (min spec MX330 @ 60 FPS; LOD0 budget 22k; gear under the
  new PR-21 budget).
- Armor: `dimension_gate` + `t54_reference_spec` (PR-20) + deliberate replay re-pins.
