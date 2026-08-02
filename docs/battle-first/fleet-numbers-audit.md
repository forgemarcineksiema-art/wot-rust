# Fleet numbers — audit before 1.2 / 1.3 / 1.4

2026-08-02. Every number a shot resolves against, read out of the code and checked two ways: is it
HISTORICAL, and is it BALANCED. Armour, penetration, damage, muzzle velocity, rack capacity, hit
points. Plus what the six ammunition GAPs actually are, and what the debt registers hold today.

## Armour — all eight are historically right

Effective millimetres at 0°, from `ArmorProfile::effective_thickness_mm`:

| vehicle | hull front | hull side | turret front | turret side |
|---|---:|---:|---:|---:|
| T-54 obr. 1951 | 100 @ 60° → 82 | 80 | 200 @ 35° → **180** | 160 |
| Tiger I | 100 @ 9° → 90 | 80 | 100 @ 8° → **92** | 80 |
| Tiger II | 150 @ 50° → 150 | 80 | 180 @ 10° → **162** | 80 |
| Jagdtiger | 150 @ 50° → 150 | 80 | 250 @ 15° → **238** | 80 |
| Panther II | 100 @ 55° → 100 | 60 | 120 @ 20° → **108** | 60 |
| IS-3 | 110 @ 56° → 99 | 90 | 250 @ 55° → **225** | 160 |
| Centurion Mk 3 | 76 @ 57° → 68 | 51 | 152 @ 40° → **137** | 112 |
| T-34-85 | 45 @ 60° → 38 | 45 | 90 @ 28° → **81** | 75 |

Each matches its dossier and the standard references: the Jagdtiger's 250 mm casemate, the IS-3's
pike nose and 250 mm turret, the Tiger II's 150/180 Serienturm, the Centurion Mk 3's 76/152, the
T-34-85's 45 @ 60°. **No historical defect found in the armour table.**

**"The Tiger I has no mantlet" — WITHDRAWN, and it was wrong three ways.** `ArmorZone::Mantlet` is
a zone; `ArmorProfile::plate` derives the mantlet from the turret front at ×1.18 thickness; and
BOTH turret builders — the cast dome and the welded prism — place a mantlet patch on the front
face. The Tiger I's mantlet is 118 mm nominal against its 100 mm turret face, which is what the
real tank carried. A 75 or an 85 going through it at 100 m is history, not a modelling defect.

The claim came from reading `effective_thickness_mm(TurretFront, 0.0)`, finding 92, and stopping —
the same accessor, and the same mistake, as Finding 1 below. **The armour table has no defect at
all.** Locked by `flank_armour.rs::every_vehicle_carries_a_mantlet_thicker_than_its_turret_face`.

## Rack capacity — all eight are historically right

T-54 34 · Tiger I 92 · Tiger II 84 · Jagdtiger 40 · Panther II 79 · IS-3 28 · Centurion 65 ·
T-34-85 56. Every one matches its vehicle. The IS-3's 28 rounds against the Tiger I's 92 is the
kind of characterful spread the honest-ammo pillar asked for, and it is real.

## The penetration matrix, and what it says about balance

Every gun's stock and special round against every vehicle's turret front and hull side, at 0°:

**Finding 1 — WITHDRAWN. Side armour matters; this audit measured it with the wrong instrument.**

The first version of this section read "side armour is decorative: every gun penetrates every
vehicle's side, without exception", and a design decision was taken on it. It was wrong, and the
error is worth keeping on the page because it is the same one this programme has now made three
times: measure with a tool that does not include the thing being measured, then conclude
confidently.

The number used was `effective_thickness_mm(HullSide, 0.0)`, which is `nominal x weakspot` and
contains **no geometry at all**. A plate's slope does not live there — it lives in `plate_normal`,
which builds the true 3D outward normal, and the impact angle is taken against that. The file even
says so: *"slope must NOT be added to the impact angle anywhere downstream — it lives here, in
geometry."*

Resolved properly — through `resolve_penetration_through_screens`, across the track belt, at real
hull yaw — the flank is a genuine skill surface. Whether the stock round crosses belt and side:

| hull yaw | 7.5 cm KwK 42 (138) | 100 mm D-10T (185) | 84 mm 20-pdr (230) |
|---|---|---|---|
| 0° (broadside) | through everything | through everything | through everything |
| 45° | stopped by every 80 mm+ flank; through Panther II, Centurion, T-34-85 | through everything | through everything |
| 55° | stopped by all but the three thin flanks | stopped by the IS-3 only | through everything |
| 65° | stopped by everything | stopped by all but the three thin flanks | still through five of eight |

So: a flat flank is lethal to every hull in the fleet, which is historically right — no tank of
this era carried a side that stopped a contemporary anti-tank round square on. **Angle is the
skill, and it pays differently depending on what you are angling.** An 80–90 mm flank (T-54, both
Tigers, Jagdtiger, IS-3) starts turning medium guns away at 45°; a 45–60 mm flank (Panther II,
Centurion, T-34-85) cannot be saved by angling against the same gun. And the 20-pounder is the
weapon angling answers worst — it still crosses a T-54's flank at 65°.

