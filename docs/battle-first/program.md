# Battle First — the plan

Ordering principle: **the battle is the product; nothing around it matters until it feels right.**
Waves are ordered by what the player feels, not by what is architecturally tidy — with the ratchets
first, because everything below adds code.

Findings referenced here live in [`audit-register.md`](audit-register.md); numbers in
[`measurements.md`](measurements.md).

**STATUS — proposed, not started.**

---

## W0 — Ratchets (5 PR, no gameplay risk)

Twelve gate rules in `crates/tooling/quality`, each landing green with an explicit allowlist. Full
specification: [`gate-rules.md`](gate-rules.md).

Layers · exhaustive dispatch · coverage asserts · enum↔WGSL binding · no silent caps · data
contracts · no orphan crates · manifest hygiene · gate completeness · duplication in `tests/` ·
naming · honest getters.

**Why first:** every wave below adds code. Without the ratchets each one adds fresh instances of the
same six patterns the register documents.

---

## W1.0 — What playing the game revealed

From [`playtest-2026-08-01.md`](playtest-2026-08-01.md). **Read the correction there first**: the
first pass produced three dramatic findings — zero ricochets, a battle that never resolves, passive
bots — and all three were instrument error. The armour model works (8 ricochets in 79 impacts, 8 of
10 above-threshold hits bouncing), the timer is 600 s and the battle ends on it, and 9 of 14 tanks
die in a real fight.

**a. A 4-to-1 battle is declared a draw.** `Draw { TimeExpired }` with a fourfold tank advantage is
the least satisfying resolution available. Award the win on tanks remaining or damage dealt. This is
the one genuinely broken thing the playtest found, and it is cheap.

**b. Fix the camera freezing on player death.** `tick_death_spectate` exists but does not drive this
path.

**c. Bot decision depth** (moved down from a false alarm, but the critique stands): target selection
is `bot_nearest_engageable_enemy` and nothing else — no threat priority, no finishing a cripple, no
answering incoming fire, no withdrawal. The bots fight; they do not choose.

## W1 — The feel of the shot (3–4 PR)

The shot chain is the best-built part of the project — one collision implementation shared by the
server, the reticle preview and the aim sweep, so a previewed hit is never one the server rejects;
the shell leaves the visible muzzle through the real pivot chain; a 0.3 s fire buffer; impact FX that
distinguish penetration, ricochet, non-penetration, water and stone; 343 m/s sound delay and a
one-per-shell flyby crack.

**1.1 Predict the player's own shot locally.** Measured at **1 tick = 16.7 ms** locally, not the
"≤50 ms" this audit first assumed — so this is a networked-play and edge-case fix, not the emergency
it was billed as. What stands: the cue is derived rather than predicted, and a tank that fires and
dies inside one snapshot window shows no flash at all.

**1.2 Replicate the shot as an event** instead of deriving it from a `reload_remaining_s` jump.
Closes 1.1's remaining holes.

**1.3 A ricochet transition band** — 60–70° energy loss plus a visual near-glance signature. Pairs
with W1.0a.

**1.4 One battlefield memory budget** — 64 craters for a whole battle against 128 terrain scars and
16 decals per tank; the field never reads as shelled.

**1.5 Measure the frame — BLOCKING for W2.1.** `perf_capture` reports bake times only; no frame-time
measurement exists anywhere in the project, and the "one look" policy has neither test nor tool. Add
p50/p95/p99 plus a budget test before densifying terrain 4–16×.

**1.6 Cheap visual fixes**, from [`visual-review-2026-08-01.md`](visual-review-2026-08-01.md):
barrel thickness · grass ring fade · per-instance grass variance · tree trunk proportions and
placement jitter · the river's hard bank edge · unlit tree undersides and polygonal shadows.

---

## W2 — Depth of the battle (6–8 PR)

Every system here has a strong skeleton and a shallow last 30%, and that last 30% is where the
gameplay lives.

**2.1 Terrain resolution — prototyped, measured, and smaller than planned.** `cell_m: 5.0` means
the tank is one cell. The sweep on Bystra (see `measurements.md`) says:

