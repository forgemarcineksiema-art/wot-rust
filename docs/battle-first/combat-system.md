# The combat system, end to end — what stands, and where it grows

2026-08-02. A full reading of the shot chain: aiming, ballistics, armour, penetration, interior
damage, and the loop that surrounds them. Method note, earned twice this week: **everything below
was read through the resolution path** — `plate_normal`, `resolve_penetration_through_screens`,
`apply_internal_module_path` — never through a descriptive accessor. A number that is convenient
to print is not evidence.

Companion documents: `fleet-numbers-audit.md` (the numbers), `docs/ammunition.md` (the rounds),
`audit-register.md` (historic findings), `program.md` (the plan this feeds).

---

## I. The chain as built

### 1. Intent → aim

Dispersion is a single scalar per tank (`aim_dispersion_mrad`) with four inflows and one outflow:

- **settled minimum** — the gun's `dispersion_mrad`, raised ×2.5 at a destroyed gun module
  (a wounded gun shoots worse, smoothly, from partial damage);
- **movement bloom** — hull speed, steering and turret/gun traverse each feed bloom per tick;
- **shot bloom** — firing costs accuracy;
- **recovery** — exponential toward the minimum on the gun's `aim_time_seconds`.

The shot direction perturbs the aim by a deterministic pair hashed from `(tank, tick,
shot_index)` — **reproducible dispersion, no server RNG**. This is the "honest tank" pillar
delivered mechanically: there is no ±25% roll anywhere in the chain — not on damage, not on
penetration. The only randomness a player meets is the aim cone they can see, and even that
replays bit-identically.

The fire trigger buffers 0.3 s (`FIRE_BUFFER_S`): a click inside the window is held and released
the tick the breech closes. The reticle previews through **the same collision implementation the
server resolves with** (`shell_trace`, shared), so a previewed hit is never one the server
rejects — the single most load-bearing honesty decision in the codebase.

### 2. Flight

Semi-implicit Euler: linear drag per shell class (AP 0.09/s, APCR 0.21/s, HEAT & HE 0.05/s),
gravity, shared integrator between the authoritative step, the reticle preview and the
gun-elevation solver. The shell is a **swept volume** half a caliber wide (protocol v24), so
grazes clip terrain and cover honestly. Sound arrives at 343 m/s, one flyby crack per shell.

Kinetic penetration falls out of impact **velocity** (De Marre-style power 1.5), so range bleeds
AP and APCR penetration while HEAT holds flat — the sidegrade geometry the ammunition pass
depends on.

### 3. Meeting armour

The plate a shell meets is real geometry: convex volumes baked from blueprints (the IS-3's pike,
the T-54's casting taper, sector domes and welded prisms), each plane tagged with a zone. The
impact angle is taken against the **true 3D normal** — slope lives in geometry, never added
downstream. On top of that:

- **the mantlet** is a patch on every turret at ×1.18 the face thickness;
- **spaced stacks** resolve outermost-first: skirt → track belt → side plate, each stripping its
  line-of-sight steel; HEAT pays the whole stack doubled (standoff kills the jet); HE bursts on
  the first surface it touches; a **broken track is not there any more** and stops screening;
- **overmatch**: past 3× caliber-to-thickness the plate cannot glance the shell and its presented
  angle caps at 70°;
- **normalization**: AP turns 5° into the plate, APCR 2°, chemical rounds none;
- **the glancing band (new, W1.3)**: from 60° a kinetic round starts skidding, losing up to a
  third of its bite by 70°, where it ricochets — and a ricochet is a **continuation**: the shell
  reflects off the true normal with retained speed and flies on, once;
- **open channels**: a shell whose full cross-section fits through an existing breach pays no
  steel for the entry — the second shot through the same hole is the rare mechanic almost nobody
  ships, and it is here.

### 4. Inside the hull

A penetration runs a ray through the vehicle's **damage layout** — 16–17 authored components per
hull (breech, recoil gear, racks, fuel, engine, gearbox at the *driven* end, shafts on the German
hulls, radio, suspension) — spending residual penetration on each component's material resistance.
Energy fractions scale module damage; components hit with **spall-level** energy shed deterministic
spall rays that wound neighbours; components hit with **fire-level** energy can ignite what is
flammable. Ignition is energy-gated at the source — a bow shot does not light fuel; the tuning
target is about two fires per battle.

Consequences are mechanical, not cosmetic: a damaged gun widens dispersion; thrown tracks
immobilise a side and record *where* the belt broke; an ammo-rack detonation pops the turret off
the ring, ballistically, replicated; fuel and engine fires burn deterministically (12 s fought,
pulsed damage, credited to the arsonist); crews repair on fixed clocks. Two-piece ammunition is
anatomy: the IS-3's charges ride the bustle, its projectiles ride low.

### 5. The loop around it

7v7, 600 s, the clock decides **on the board** (tanks and damage — never a 4:1 "draw").
Spotting is team-shared LOS + range on a cadence, the sight line stepping at two samples per
terrain cell so the eye can never miss a crest the shell would hit. Bots **choose** — a five-term
score (proximity, cripple, return fire, gun-laid-on-me, focus) with a deterministic
concentration snapshot — and withdraw below a quarter health under fire. Snapshots at 20 Hz with
per-tick events accumulated between them; the shot itself is a replicated **fact**
(`ShotFired { shooter, shell_id }`, protocol v41), not an inference from a reload clock.

---

## II. The load-bearing decisions

Worth naming, because future work should lean on them rather than around them:

1. **One collision implementation.** Server, reticle preview and bot aim sweep share
   `shell_trace`. Any new mechanic that traces (spall to exterior? shrapnel?) must join it, not
   fork it.
2. **Determinism as the honesty budget.** Dispersion, spall directions, fire cost, crater merges —
   all reproducible. New systems inherit the rule: no hidden rolls; variance only where the player
   can see and learn it.
3. **Data first.** Blueprints → armour volumes → damage layouts → assets, with append-only enums
   and gate rules. A new combat feature starts as data with a source (the ammunition pass is the
   template).
4. **Events are facts.** v41's `ShotFired` carries `shell_id` precisely so future cues (own-shot
   prediction, tracer-to-impact correlation, fire-reveal spotting) can correlate instead of guess.
