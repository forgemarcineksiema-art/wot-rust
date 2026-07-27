# KV-1 model 1942 (cast turret) — reference dossier

> Dossier per `_template.md` (Genialna Flota, PR-X.1). **Data first, model second**: the anchor
> numbers below become `DimensionTarget`/`RatioTarget` entries and benchmark-cage asserts *before*
> the blueprint RON is authored. Worked examples: `t-54.md` (richest), `is-3.md`.

## Identity

The **KV-1 model 1942 with the reinforced cast turret**, as built at the Chelyabinsk Kirov Plant
(ChKZ) between January and August 1942 — the heaviest, best-protected and slowest form the KV-1
reached before the line turned to the lightened KV-1S. Its distinguishing feature is the **fully
cast turret** with selective 110–120 mm reinforcement around the earlier casting's weak areas and
an armoured collar around the rear machine-gun mount, carrying the long-barrelled **76.2 mm
ZiS-5**.

This model is deliberately **not**:

- the **model 1939/1940** — welded slab turret, short 76 mm L-11 or F-32, ~45 t. A different
  turret construction entirely; it would be a different vehicle, not a variant of this one.
- the **model 1941 *s ekranami*** — bolted/welded appliqué screens over a welded turret. A spaced
  armour story worth telling one day, but it is not this tank.
- the **KV-1S** — the 1942–43 "skorostnoy" redesign that threw armour away to recover speed, added
  a commander's cupola and a planetary transmission. The KV-1S is the *answer* to this vehicle's
  central flaw and must never be conflated with it.

**Era placement — a deliberate choice that needs stating.** This is a 1942 vehicle and the game's
`Era::EarlyWar` bracket is labelled 1939-42, so a reader will expect it there. It is filed in
**`Era::LateWar` (Era II, 1943-45)** instead, for two reasons. First, mechanically: Era I is empty
and `vehicle_kind.rs::every_populated_era_fields_at_least_two_playable_vehicles` forbids a
one-vehicle era, because that would put clone armies on both teams. Second, historically: cast-turret
KV-1 production ran to August 1942 and the survivors fought on well into 1943, including at Kursk,
against the very Tigers this bracket is built around. Era I stays reserved for a future wave that
opens it with at least two vehicles. **This is a roster decision, not an error.**

## Reference anatomy (anchor numbers)

Confidence: high = multiple independent sources agree; medium = single good source or derived;
low = estimated. **Only high-confidence rows become tight `DimensionTarget` anchors in PR 3**;
medium rows enter with a widened tolerance and a TODO, per the masterplan.

| Dimension | Value | Source | Confidence | Encoded as |
| --- | ---: | --- | --- | --- |
| Hull length | 6.75 m | [1] | high | `DimensionKind::HullLength`, `hull.half_len` |
| Width over tracks | 3.32 m | [1] | high | `DimensionKind::HullWidth`, `track.outer_x` |
| Overall height | 2.71 m | [1] | high | `DimensionKind::HeightToTurretRoof` |
| — of which turret roof | 2.62 m | derived | medium | `turret.roof_y` (the periscope on it reaches 2.71) |
| Overall length, gun forward | ≈ hull length (modelled 7.20 m — see deviations) | [1] | high | `DimensionKind::OverallLengthWithGun` |
| Combat weight | 47.0 t (range 45–52 t across variants) | [1][2] | medium | Σ module `mass_kg` |
| Crew | 5 | [1] | high | — |
| Hull armour, front | 90 mm | [1] | high | `HullChassis::front_mm` |
| Hull armour, side | 75 mm, vertical | [1] | high | `HullChassis::side_mm` |
| Hull armour, rear | 70 mm | [1] | high | `HullChassis::rear_mm` |
| Turret, cast | ~100 mm, selectively 110–120 mm at the weak areas | [2] | medium | `TurretModule::front_mm/side_mm` |
| Main gun | 76.2 mm ZiS-5, L/41.5 | [1][2] | high | `GunSpec::name`, `barrel_length_m` |
| Secondary | 3–4 × DT: bow, coaxial, **rear turret ball with an armoured collar** | [1][2] | high | recipe fittings |
| Engine | V-2K V12 diesel, 600 hp (441 kW metric) | [1] | high | `EngineModule::power_kw` |
| Suspension | Torsion bar, individual shock absorption per station | [1][3] | high | `SuspensionKind::TorsionBar` |
| Road wheels | 6 per side | [3] | high | `track.wheel_count` |
| Return rollers | 3 per side | [3] | high | `track.return_rollers` |
| Drive / idler | Drive sprocket REAR, idler front | [3] | high | `track.drive_front: false` |
| Track | 700 mm wide, **88 links per side** | [3] | high | `link_half_width`, `link_count` |
| Top speed | 35 km/h (model 1941, 45 t) → ~28 km/h at this model's weight | [1], derived | medium | `max_forward_speed_mps` |
| Road range | 250 km road / 150 km cross-country | [1] | high | — (not modelled) |
| Road wheel diameter | 0.60 m | estimated from photo proportion | **low** | `track.wheel_radius` |
| Ground clearance | 0.43 m | commonly cited, unconfirmed here | **medium** | `hull.belly_y` |
| Turret ring diameter | 1.535 m | commonly cited, unconfirmed here | **medium** | `turret.ring_radius` |
| Ammunition | 114 rounds | commonly cited, unconfirmed here | **medium** | `ammo_capacity` |
| Turret plan (L × W) | ≈ 2.64 × 1.96 m | derived from photo proportion | **low** | `RatioKind::TurretLengthToWidth` ≈ 1.35 |