That spread is exactly what makes side thickness a stat rather than decoration, and it is now
locked by `game_core/tests/flank_armour.rs` so the claim cannot drift back into prose.

**Finding 2 — the frontal hierarchy is real and steep.** Turret fronts split the roster into four
tiers: Jagdtiger 238, IS-3 225, T-54 180 / Tiger II 162, then everything else at 137 and below.

- The **Jagdtiger** is frontally immune to 18 of the 20 gun/round combinations in the game. Only
  the Centurion's APDS (300) and the D-10's BK-5 HEAT (280) crack it head-on.
- The **IS-3** resists 16 of 20; the 20-pounder (both rounds), the KwK 43's APCR and the BK-5 get
  through.
- Everything else is penetrable frontally by nearly everything, including the T-34-85's AP.

That is a coherent design — a casemate destroyer with no turret and a 13.5 s reload SHOULD be hard
to shoot from the front — but it is worth stating that two rounds in the whole game answer it, and
one of them belongs to the vehicle that opens Era III.

## Damage, velocity and rate of fire

| gun | AP alpha | pen | reload | DPM | HE alpha |
|---|---:|---:|---:|---:|---:|
| 12.8 cm Pak 80 | 530 | 223 | 13.5 s | 2 356 | 520 |
| 8.8 cm Pak 43/3 | 390 | 202 | 8.6 s | **2 721** | 300 |
| 8.8 cm KwK 43 | 390 | 202 | 8.8 s | 2 659 | 300 |
| 8.8 cm KwK 36 | 360 | 165 | 7.8 s | 2 769 | 300 |
| 122 mm D-25T | 390 | 175 | 12.6 s | 1 857 | **510** |
| 100 mm D-10T | 320 | 185 | 8.4 s | 2 286 | **430** |
| 7.5 cm KwK 42 | 240 | 138 | 6.6 s | 2 182 | 250 |
| 84 mm 20-pdr A | 240 | 230 | 8.0 s | 1 800 | 290 |
| 85 mm ZiS-S-53 | 200 | 145 | 7.4 s | 1 622 | 300 |

**Finding 3 — the Jagdtiger's 88 beats its own 128 at almost everything.** 2 721 DPM against
2 356, a 8.6 s reload against 13.5 s, tighter dispersion, and neither round changes what the gun
can crack frontally (both bounce off a Jagdtiger, both go through everything else). The 128's only
advantages are +140 alpha and +21 mm of penetration that buys no new target. As mounted, the 88 is
the default and the 128 is the sidegrade — which may be the intent (the dossier calls the long 88 a
"DPM/handling trade"), but the trade currently runs one way.

**Finding 4 — on Soviet guns HE now out-damages AP; on German guns it does not.** The D-10's HE is
430 against its AP's 320; the D-25T's is 510 against 390. The 88s' HE is 300 against 390. This
falls straight out of pricing HE by filler mass: Soviet tank HE carried 2.16 kg and 3.605 kg of TNT
where the German 88's shell carried 0.870 kg. It is historically grounded and it gives the two
schools genuinely different HE — but it means that on a T-54 or an IS-3, HE is the higher-alpha
round, and with 33–41 mm of penetration it will go through roofs (30 mm) and some rear plates. That
is a real tactic the fleet did not have yesterday and it deserves a playtest before it is called
balanced.

**Muzzle velocities** are all sourced or authored per `docs/ammunition.md`; nothing is derived.

## Hit points — the only table with no history in it

| vehicle | HP | mass | HP per tonne |
|---|---:|---:|---:|
| T-34-85 | 1 300 | 32.0 t | 40.6 |
| Centurion | 1 650 | 49.0 t | 33.7 |
| T-54 | 1 550 | 36.0 t | **43.1** |
| Panther II | 1 700 | 53.0 t | 32.1 |
| Tiger I | 1 850 | 57.0 t | 32.5 |
| IS-3 | 1 900 | 45.9 t | 41.4 |
| Tiger II | 2 050 | 69.8 t | 29.4 |
| Jagdtiger | 2 200 | 75.2 t | **29.3** |

HP is a pure game quantity — no source exists and none should be invented. What the table shows is
a consistent 29–43 band with the Soviet vehicles at the top: the T-54 carries 43 HP per tonne
against the Tiger II's 29. Whether that is the intended national character or an accident of
authoring one vehicle at a time is a question only the designer can answer, and it is the single
largest un-sourced lever in the combat model.

## The six GAPs — what they actually are

A GAP is **not** a missing number in the game. Every gun fires every round it has, and every round
has a full set of figures. A GAP means: *that figure came from judgement, not from a source*, and
the catalog comment beside it says so. The distinction matters because it tells the next person
what they are allowed to change without re-opening a design argument — a sourced figure is a fact
to be corrected only by a better source, a GAP is a balance decision to be re-tuned freely.

