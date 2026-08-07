# State of the game & road to release

The whole picture — not the current sprint. Program docs (urban map, destruction, fleet…)
are execution details; THIS is what the game is and what it still owes the player.
Release shape: buy-to-play (~20-25 EUR), 7v7, three eras, skill matchmaking from day one.
The road to that release — builds, dates, decision gates and the numbers that end each one —
is `docs/product-program.md`.

## The creed (why this game exists)

The honest tank: **no ±25% damage RNG**, dispersion ~0.1-0.3 mrad, armor resolved against
real 3D plates, no premium ammo (ammo-rack slots instead), no satellite-view artillery,
eras instead of tiers, what-you-see-is-what-you-shoot everywhere. Every promise above is
test-locked, not marketing.

## Systems inventory

**DONE and test-locked** — meaning: the MECHANICS work and their promises sit in regression
tests. It does not mean final art polish; where finish varies, the partial list says so.
- **Combat**: 3D armor volumes from blueprints, ricochet/normalization, spaced armor & HEAT
  honesty, swept-shell kinetics (v24), HE splash, module damage, fire feel + feedback; weakspots
  are measured patches (mantlet, cupola volume, aimable bow ports), not facet multipliers; rack
  cook-off the crew can fight, replicated with a teammate-only countdown.
- **Movement**: planar rigid-body hull (velocity + yaw inertia, drift), per-wheel suspension
  with sprung attitude, hull-down that actually works, track damage in two tiers.
- **Destruction (Honest Steel)**: buildings→rubble, breachable walls, crushable fences and
  tree lines, terrain craters, wall scars — replicated, honest.
- **Spotting**: per-era optics, radio-dead isolation, LOS through real cover, minimap +
  spotted gates, fog-fairness lock across all weather looks; concealment that is readable —
  a stationary hull is seen from 70 % of range, firing reveals you for 8 s.
- **Maps**: 4 shipped (steppe / river town / mountain pass / city), all data-driven with a
  full in-repo editor, playability BFS gates and golden hashes.
- **Fleet**: 8 blueprint-born vehicles across 3 eras and 3 nations, one RON source feeding
  hitbox/armor/visuals; a review workshop with measurable gates (Studio, dossiers, ratios);
  no clones. Finish is UNEVEN — the T-54 is the benchmark, others trail it.
- **Presentation**: wgpu renderer (cascaded shadows, SSAO, HDR+bloom, weather + timeline,
  water, grass, battle FX, fully procedural buildings/trees — no imported flora),
  procedural audio (DSP, speed-of-sound delay), garage with workshop UX, full battle HUD,
  frame-time p50/p95/p99 measurement backing the one-look budget.
- **Sim/net foundation**: deterministic fixed tick, authoritative headless server, protocol
  snapshots (**wire v45** — breaches v39, `ShotFired` as a replicated fact v41, cook-off
  staging v42, rack countdown v43, a third-party projectile's owner withheld from a viewer
  who has not spotted the shooter v44, the battle clock on the wire v45), replay regression,
  bots with routes/fire discipline, 7v7
  mode. Remote input
  has epoch-safe reconnect, lightweight ACKs, snapshot-aligned prediction replay, and a terminal
  gameplay gate that freezes prediction on outcome/timeout/stalled world state instead of
  permitting a zombie client. Personal hit, damage, absorbed-impact, and lethal truth has its own
  ordered retransmit/ACK lane with exactly-once presentation instead of depending on snapshot loss.

**PARTIAL** (works, known debt):
- World close-range quality and authoring power (`docs/world-2.0-program.md`): Cover 2.0,
  camera/horizon, District/Scatter 2.0 — flora is already procedural-only (F0 landed).
- Breach interiors: cross-frame remesh merged, but museum detail, interior variants and
  interior audio are open.
- Fleet finish: per-vehicle polish passes outstanding outside the benchmark vehicles.
- Map rotation & per-map bot battle tests (only Orliny has one).
- Human playtesting: the maps have never met a second human.

**MISSING toward release** (the honest gap list; dates and thresholds now live in
`docs/product-program.md`):
1. **Production networking hardening**: the dedicated UDP path, lossy lifecycle, epoch-safe
   reconnect and client prediction work today. Still missing are public-session
   discovery/relay, player authentication, beta-validated lag compensation, cheating posture
   and dedicated-server operations — the register and wave plan are in
   `docs/multiplayer-production-program.md`.
2. **Meta & matchmaking**: OpenSkill-based MM, sessions/lobbies, player identity, and a
   record that a battle happened at all (today the game keeps none). Progression is **proof,
   never power**: no XP, no credits, no research, and no module unlocks — modules carry real
   stat deltas, so gating them behind time would be power behind time, which the creed
   forbids. The design is `docs/product-program.md` §6.
3. **Content breadth**: more vehicles per era/nation (Era I is an empty bracket and Britain
   has one tank), 2-3 more maps, game modes beyond the single 7v7 skirmish.
4. **Product shell**: settings/keybinds UI, localization (PL/EN — the glyph atlas bakes ASCII
   only today), onboarding/tutorial, packaging/installer, crash reporting, store presence,
   trailers/devlogs, NAME of the game.
5. **Audio/presentation polish**: voice-over callouts, music, more FX variety.

## How work is organized

Programs of small PRs (1 branch = 1 PR), local `scripts/verify.ps1` as the only merge gate,
every feature landing with a locking test. Current battle work lives in
`docs/battle-first/program.md`; completed program docs stay as doctrine.

**`docs/product-program.md` is the plan above all of them** (approved 2026-08-04): it holds
the release ladder — a closed playtest, a demo at Next Fest, then Early Access — the three
named builds, the go-to-market track, the honest-retention design that replaces grind, and
the first numeric release criteria this project has written down. The other programs keep
their own documents; that one sequences them against dates. The open picture work is
`docs/art-direction-program.md`, `docs/world-scale-program.md` (the world reads too small —
a standing user verdict) and `docs/hala-2-program.md`, and it is now on the marketing
critical path, because a store page is made of frames.