**Honesty note on sourcing.** The rows marked low/medium were not confirmed against a primary
source while writing this dossier; they are carried as working values so the shape can be authored,
and they are explicitly excluded from the tight dimension gate until someone checks them against a
drawing or a measured specimen. A number without a source is a guess wearing a table row, and this
table says which is which.

## Form rules (what makes it *this* tank)

1. **Mass without slope.** The KV is a box of thick plate. Where the T-34 bet everything on a 60°
   glacis, the KV bet on millimetres: a shallow stepped bow, near-vertical 75 mm sides, and a flat
   engine deck. If the model reads as "sloped", it is wrong.
2. **Six small wheels at an EVEN pitch.** No T-34 Christie gap, no T-54 first/second stagger. The
   even run is the KV's ground signature and the thing that separates it from every other Soviet
   vehicle in the game at 300 m.
3. **Three return rollers carry the top run.** The belt runs taut and level off them — it does not
   sag onto the wheels the way the T-34's and T-54's do.
4. **Rear sprocket, front idler.** Soviet convention; the toothed wheel is at the tail.
5. **The turret is a LOAF, not a dome.** A long slab-sided casting with near-vertical walls, a
   broad flat roof, and rounded caps front and rear — roughly a third longer than it is wide. It
   must not read as a member of the T-34-85 / T-54 / IS-3 dome family.
6. **The rear DT ball in an armoured collar** closes the back of the turret. It is the single most
   recognisable fitting on the mod-1942 casting and the strongest anti-clone feature available.
7. **No commander's cupola.** The mod-1942 turret carries flush roof hatches and periscopes. A
   drum cupola on the roof would make it a KV-1S, which it is not.
8. **Full-length squared track guards.** Wide flat fenders running the whole hull, square-ended —
   not the T-34's stepped fender or the Centurion's hung skirt.
9. **The gun barely clears the bow.** The ZiS-5 is short: overall length gun-forward is essentially
   the hull length. Nothing else in the fleet has so little muzzle overhang, and a KV with a
   long gun is not a KV.
10. **A clean tube.** No muzzle brake, no fume extractor on the ZiS-5.

## Shape locks

Every rule above maps to a lock in `crates/kernels/vehicle_geometry/tests/kv1_benchmark.rs`
(authored in PR 3). A rule without a lock is a wish.

