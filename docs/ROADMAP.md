# State of the game & road to release

The whole picture — not the current sprint. Program docs (urban map, destruction, fleet…)
are execution details; THIS is what the game is and what it still owes the player.
Release shape: buy-to-play (~20-25 EUR), 7v7, nation trees (lines and tiers), skill
matchmaking from day one. The retired release-ladder notes live in git history
(`git show 83b261d:docs/product-program.md`).

## The creed (why this game exists)

The honest tank: **no ±25% damage RNG**, an aiming envelope that is a hard maximum radius
(1.9–3.4 mrad at rest across the fleet, centre-biased, no gaussian tail — tighter than WoT's
3.2–4.2 and with no shot ever outside the circle), armor resolved against real 3D plates, no premium ammo (ammo-rack slots instead), no satellite-view artillery,
tiers and lines like World of Tanks, what-you-see-is-what-you-shoot everywhere. Every promise above is
test-locked, not marketing.

## Systems inventory

**DONE and test-locked** — meaning: the MECHANICS work and their promises sit in regression
tests. It does not mean final art polish; where finish varies, the partial list says so.
- **Combat**: 3D armor volumes from blueprints, ricochet/normalization, spaced armor & HEAT
  honesty, swept-shell kinetics (v24), HE splash, module damage, fire feel + feedback (the sim
  side — the shot's presentation channels are an open register, `inny-poziom-program.md` S); weakspots
  are measured patches (mantlet, cupola volume, aimable bow ports), not facet multipliers; rack
  cook-off the crew can fight, replicated with a teammate-only countdown.
- **Movement**: planar rigid-body hull (velocity + yaw inertia, drift), hull-down that actually
  works, track damage in two tiers. The authoritative hull is a sprung mass on its support plane (G7, 2026-09-02) — it used to be a rigid beam on a rate-limited
  attitude ramp; per-wheel travel is still presentation-side. The sprung hull is
  `inny-poziom-program.md` G7, landed.
- **Destruction (Honest Steel)**: buildings→rubble, breachable walls, crushable fences and
  tree lines, terrain craters, wall scars — replicated, honest. Mechanics only: the collapse is
  a state swap, cover damage is two constants and three maps are near-static — the register is
  `inny-poziom-program.md` Z.
- **Spotting**: per-vehicle optics, radio-dead isolation, LOS through real cover, minimap +
  spotted gates, fog-fairness lock across all weather looks; concealment that is readable —
  a stationary hull is seen from 70 % of range, firing reveals you for 8 s.
- **Maps**: 5 shipped (steppe / river town / mountain pass / city / lake defile), all data-driven with a
  full in-repo editor, playability BFS gates and golden hashes.
- **Fleet**: 8 blueprint-born vehicles across 3 nations (lines and tiers), one RON source feeding
  hitbox/armor/visuals; a review workshop with measurable gates (Studio, dossiers, ratios);
  no clones. Finish is UNEVEN — the T-54 is the benchmark, others trail it.
- **Presentation**: wgpu renderer (cascaded shadows, SSAO, HDR+bloom, weather + timeline,
  water, grass, battle FX, fully procedural buildings/trees — no imported flora),
  procedural audio (DSP, speed-of-sound delay), garage with workshop UX, full battle HUD,
  frame-time p50/p95/p99 measurement backing the one-look budget.
  **Drzewa 3.0 (2026-08-22, #620–#631): the homegrown SpeedTree** — every species grows a
  Weber–Penn-style skeleton (pure data, LOD rungs filter one growth), bark swept under the
  roundness law, cluster-card canopies cutting a wholly procedural SDF leaf atlas
  (coverage-preserving mips), a true crossed-quad impostor splatted from the same bake, and
  a three-level wind hierarchy (trunk cantilever / baked branch jitter / uv-gated leaf
  flutter) whose shadow moves with it. Measured on the MX330 @ shipped 1×: worst probe view
  11.2 ms of the 16.667 budget, two gated views (lineup + under-crown fill). Grown, not yet
  rolled out: two of six species are placed inside the maps (`inny-poziom-program.md` F2);
  the map-border backdrop ring stands on every species' impostor since F1 (2026-09-01),
  mixed per map by `HorizonSpec::flora`.
- **Sim/net foundation**: deterministic fixed tick, authoritative headless server, protocol
  snapshots (**wire v50** — the sprung hull's attitude velocities v50, breaches v39, `ShotFired` as a replicated fact v41, cook-off
  staging v42, rack countdown v43, a third-party projectile's owner withheld from a viewer
  who has not spotted the shooter v44, the battle clock on the wire v45, crew battle wounds
  team-private v46, concrete-round identity and the tungsten shatter flag v47, the
  test-only PrototypeMedium deleted v48), replay
  regression,
  bots with routes/fire discipline, 7v7
  mode. Remote input
  has epoch-safe reconnect, lightweight ACKs, snapshot-aligned prediction replay, and a terminal
  gameplay gate that freezes prediction on outcome/timeout/stalled world state instead of
  permitting a zombie client. Personal hit, damage, absorbed-impact, and lethal truth has its own
  ordered retransmit/ACK lane with exactly-once presentation instead of depending on snapshot loss.

**PARTIAL** (works, known debt):
- World close-range quality and authoring power: Cover 2.0, District/Scatter 2.0 — flora is
  already procedural-only (F0 landed). Immersja (14 PRs, closed 2026-08-22: opening-scale
  absolutes, town de-clone, horizon/backdrop rise, the camera riding the hull with recoil and
  tremor, engine-strain audio, the landmark rise #596) left only its optional tail (C3, A4.2).
- Breach interiors: cross-frame remesh merged, but museum detail, interior variants and
  interior audio are open.
- Fleet finish: per-vehicle polish passes outstanding outside the benchmark vehicles.
- Map rotation: every shipped map now carries a bot battle test (Prokhorovka and Mazurski
  landed 2026-08-26; Mazurski's soak also asserts nobody ends below the drowning line),
  but there is still no seeded rotation.
- Human playtesting: the maps have never met a second human.

**MISSING toward release** (the honest gap list; dates and thresholds are no longer written
down anywhere — the document that held them was retired):
1. **Production networking hardening**: the dedicated UDP path, lossy lifecycle, epoch-safe
   reconnect and client prediction work today. Still missing are public-session
   discovery/relay, player authentication, beta-validated lag compensation, cheating posture
   and dedicated-server operations — the register and wave plan are in
   `docs/multiplayer-production-program.md`.
2. **Meta & matchmaking**: OpenSkill-based MM, sessions/lobbies, player identity, and a
   record that a battle happened at all (today the game keeps none). Progression is **proof,
   never power**: no XP, no credits, no research, and no module unlocks — modules carry real
   stat deltas, so gating them behind time would be power behind time, which the creed
   forbids.
3. **Content breadth**: more vehicles per nation/line (Britain has one tank), 2-3 more maps,
   game modes beyond the single 7v7 skirmish.
4. **Product shell**: settings/keybinds UI, localization (PL/EN — the glyph atlas bakes ASCII
   only today), onboarding/tutorial, packaging/installer, crash reporting, store presence,
   trailers/devlogs, NAME of the game.
5. **Audio/presentation polish**: voice-over callouts, music, more FX variety.

## How work is organized

Programs of small PRs (1 branch = 1 PR), two local gates and no CI (`scripts/verify-pr.ps1`
per PR, `scripts/verify.ps1` — the full one — daily over what landed; `docs/engineering-rules.md`),
every feature landing with a locking test. Surviving program docs stay as doctrine.

**The open second pass (approved 2026-09-01)** is [inny-poziom-program.md](inny-poziom-program.md):
the audit that found the simulation mature and the visible layer thin, its register across the
kernels and fleet, tracks, Honest Steel, armour, flora, the shot, roles and the interface, and
the wave plan (W1 Widok → W2 Jazda → W3 Kuźnia 2.0 → W4 Obraz → W5 Miasto i Ziemia, with the
frame lane and the interface lane in parallel). Its decisions include the sprung authoritative
hull, gravity at 9.81, the rewritten dispersion creed above, instanced grammar-built buildings,
a scattering-LUT sky and per-vehicle fording depth.

**The release ladder approved 2026-08-04** lives in git history
(`git show 83b261d:docs/product-program.md`), not a file in the tree.

The open picture work is `docs/art-direction-program.md`, and it is on the marketing
critical path, because a store page is made of frames. One standing verdict outlived its
documents: the world reads too small. The garage's rebuild verdict was answered by Hala 3.0
(PR #536–#557, closed 2026-08-10); what the room still owes is itemized in
`docs/hala-4-program/plan.md` — at the shipped 1× the garage frame is ~0.5 ms over the MX330
budget after the P2+P4+P5 diet (the 4× MSAA candidate stays parked ~3.5 ms short, with the
deck reflection behind it), and the product shell around the garage (battle results,
settings, PL glyphs) does not exist yet.
