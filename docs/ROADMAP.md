# State of the game & road to release

The whole picture — not the current sprint. Program docs (urban map, destruction, fleet…)
are execution details; THIS is what the game is and what it still owes the player.
Release shape: buy-to-play (~20-25 EUR), 7v7, three eras, skill matchmaking from day one.

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
  water, grass, battle FX, procedural buildings/trees + CC0 textured-flora pipeline),
  procedural audio (DSP, speed-of-sound delay), garage with workshop UX, full battle HUD,
  frame-time p50/p95/p99 measurement backing the one-look budget.
- **Sim/net foundation**: deterministic fixed tick, authoritative headless server, protocol
  snapshots (v43 — breaches v39, `ShotFired` as a replicated fact v41, cook-off staging v42,
  rack countdown on the wire v43), replay regression, bots with routes/fire discipline, 7v7
  mode. Remote input
  has epoch-safe reconnect, lightweight ACKs, snapshot-aligned prediction replay, and a terminal
  gameplay gate that freezes prediction on outcome/timeout/stalled world state instead of
  permitting a zombie client. Personal hit, damage, absorbed-impact, and lethal truth has its own
  ordered retransmit/ACK lane with exactly-once presentation instead of depending on snapshot loss.

**PARTIAL** (works, known debt):
- Flora sourcing (a replacement CC0 bush and textured birch). Alpha-preserving atlas mips,
  the accepted tree/pine pipeline and the four-map scatter retrofit are complete (FL-5).
- Breach interiors: cross-frame remesh merged, but museum detail, interior variants and
  interior audio are open.
- Fleet finish: per-vehicle polish passes outstanding outside the benchmark vehicles.
- Map rotation & per-map bot battle tests (only Orliny has one).
- Human playtesting: the maps have never met a second human.

**MISSING toward release** (the honest gap list — the master plan's waves W1-W5):
1. **Production networking hardening (W1)**: the dedicated UDP path, lossy lifecycle,
   epoch-safe reconnect and client prediction work today. Still missing are public-session
   discovery/relay, player authentication, beta-validated lag compensation, cheating posture
   and dedicated-server operations. A production program covering this list is pending as
   PR #431 (`docs/multiplayer-production-program.md`).
2. **Meta & matchmaking (W3)**: OpenSkill-based MM, sessions/lobbies, player identity,
   progression that respects the no-grind creed, garage economy (module unlocks without
   pay-to-win), stats.
3. **Content breadth (W2/W4)**: more vehicles per era/nation, 2-3 more maps, game modes
   beyond the single 7v7 skirmish.
4. **Product shell (W5)**: settings/keybinds UI, localization (PL/EN), onboarding/tutorial,
   packaging/installer, crash reporting, store presence, trailers/devlogs, NAME of the game.
5. **Audio/presentation polish**: voice-over callouts, music, more FX variety.

## How work is organized

Programs of small PRs (1 branch = 1 PR), local `scripts/verify.ps1` as the only merge gate,
every feature landing with a locking test. Current program status lives in
`docs/battle-first/program.md`; completed program docs stay as doctrine. Master plan:
**W0 foundations → W1 network → W2 content → W3 meta → W4 new content → W5 release** —
W0-level foundations are effectively done; the center of gravity should now shift toward
W1 (network) and W3 (meta), which no amount of map polish substitutes for. Two programs are
on pending PRs: **world scale** (#430, `docs/world-scale-program.md` — the world reads too
small, the standing user verdict) and **multiplayer production** (#431 — the road to Steam
multiplayer).