| Rule | Lock |
| --- | --- |
| 1 — mass without slope | `the_hull_is_thick_plate_not_slope` |
| 2 — six wheels, even pitch | `six_evenly_spaced_wheels_under_three_return_rollers` |
| 3 — three rollers, taut run | same assert + `top_sag_m` bound |
| 4 — rear sprocket | `the_toothed_wheel_is_at_the_tail` |
| 5 — loaf turret | `the_turret_is_a_long_slab_not_a_dome` (plan ratio, side slope, **and the baked roof band vs shoulder band** — the assert that fails if the shared dome shell is ever reused) |
| 6 — rear DT ball | `the_cast_turret_carries_its_rear_dt_ball` |
| 7 — no cupola | `no_cupola_crowns_the_roof` |
| 8 — squared fenders | `the_full_length_fenders_stay_inside_the_track_band` |
| 9 — minimal overhang | `the_zis5_is_a_clean_short_tube_that_barely_clears_the_bow` + `RatioKind::GunProtrusionToHullLength` |
| 10 — clean tube | same assert (`muzzle_brake.is_none()`, `evacuator.is_none()`) |
| anchor envelope | `the_hitbox_is_the_researched_body` — 6.75 / 3.32 / 2.71 m |

## Sources

1. **Wikipedia, "Kliment Voroshilov tank"** — https://en.wikipedia.org/wiki/Kliment_Voroshilov_tank
   Trusted for: the dimension and specification table (6.75 × 3.32 × 2.71 m, crew 5, 90/75/70 mm
   armour, V-2K 600 hp, torsion bar, 35 km/h, 250 km range) and the model-by-model variant
   breakdown. Not trusted for: which mass belongs to which production batch — its own variant list
   gives 45 t, 47–50 t and 52 t for overlapping designations.
2. **o5m6.de, "KV-1 Model 1942 Heavy Tank"** — https://www.o5m6.de/redarmy/kv1_1942.php
   Trusted for: the mod-1942 turret's identity — the reinforced cast turret with selective
   110–120 mm armour, the widened armour around the turret ring, and **the armoured collar around
   the rear machine-gun mount**. Not trusted for: dimensions; the page carries no specification
   table and cites Vollert, *KV-1 — Soviet Heavy Tank of WWII, Late Variants* (Tankograd) for those.
3. **GlobalSecurity, "KV-1 Heavy Tank — Design"** —
   https://www.globalsecurity.org/military/world/russia/kv-1-design.htm
   Trusted for: the running gear — six road wheels per side, three return rollers, rear drive
   sprocket, front idler, torsion-bar suspension with individual shock absorption per station, and
   **88 track links of 700 mm width per side**.

**Still to obtain** before the medium/low rows can carry tight anchors: a factory drawing or a
measured specimen for the road wheel diameter, turret ring diameter, ground clearance, turret plan,
and the ammunition stowage. Tankograd's Vollert volume (cited by [2]) is the obvious candidate.

## Gameplay translation

**Design intent — the anvil that cannot answer.** The slowest vehicle in the game and the
clumsiest-steering turreted one (only the casemate Jagdtiger turns worse, and it steers to aim),
protected from every angle, carrying a gun that cannot hurt what it most wants to kill. It
plays corners, flanks and city blocks, and it dies in the open. That is a role, not a handicap:
nothing else in Era II can bully a T-34-85 or a flanking Panther the way it can.

What reaches `TankSpec` and where the model deviates:

| Property | Value | Reason |
| --- | --- | --- |
| Hull 90 / 75 / 70 mm | as sourced [1] | — |
| Turret 100 / 100 / 90 mm | base casting, not the 110–120 mm selective patches | The armour model carries one thickness per facet; the selective reinforcement was applied to specific weak areas, not the whole face, so modelling it as a uniform 110 would overstate the tank. **The 100 mm turret SIDE is the thickest turret flank in Era II** — this is the vehicle's real armour identity, and it is honest. |
| Top speed 28 km/h | derived, not the sourced 35 km/h | 35 km/h belongs to the 45 t model 1941. This model carries the heavy cast turret on the same V-2K, and the mobility loss is precisely why the lightened KV-1S was developed in 1942. Using the lighter model's figure would misrepresent the vehicle. **Recorded as a deliberate deviation.** |
| Combat weight 47.0 t | midpoint of the 45–52 t source spread | Sources disagree on which mass belongs to this designation; 47 t sits with the cast-turret KV-1 and keeps the vehicle the lightest heavy in Era II. Medium confidence — revisit if a primary source settles it. |
| 76.2 mm ZiS-5 | AP ~78 mm at 100 m | **This gun cannot defeat a Tiger I's frontal armour at any range.** Under the honesty doctrine that is correct and must not be buffed away; the counterplay is flanking, tracking, and the scarce APCR round. |
| Ammunition 114 rounds | as commonly cited | The deepest rack in Era II — the honest-ammo economy's compensation for a gun that needs several hits per kill. |
| No interior | out of scope | Only the T-54 has a modelled interior; the Honest Steel interior program is separate. |

