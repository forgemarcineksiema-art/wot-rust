# Jedna Trawa — the grass program

Thesis: **one tuft population, three costumes of the same tuft by distance. Grass never
disappears — it simplifies, and its final costume is the ground itself wearing the meadow's
tone.** A seam is invisible exactly when the same tuft stands on both sides of it: same root,
same height, same species, same tone.

Approved 2026-08-05 from a live-frame audit (sniper scope, Prokhorovka): the game holds **two
unrelated grasses** — the near blade ring (`crates/world/scene_build/src/grass.rs`, procedural
tufts to 48 m) and the card meadow (`grass_cards.rs`, solid-trapezoid "tents" to 330 m) — with
different seeds, different geometry, and far cards ~2× TALLER (0.55–0.85 m) than the near
blades they claim to continue (0.16–0.45 m). The tents read as alien objects, the hand-off
reads as a ring, and past 330 m the meadow vanishes because bare splat ground does not look
like grass. The two systems already share the where-rules (clump centres, baldness, splat
acceptance in `crate::grass`) — this program unifies the WHAT.

## Decisions (user, 2026-08-05)

- **D1 — height cap 0.6 m**: grass never visually hides a tank. There is no camouflage
  mechanic, so visual concealment would lie about gameplay. Locked by test.
- **D2 — shadow doctrine amended**: NEAR tufts (costume A) cast into cascade 0 only, behind a
  measurement. `docs/shadow-policy.md` gets the carve-out; the compile-time shadowless lock
  (`grass.rs:18`) is rewritten to say what is now true. Far costumes stay shadowless.
- **D3 — zoom-aware bands**: sniper magnification multiplies the band thresholds; the scope is
  a combat view, not a debug view. One multiplier, no quality option (one-look intact).
- **D4 — tank-in-grass is IN the program** (P9): tracks and hull press the meadow down;
  today's `bounce` lane is GI radiance, not interaction — this needs its own mechanism.

## Architecture — the three costumes

- **Costume A — full tuft (0 → ~35–40 m)**: blade kernel 2.0 — two-segment arced blades,
  tapered to a POINT (the serrated silhouette falls out of pointed tips at varied heights);
  species-shaped; realistic sizes (carpet 8–18 cm, meadow tufts 25–45 cm, seed-head accents
  45–60 cm, hard cap 0.6 m); plus a cheap low CARPET layer (≤ ~18 m) between tufts — density
  comes from the carpet, not from multiplying tall tufts.
- **Costume B — far tuft (~30 → ~200 m, baked static dressing)**: the tents die. Same
  delivery mechanism (whole-map bake, chunked, color-pass only, `dressing.rs`), new geometry:
  two crossed quads whose top edge is a serrated tuft silhouette. **Same population as A** —
  costume B takes the first N candidates of the exact hash stream A grows, so the tuft you saw
  up close stands in the same spot with the same height, species and tone when you drive away.
  Only TALL species get a costume B (carpet is sub-pixel past 40 m; its far costume is C).
  A↔B hand-off: per-tuft hashed personal radius + crossfade — no ring line exists.
- **Costume C — the ground wearing the meadow (~200 m → horizon, and under A/B)**: baked
  meadow-AO darkening where the population is dense (density field is CPU-deterministic — bake
  once, zero runtime cost) + a meadow tone treatment in the terrain shader statistically
  matched to costume B's aggregate, so B's collapse is a true dissolve. Honesty note:
  "never disappears" is engineered as "no transition is visible", not polygons to 1000 m.

Rejected consciously: GPU-side generation from instance id (population must stay CPU-side —
that is where the locks live), camera-facing billboards and alpha blending (world-anchored
doctrine, no sorting), grass as gameplay (scenery never blocks gameplay; hence D1).

## Wind 2.0

Today: two global sine waves, everything gusts in unison, tips slide sideways. Plan: a 1-D
gust-front field advected along the wind direction (visible waves rolling across the field —
one `value_noise` sample in VS); per-tuft phase from the root hash; stiffness per species
(seed heads sway deep and slow, carpet barely moves); arc-true bend (tip drop ∝ deflection²,
the tuft LIES DOWN instead of sliding); micro-flutter on A's tips only. `WindState`
(direction, base strength, gust envelope) becomes one uniform truth — stretch goal: the audio
wind reads the same envelope, so the gust you hear is the gust that flattens the field.

## Perf ledger (one-look policy)