- **2.5 m is the target**: 4× the samples for +18 % scene work, contracts still pass.
- **2.0 m is the ceiling**: +28 %, and scene work alone is 15.99 of the 16.67 ms budget on hardware
  above min spec.
- **1.25 m is off the table.** The map fails its own playability contract — the passability rule is
  a gradient, and a fine grid resolves local pitches that the 5 m sampling averaged away, so
  authored hillsides stop being drivable. Densifying adds walls nobody placed.
- `cell_m` is constrained by `symmetry: MirrorZ` to even divisors of the map size: 5.0, 2.5, 2.0,
  1.25 and nothing between.

So: one map at 2.5 m with the contracts re-run. 1.25 m returns only behind a sculpt rewrite aimed
at the gradient — a separate program, not a step in this one.

**WITHDRAWN 2026-08-02, after measuring the benefit instead of only the cost.** See
`measurements.md`, "the other half of the ledger". Densifying buys about one centimetre of RMS
relief at tank scale (0.061 → 0.070 m) for 1.87 ms of a 16.67 ms frame, because sampling cannot
create relief nobody authored — and a 5 m grid already represents any crest from ~10 m wavelength
up, which is what a hull-down position is. Ostrogorsk is excluded outright at 18.55 ms of scene
work. **2.1 becomes: author the relief where the fight happens, with the Ridge brush and strokes
that already shipped, at zero frame cost.** Densification reopens only if authored relief hits the
5 m wall — folds narrower than ~10 m — where it would then have a named purpose and a known place.