## Known deviations & follow-ups

1. **The stepped bow is a two-plate approximation.** The real KV bow is three plates — a shallow
   upper glacis, a near-vertical driver's plate, and a lower nose. `armor/vehicle_volumes.rs` bakes
   exactly one bow plane above the sponson fold and one below, so the third plate is folded into an
   averaged upper-glacis angle. Extending the shared armour kernel to three bow plates is out of
   scope for this vehicle's wave. **This is the headline deviation — a reviewer will look for it.**
2. **The gun overhangs the bow by 0.45 m; the real one barely reaches it.** Sources quote the
   KV's overall length as its hull length, so the honest figure would be ~0 m of protrusion. Two
   engine invariants forbid that: every barrel must clear its hitbox (`all_vehicles.rs`) and the
   mount frame requires 2.5 m of tube ahead of the trunnion (`mount.rs:82`), while the trunnion
   itself must sit under the mantlet for the shared socket contract (`vehicle_fittings.rs`). The
   modelled overall length is therefore 7.20 m rather than 6.75 m. Even so the KV overhangs about
   a fifth as much as the next-shortest gun in the fleet, so the silhouette identity survives; the
   cage asserts both margins so a later nudge to the hull or trunnion names its own cause.
3. **Low-confidence anchors** (road wheel diameter, turret plan) ship with widened ratio tolerances
   and a TODO; tighten them once source [2]'s Tankograd reference is obtained.
4. **The selective 110–120 mm turret reinforcement is not modelled** as distinct patches — see the
   table above.
5. **The superstructure is narrower than the width over tracks** (1.42 m half-width against a
   1.66 m track face). This is correct for the KV — the fenders bridge outward over the running
   gear — and it is recorded because the first cut got it wrong: see the review findings below.

## Close-up review (2026-07-27, `closeup_probe`)

Run and looked at, per the model-logic gate. What it caught and what it left:

**Fixed as a result of the review:**

- **The fenders were buried in the hull.** The first cut set the hull half-width flush with the
  track face, so the shelves protruded by 3 cm and the three return rollers were unreadable from
  the flank — on a vehicle whose rollers are one of its ten form rules. The superstructure is now
  1.42 m half-width and the fenders bridge outward; the rollers read at flank distance.

**All four review findings have since been fixed** (audit #18):

- **The mantlet** is now an authored cast mask, taller than it is wide — the opposite proportion
  to the T-54's flat oval, and the second authored mantlet mass in the fleet. Note this moved the
  *ballistic* mask too: `mantlet_radius` drives the armour patch in `armor/vehicle_volumes.rs`,
  so what you see and what you shoot grew together.
- **The turret walls** read flat. The loaf's straight side runs were being blended into its
  rounded caps by the cast smoothing group; hard seams cost zero triangles, which was the only
  currency left at 860 of a 900 ceiling.
- **The idler** is a spoked casting matching the road wheels. This was never a KV bug — the whole
  fleet shared one smooth drum — so the fix added an opt-in `IdlerFace` with a `Smooth` default
  and the other eight vehicles were untouched.
- **The plate is no longer bare** (defect **D15** for this vehicle): pistol ports and lifting lugs
  cast into the turret, spare track racked on the bow armour plane, a tow cable and two stowage
  boxes per fender shelf, three grab rails a side with a bracket at each end.

**Still open:**

- The remaining half of D15 is fleet-wide, not KV-specific: hull and turret still read as two
  different paints (the cast/rolled split is too far apart) and the running gear is a dark void
  with little contact shading. That closes with the W3 vehicle wave, for everyone at once.
- The Forge Studio tile gate is stale **fleet-wide** — an opt-in run drifts on vehicles this
  program never touched, including the T-54's hybrid bake. The KV's own tiles are recorded and
  hold; re-recording the rest is a separate PR and a separate review.

**NOT sealed.** Per `vehicle-fidelity-masterplan.md` this dossier and these renders are the floor,
not the bar. The vehicle is not done until a human has looked at it and until the gun's deliberate
weakness has been felt in a live battle. Green gates are not a seal.
