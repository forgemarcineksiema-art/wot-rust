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

**DONE and test-locked** (each with its doctrine doc and regression suite):
- **Combat**: 3D armor volumes from blueprints, ricochet/normalization, spaced armor & HEAT
  honesty, swept-shell kinetics (v24), HE splash, module/crew damage, fire feel + feedback.
- **Movement**: planar rigid-body hull (velocity + yaw inertia, drift), per-wheel suspension
  with sprung attitude, hull-down that actually works, track damage in two tiers.
- **Destruction (Honest Steel)**: buildings→rubble, breachable walls, crushable fences and
  tree lines, terrain craters, wall scars, interiors behind breaches — replicated, honest.
- **Spotting**: per-era optics, radio-dead isolation, LOS through real cover, minimap +
  spotted gates, fog-fairness lock across all weather looks.
- **Maps**: 4 shipped (steppe / river town / mountain pass / city), all data-driven with a
  full in-repo editor, playability BFS gates and golden hashes.
- **Fleet**: 8 blueprint-born vehicles across 3 eras and 3 nations; museum-grade review
  workshop (Studio, dossiers, ratio/dimension gates); no clones.
- **Presentation**: wgpu renderer (cascaded shadows, SSAO, HDR+bloom, weather + timeline,
  water, grass, battle FX, procedural buildings/trees + CC0 textured-flora pipeline),
  procedural audio (DSP, speed-of-sound delay), garage with workshop UX, full battle HUD.
- **Sim/net foundation**: deterministic fixed tick, authoritative headless server, protocol
  snapshots (v35+), replay regression, bots with routes/fire discipline, 7v7 mode.

**PARTIAL** (works, known debt):
- Flora on maps (pipeline done; scatters/retrofit = FL-5), foliage sourcing (bush, birch),
  alpha-preserving mips.
- Map rotation & per-map bot battle tests (only Orliny has one).
- Human playtesting: the maps have never met a second human.

**MISSING toward release** (the honest gap list — the master plan's waves W1-W5):
1. **Real networking hardening (W1)**: internet transport (today: local/loopback authority),
   connection lifecycle, lag compensation policy, cheating posture, dedicated server ops.
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
`docs/urban-map-program.md`; completed program docs stay as doctrine. Master plan:
**W0 foundations → W1 network → W2 content → W3 meta → W4 new content → W5 release** —
W0-level foundations are effectively done; the center of gravity should now shift toward
W1 (network) and W3 (meta), which no amount of map polish substitutes for.