The live frame that opened this program showed ~21 ms (~51 FPS) — already under budget, so
the program's NET target is: **no more expensive than today, ideally cheaper.** Gains to
spend: the tent meadow dies (up to 90k cards × 8 tris + ~50 MB static VRAM), geometry horizon
330 → ~200 m. Costs to pay: kernel 2.0 (~2× tris in A), the carpet, serrated tops in B,
shadows S2/S3. Every step lands with a before/after from `detail_cost_probe` +
`perf_capture` on this machine; numbers choose blade counts, segment counts and band radii.

Shadow ladder: S1 free (meadow-AO bake + deeper base→tip gradient) → S2 A-casts-into-cascade-0
(measured; doctrine change D2) → S3 A in the SSAO prepass ≤ ~20 m (measured). Fallback if
S2/S3 blow the budget: S1 + analytic anti-sun darkening of blades.

## The PR ladder

| PR | What lands | Key locks |
|----|-----------|-----------|
| P0 | Baseline measurements + interleaved A/B instrument in `perf_capture` (full / no card meadow / no near ring rotate inside ONE process — sequential processes measure this laptop's thermal ramp, not the scene) | numbers in STATUS |
| P1 | Blade kernel 2.0: arc, taper, pointed tips, real sizes, cap 0.6 m | tip sharpness; cap; size distributions |
| P2 | Species in A (one kernel, parameter sets; field-quilt + hash selection) | determinism; mirror-fair; per-map mix |
| P3 | **Death of the tents**: costume B baked from A's candidate stream, serrated tops, shared heights, horizon by measurement | population unification (B ⊂ A stream); silhouette serration; height continuity |
| P4 | Invisible seam: per-tuft dither radii, A↔B crossfade, zoom-aware bands (D3) | stand_A + stand_B ≈ 1 across bands; zoom multiplier |
| P5 | Costume C: meadow-AO bake + terrain meadow tone | B-aggregate ↔ C tone Δ < threshold |
| P6 | Carpet layer + near densification (paid from P3's gains) | budget sweep (existing pattern) |
| P7 | Wind 2.0: gust fronts, arc bend, stiffness, flutter, `WindState` uniform | roots planted; dy ∝ dx²; world-anchored gusts |
| P8 | Shadows S2 (+S3 if the number allows); shadow-policy carve-out (D2) | measured budget; doctrine doc updated |
| P9 | Tank-in-grass: hull/track press-down (D4) | press is local, deterministic, recovers |
| P10 | (stretch) audio-visual wind: one gust envelope for ear and field | shared envelope |

Golden probes (`bystra_views`, `orliny_views`, `flora_probe`, look metrics) are re-blessed
deliberately after each visual step. Existing `grass_cards.rs` locks migrate to the new
generator — locks travel, they do not die.

## STATUS

- 2026-08-05 — program approved (D1–D4).
- 2026-08-05 — **P0 measured** (dev box = min-spec, offscreen 1920×1080, Bystra walk,
  4 interleaved cycles): full-scene work **~16.4 ms p50** (25.28 ms incl. the 8.93 ms readback
  fence) — the 60 FPS budget is already spoken for; every addition must be paid.
  **Card meadow GPU cost ≈ 0** (Δ +0.52 ms p50 when REMOVED — inside the ±0.5 ms noise floor):
  the tents' death buys ~50 MB VRAM and honesty, not frame time — the program's spend money
  must come from its own cuts (geometry horizon, per-band density), not from this removal.
  **Near ring: 0.83 ms GPU + 0.71 ms CPU conjure** (3170 instances) — kernel 2.0's ~2×
  triangle plan costs ≈ +0.8 ms GPU worst case; the carpet pays in BOTH lanes.
  Method note: sequential probe processes were discredited live (thermal ramp read REMOVING
  work as +2.6 ms) — the in-process rotation instrument replaced them.
- 2026-08-05 — **P1 landed**: blade kernel 2.0 — 10 blades × 2 segments per tuft, POINTED
  tips (the serration), convex outward arc (mid station at 35 % reach), height spread
  0.14–0.34 mesh-local, wind lane quadratic along the blade (arc-true bend groundwork for
  P7), `GRASS_HEIGHT_CAP_M = 0.6` + scale consts with the D1 lock. Cost: 50 verts / 60 tris
  per tuft vs the old 48 / 48 — the interleaved capture reads the ring delta at +0.02 ms p50
  (inside noise); no measurable regression. Locks rewritten: pointed/arced/serrated/capped,
  wind-lane monotonicity; tuft budget raised 150 → 180 indices with the P0 number cited.