1. **7.5 cm Sprgr 42 — filler mass.** Shell mass (5.74 kg) and velocity (700 m/s) are sourced; how
   much explosive is inside it is not. Its 250 HP is set from the shell class.
2. **12.8 cm Sprgr — filler mass.** Shell mass (28 kg) is sourced, filler is not. Damage 520 HP.
3. **20-pounder HE — filler mass.** Nothing is sourced for this round beyond its existence.
   Damage 290 HP.
4. **12.8 cm Sprgr — muzzle velocity.** Set at 750 m/s from the class; the sourced 950 m/s belongs
   to the AP round.
5. **20-pounder HE — muzzle velocity.** Set at 850 m/s.
6. **Pzgr 40 penetration at 100 m — KwK 36 and KwK 43.** Every APCR table found starts at 500 m,
   so the 100 m figures (217 mm and 237 mm) are interpolated back from those tables and from the
   guns' own AP curves.

A seventh, half-open: the 122 mm OF-471's 800 m/s is convention rather than a figure any source
states, and its mass and filler ARE sourced.

Five of the six are HE, which is not a coincidence — HE ballistics are the least-documented part of
tank gunnery because nobody was trying to defeat armour with them.

## The debt registers, counted

| register | entries | what it holds |
|---|---:|---|
| `IDENTICAL_BODY_ALLOWLIST` | 19 | identical function bodies across test fixtures |
| `UNEXERCISED_CHECK_ALLOWLIST` | 9 | map-contract checks no test has ever made fire |
| `MIXED_MODULE_STYLE_ALLOWLIST` | 5 | crates using `mod.rs` and sibling files at once |
| `APP_TO_APP_ALLOWLIST` | 2 | `editor → client`, `client → server` |
| `UPWARD_ALLOWLIST` | 1 | `scene_build → renderer_api` |
| `RECORDED_MERGE_CEILING` | 1 | T-34-85 road wheels 0.09 m inside the end wheels |
| `ARMOUR_CONTAINMENT_EXEMPT` | 1 | the T-54 breech, which reaches into a mantlet that is a patch rather than a volume |
| `ORPHAN_ALLOWLIST` | **1** | `quality` itself — burned from 4 by deleting `panel`, `shell`, `experimental_geometry` |
| `DECORATIVE_FIELD_ALLOWLIST` | **0** | — |
| `IGNORE_ALLOWLIST` | **0** | — |
| `INLINE_VERSION_ALLOWLIST` | **0** | — |

Plus two behavioural debts recorded in code rather than in a register: the IS-3 draws its belt
bottom at 0.11 against the 0.03 it authors, and a non-penetrating HE hit anywhere — turret roof
included — chips BOTH tracks, a shortcut from before the damage layouts existed.

## What this audit found, after its own corrections

**Two of its four defect claims were errors, and both came from the same habit: auditing through a
descriptive accessor instead of through the path a shot actually takes.** `effective_thickness_mm`
is `nominal × weakspot` with no geometry in it; the geometry lives in `plate_normal` and the
resolution in `resolve_penetration_through_screens`. Read the summary, and a sloped flank reads
flat and a mantlet reads absent. Both claims are withdrawn above and both are now locked by tests
so they cannot be re-made in prose.

**The rule this earns:** an audit measures through the resolution path, never through an accessor
that summarises it. A number that is convenient to print is not evidence.

What survives:

1. **The armour table is historically correct on all eight vehicles**, mantlets included.
2. **Rack capacities are historically correct on all eight.**
3. **The frontal hierarchy is steep and intended** — the Jagdtiger resists 18 of 20 gun/round
   combinations head-on, the IS-3 16 of 20.
4. **Soviet HE now out-damages Soviet AP** and German HE does not, which falls out of pricing HE by
   filler mass. Situational rather than dominant: at 33–41 mm of penetration it lands full alpha
   only through roofs and thin rears, and is splash everywhere else. **Decision: keep.** The real
   test is a battle, and it is worth one.
5. **The Jagdtiger's two guns. Decision: keep both, unchanged.** The audit called the 88 dominant on
   DPM, and it is — by 13 % (2 721 against 2 356). The 128 answers with 36 % more alpha (530 against
   390). In a fleet whose hit-point pools run 1 300–2 200, alpha compresses the number of shots a
   kill takes far more sharply than a 13 % rate advantage stretches it: 530 kills a T-34-85 in three
   and a T-54 in three, where 390 needs four of each. That is a real trade, and the DPM column alone
   overstated the case.
6. **HP per tonne, 29 to 43. Decision: keep, and here is the rationale it was missing.** Absolute
   hit points rise with mass (1 300 → 2 200) but SUB-LINEARLY, on purpose: if the best-armoured
   vehicles also carried the deepest pools, armour and hit points would be paying twice for the
   same quality. The spread means armour does the differentiating and the pool does not, which is
   the honest split. It is un-sourced because no source exists — but it is no longer undecided.