The prerequisites landed anyway and stand on their own: one climb-grade constant (PR #379), a sight
line whose step follows the grid (#379), and a pooling contract that asserts its property instead
of balancing on 664 texels (#380).

**2.1 needs an instrument, and that is the whole lesson of the withdrawal.** "Author the relief
where the fight happens" is a claim with no number attached, which is exactly what densifying was
before somebody measured it. Nothing in the map contract can say whether a sculpting session
achieved anything. The missing check is a **hull-down census**: count and locate the crests a tank
can fight from — a drivable approach, a rise of ~0.8-2.0 m within 5-15 m, hull hidden and turret
over — and give the contract a floor. Then the Ridge brush has a target instead of a hope, and a
map cannot be called playable with nowhere to fight from. Build it before the sculpting, not after.

**2.2 Armor depth — HALF OF THIS ALREADY SHIPPED; re-scope before scheduling.** The line below was
written against an older tree. `armor/resolve.rs` already implements normalization AND overmatch:
`overmatches()` decides when a shell's caliber bites into a plate instead of glancing, and the
resolved thickness angle is capped for overmatched plates. What may still be open is the second
half — damage that depends on what was hit — and even that overlaps the damage-layout components
and the fire model that landed since. Audit the code before planning the work; do not schedule what
is already in master.

~~normalization as a function of caliber against thickness~~; damage that depends on what was hit.

**2.3 Author the ammunition** — the derived ×1.20/×1.25/×0.85 rounds break the doctrine the module
catalog explicitly upholds.

**Measured 2026-08-02, and it is worse than a style complaint.** Twelve guns, every round the game
can fire, printed from `GunSpec::ammo_options()`:

| gun | AP | slot 1 | HE |
|---|---|---|---|
| 84 mm 20-pounder | 230 mm / 240 HP | APDS 300 mm | **80 mm** / 336 HP |
| 122 mm D-25T | 175 mm / 390 HP | *fabricated APCR 219 mm* | **61 mm** / 546 HP |
| 12.8 cm Pak 80 | 223 mm / 530 HP | *fabricated APCR 279 mm* | 78 mm / 742 HP |
| 8.8 cm KwK 43 | 202 mm / 390 HP | *fabricated APCR 252 mm* | 71 mm / **546 HP** |
| 7.5 cm KwK 42 | 138 mm / 240 HP | *fabricated APCR 172 mm* | 48 mm / 336 HP |

Three defects fall out of it, and none is cosmetic:

1. **HE penetration ranks by AP penetration.** It is 35 % of the AP round, so the 84 mm gun has
   the highest-penetrating HE shell in the game — ahead of the 122 mm (61 mm) and the 128 mm
   (78 mm). High-explosive penetration comes from caliber and filler; it cannot be inherited from
   how good the same gun's armour-piercing round is.
2. **HE damage is 1.4x AP damage,** so the 122 mm D-25T and the 88 mm KwK 43 fire HE shells doing
   IDENTICAL damage (546 HP) because their AP alpha happens to match. A shell's identity is coming
   from a different shell.
3. **Guns that never fielded a special round are given one.** The 12.8 cm Pak 80 and the 122 mm
   D-25T get fabricated APCR because the fallback exists, which is the "no clones" rule broken by
   arithmetic instead of by copying.

The four authored rounds (Centurion APDS, ZiS-S-53 APCR, both D-10 HEAT) show the shape the rest
should take. What blocks finishing it is not code: HE ballistics for twelve guns are not in the
dossiers, and inventing them is a BALANCE decision, not a refactor. Research the rounds into the
dossiers first — the same source-and-confidence pass every other number in this project got — then
author them and delete the multipliers together with the fallback that allows them.

### The three decisions, taken 2026-08-02

**What HE is for.** Chip damage, finishing a cripple, and a weapon against tracks and crew — that
is its job on the anti-tank guns this roster fields. AND a genuinely powerful shell where the gun
is built for it, which is a class of gun the roster does not have yet: large calibre, low velocity,
a heavy filler. So HE is not one round with one role; it is a round whose role follows the gun that
fires it. Two consequences: its damage and penetration must come from the SHELL (caliber, filler),
never from the gun's armour-piercing round, and the splash has to reach modules and crew for the
utility half to exist at all — which the damage-layout components and the fire model now make
possible.

**Whether every gun carries three slots: no — history decides.** A gun fields the rounds it
actually fielded, so the slot count differs from gun to gun and that difference IS a property of
the gun. The 12.8 cm Pak 80 keeps AP and HE, and losing a fabricated APCR is not a nerf, it is the
removal of a round that never existed. The fallback dies with the multipliers.

**APCR stays, because it is historical.** Pzgr 40 for the German 75s and 88s, BR-365P for the
ZiS-S-53, APDS for the 20-pounder. What goes is the INVENTED APCR handed to guns that never had
one — not the round class.

Ordering follows from this: research per gun (what it fired, with sources and confidence) → author
per gun → delete `special_shell`'s derivation and the HE multipliers together. The "powerful HE"
half stays a design note until a gun that deserves it joins the roster.

**The research pass is DONE: `docs/ammunition.md`.** Twelve guns, keyed by gun rather than by
vehicle because the KwK 43 and the Pak 43/3 are one weapon. It confirms the fabricated-APCR charge
against the 12.8 cm Pak 80 and the 122 mm D-25T (neither fielded a tungsten round), and it turns up
a defect nobody had named: the HE velocity multiplier is roughly right for the German guns and
**30 % wrong for the Soviet ones**, whose tank HE was full-charge. The 100 mm D-10's real HE round
leaves the muzzle FASTER than its AP round — 900 against 895 — and the game flies it at 626. Flight
time and drop are what a player leads with, so that is a third of the lead.

Six holes remain, each recorded as GAP rather than filled with a guess: HE filler for three guns,
HE velocity for two, and Pzgr 40 penetration at 100 m for the two 88s.

### What the HE mechanism can actually deliver (audited 2026-08-02, before authoring anything)

The decided role has three parts. The code answers them differently, and the numbers are worth
nothing until it answers all three, so this audit re-orders the work.

**Chip damage and finishing: WORKS.** `burst_he_splash` throws attenuated blast at every hull
inside the radius, soaked by the thinnest plate facing the burst and killed by terrain between —
so a burst finds the roof and the engine deck, heavies shrug off what mediums feel, and a crest
that stops the shell stops its pressure wave.

**Tracks: WORKS ON A DIRECT HIT, NOT ON A BURST.** A non-penetrating HE hit degrades BOTH bands
(`module_hit::degrade_both`) and reports `ModuleSlot::Suspension`. But `shell_splash.rs` contains
no reference to modules, tracks or crew at all: a burst BESIDE a tank takes hit points and cannot
throw a track. So the round is a track weapon only when it lands on the hull, which is the case a
player is least likely to be aiming for when they load HE at a fast flanker.

Worth a second look while the file is open: `impacted_module` returns `Suspension` for ANY
non-penetrating HE hit regardless of where it landed, so HE bursting on the turret ROOF chips both
tracks. That is a shortcut from before the damage layouts existed.

**Crew: NO MECHANISM.** `game_core::crew` defines `Crew` and `CrewRole`, and nothing in `sim`
damages a crewman — the only mentions are a comment in `fire.rs` and the repair clocks. Crew is
data with no consequence. The most distinctive half of the role chosen for HE has nowhere to land
today, and authoring twelve guns' worth of shells into that gap would produce rounds that chip hit
points and break tracks they hit directly, which is what they already do.

**DECIDED 2026-08-02: the crew half is struck.** HE's role is chip damage, finishing a cripple and
tracks — three jobs the model can express — plus the heavy-gun case when a gun that deserves it
joins the roster. Crew damage is not re-scoped, it is DEFERRED as its own question: injured crew
slowing reload, aim, traverse and repair is a system with its own design and its own balance, and
it will be decided on its own terms rather than smuggled in as a property of one shell type.

What remains inside 2.3 before the research pass is small and concrete: **splash must reach modules
and tracks**, so a burst beside a hull can throw a band instead of only shaving hit points. Today
that only happens on a direct hit. A number is worth sourcing once the model can spend it.

**2.4 Bot decisions** — threat priority, finishing a cripple, answering incoming fire, focus fire,
withdrawal.

**2.5 The T-54 turret front and the distant buildings** — the front reads as a mushroom; buildings
at range read as untextured slabs.

**2.6 Decide the spotting model** — pure LOS + range means no scouting, no ambush, no light-tank
identity. That should be a decision on the record, not an omission.

---

## W3 — The three critical register items (2 PR)

Silent armor (B1/B2) · `MaterialRole` 9/10/11 (B3) · the `.take(16)` spotting cap (B4).

---

## W4 — Structure (7–8 PR)

Per [`target-architecture.md`](target-architecture.md): `ui_kit` extracted · `probe` binary ·
the layer DAG · `mesh_core` to L0 · `scene_build` to L4 · `battle_host` to L3 · the facade
dismantled · orphan crates deleted.

**The T-54 stack**: pass the whole `&BlueprintFile` into `t54_hybrid()` (deleting 11 re-typed
constants, one pair of which already drifted); delete the dead metaball path (~600 lines) and rewrite
the test that keeps it alive; rename `HybridVisual` → `VisualDetail`; serialize the loft into RON so
fifteen `if kind == T54_1951` sites collapse into one question about data; move `solid/t54*.rs` into
`vehicle_build`.

---

## W5 — Hygiene (2 PR)

Delete `rapier3d` + `parry3d` (dead, heavy) · the ~400-line dead renderer layer · fix the stale
anti-wallhack warning in `spotting.rs` · fix the drag ODE comment in `weapon.rs` · reconcile
`engineering-rules.md` with `verify.ps1` · restructure `docs/` and drop the document assertions from
the gate.

---

## Order

1. **W0** — the ratchets.
2. **W1.0a** — the draw-at-4-to-1 outcome rule. Cheap, and the clearest broken thing measured.
3. **W1.5** — frame-time measurement. Blocking for W2.1.
4. **W1.6** — the cheap visual fixes.
5. **W2.1** — terrain resolution, once there is a frame number to measure it against.
6. Everything else in the order written.

**Does a battle in this game do what the design says it does?** After correcting three false alarms:
largely yes. Shells bounce, flanks matter, the fight resolves. What is thin is depth — bot decisions,
armour nuance, terrain that gives the fight somewhere to happen — and what is broken is the rule that
calls a 4-to-1 a draw.