5. **Audits measure through the path.** Two of four findings in the fleet audit died of accessor
   reading. The rule is in the audit and in memory; it applies to every claim below.

---

## III. Where it grows — ranked

Ordered by player-felt impact against cost, with what each leans on.

### 1. Historical gun arcs (small, loud, overdue)

**Every gun in the catalog but the T-54's runs the fleet default −8°/+20.1°.** The defaults were
placeholders; the dossiers already carry real arcs. This flattens a defining balance axis:
gun depression is *the* hull-down stat, and the fleet spans it dramatically — the IS-3's real ~−3°
against a Centurion's ~−10° is a whole identity (the pike-nosed brawler that cannot use a ridge
against the Western turret that lives on one). One catalog pass + a `flank_armour`-style lock,
research-first like the ammunition. Synergises directly with authored relief (the 2.1
replacement): ridges only matter if depression differs.

### 2. The spotting decision (2.6), with fire-reveal riding v41

Open design decision, now with a mechanical hook it did not have before: **a shot is a replicated
event, so "firing reveals you" is an afternoon, not a system**. A minimal honest model that gives
scouts an identity without camo-stat sorcery:

- stationary vehicles are harder to spot than moving ones (binary, readable);
- firing applies a spotting bonus against the shooter for N seconds (`ShotFired` drives it);
- foliage remains what it is (LOS blockers), no percentage camouflage.

Everything stays deterministic and explainable in one sentence each — the design bar this game
sets. Needs a written decision first; the register has asked for one since July.

### 3. Ammunition cook-off (staged rack fire)

The pieces already exist: racks are components, fires are deterministic wounds, the IS-3 separates
charges from projectiles, turret pop-off exists. The missing stage: a rack hit with fire-level
energy **starts a countdown instead of an instant detonation** — a burning rack the crew can still
win against (extinguish window), with detonation at the end popping the turret. Deterministic,
readable, and it turns "ammoracked" from a coin-flip impression into a drama with a decision in
it. The German sponson racks and the Tiger II bustle make placement matter exactly as the damage
layouts intended.

### 4. The mantlet as a volume (kills the last exemption)

The T-54's breech exemption in the containment rule exists because the mantlet is a *patch* on the
casting rather than a volume with its own thickness. Making mantlets real volumes: honest gun-mask
weakspots (shooting *through* the mask into the breech — currently approximated by the ×1.18
patch), the breech containment test closes, and `ARMOUR_CONTAINMENT_EXEMPT` goes to zero. Medium
effort; touches vehicle_volumes and the zone map.

### 5. Weakspot patches replace weakspot multipliers

`weakspot_multiplier` (0.82 on a glacis to stand in for the MG port) is a flat discount over a
whole facet — the last coarse hack in an otherwise geometric model. The patch mechanism the
mantlet uses generalises: cupolas, MG bosses, driver hatches as small tagged regions with their
own thickness. This is what makes "aim at the cupola" a real skill on a hull-down target. Pairs
with #1: depression creates hull-down fights, patches give them an answer.

### 6. Tactical bots (the second half of 2.4)

Target choice landed (#378); movement is still route-driven. The next behaviour with battle-wide
effect: **seeking hull-down** — which needs the hull-down census instrument first (count and
locate fightable crests; also the missing gate for relief authoring). One instrument serves both
the map contract and the bot brain. Then: approach via cover (the live-cover raycast already
exists for LOS), reverse out when the glance band says the angle has gone bad.

### 7. Own-shot prediction (1.1, the remainder)

Measured at one tick locally — a networked-play fix. Client mirrors the fire gates it already
knows (reload, ammo, gun/rack functional, buffer) and plays the muzzle cue on click;
`shell_id` correlation de-duplicates against the authoritative `ShotFired`. One decision needed:
on a mispredict (rare — desync of ammo count), swallow the ghost flash or visibly retract.
Recommend: swallow (a 50 ms ghost flash with no shell is less damaging than a retraction).

### 8. Ballistic identity (quadratic drag)

Linear drag with a velocity floor is a serviceable approximation; its cost is that long-range
feel is uniform across shell classes. Quadratic drag with per-shell ballistic coefficients would
let APCR bleed harder past 500 m (its real weakness against APDS/AP at range), sharpening the
sidegrade geometry the authored rounds set up. **Verify against the sourced penetration tables
before landing** — the falloff curve and the drag must agree with the 500/1000/1500 m columns in
`docs/ammunition.md`, or the model's numbers stop being the dossier's numbers.

### 9. Hull damage vs residual energy — a fork to decide, probably against

Today a penetration deals its shell's flat `damage_hp` to the pool, and the *energy* story
(fractions, spall, fire thresholds) applies to modules only. One could scale pool damage by
residual penetration — deep pens hurt more. **The argument against is the game's own founding
pillar**: flat, knowable alpha is the anti-±25% promise. Recommend documenting the fork and
declining it: keep alpha flat, keep depth expressed through *what the ray hits* (modules, fuel,
racks), which the layouts already do richly. Written here so it is a decision, not a drift.

### 10. Crew — deferred, deliberately

Struck from the HE brief by decision; `Crew`/`CrewRole` remain data without consequence. When it
comes, it is its own system (wounded loader → longer reload, etc.) with its own balance pass, and
HE/spall/fire all gain a second damage channel for free — the interior ray already knows what it
passed through. Blocked on the user's call, correctly.

### 11. Presentation hooks whose data already exists (blocked on "skip visual")

- `glance_loss` in every `PenetrationResult` — the near-glance FX signature, unread;
- `ShotFired.shell_id` — tracer-to-impact correlation, unread;
- `track_break_t` — where on the hull the belt snapped, read by FX already;
- twelve material roles reaching the shader since #372 — the "nothing reads as steel" fix waits
  on materials authoring, not on plumbing.

---

## IV. Recorded debts that touch combat (kept visible)

- **HE direct-hit shortcut**: a non-penetrating HE hit *anywhere* — turret roof included — chips
  BOTH tracks (`impacted_module` returns `Suspension` unconditionally). The splash path now does
  this honestly by proximity to the band; the direct path should adopt the same rule.
- **Six ammunition GAPs** (velocity/filler for three HE rounds, Pzgr 40 @ 100 m) — balance
  decisions wearing that label in the catalog, correctable only by better sources.
- **Module catalog options**: every vehicle but three offers `vec![stock]` behind a wildcard —
  deferred by decision, recorded here because it borders the loadout half of combat.
- T-34-85 gear merge 0.09 m; IS-3 drawn belt bottom 0.11 vs authored 0.03 (shape decisions).
- The inherited red test (`the_visible_gun_mount_is_no_wider_than_its_canvas_cover`) — untouched
  by standing instruction.

## V. Recommended order

1. **Gun arcs** (research → author → lock) — smallest change with the largest identity payoff.
2. **Spotting decision written**, then fire-reveal on v41.
3. **Cook-off** — drama from existing pieces.
4. **Mantlet volumes**, then **weakspot patches** — the armour model's last two abstractions.
5. **Hull-down census** → **tactical bots** — one instrument, two consumers.
6. **1.1 prediction**; **quadratic drag** behind table verification.
7. Decline #9 in writing; park crew until called.

What this order preserves: every step is data-first, deterministic, testable through the
resolution path, and none of it waits on the visual program.
