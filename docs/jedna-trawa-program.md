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
| P4a | Invisible seam: the hand-off radius is a world-anchored COASTLINE (noise-undulated), one WGSL function serves both costumes — near takes `stand`, far takes the complement, sum ≡ 1 by construction | shader-text lock on the shared function + both call sites; coastline reach < 48 m ring contract |
| P4b | Zoom-aware bands (D3): magnification derived from `P[1][1]`, scales the far collapse + dressing cutoff (near ring's CPU cache cannot scale — instance count grows with zoom²) | shader constants PARSED and matched to the CPU's; near ring must not stretch; scope measured in the rotation |
| P5 | Costume C: the ground carries the meadow's own darkness, taking over the far tufts' share on the SAME curve they fold on (`meadow_common.wgsl`, composed into both passes) | one shared fragment, two consumers; take-over linear in far presence; bare ground untouched |
| P6 | Carpet layer + near densification (paid from P3's gains) | budget sweep (existing pattern) |
| P7 | Wind 2.0: gust FRONTS advected along the sky's own storm heading, arc bend (drop ∝ deflection²), per-species stiffness, per-tuft flutter | one wind for sky and field; gusts stretched across the wind; a calm day still breathes |
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
- 2026-08-05 — **P3 landed (pulled ahead of P2)**: the tents are dead. Costume B is baked
  from the near ring's OWN candidate stream (`CellStream` + `MeadowGround::tuft_ground`
  shared; one seed, one acceptance) as its first-N standing prefix — a far card can only
  stand where a near tuft stands, with its tallest tooth EXACTLY the near tuft's tallest
  blade at that scale (`TUFT_MESH_TALLEST_M` × size, one-number height continuity; the old
  tents were fixed 0.55–0.85 m, TALLER than the blades they claimed to continue). Geometry:
  two crossed serrated planes, peak–valley–peak, 10 verts / 12 tris (was 8 / 8). The near
  ring itself now folds the mirror like the meadow always did — the whole population is
  mirror-fair from one south-half stream. Locks: unification (far ⊂ near, bit-tolerant
  roots + height), serration (no flat tent tops), ring mirror-fairness, sway=height×0.3
  collapse contract; the count band and D19/crater/road locks migrated. Measured: costume B
  Δ −0.07 ms p50 (noise — still GPU-free), conjure 672 µs (no fold regression), full-scene
  work ~16.7 ms p50. P2 (species) now flows into BOTH costumes through the shared stream.
- 2026-08-05 — **P2 landed**: four species, ONE kernel (`SpeciesParams` over the P1 blade
  builder + a stalk-and-spikelet station run for the accents): Meadow, Carpet (short wide
  turf, no far costume — its far costume is the ground), TallSeed (sparse ~8 % accents,
  stiffness 1.35 — the wind seller), DrySteppe (stiff splayed straw on dry plots). The
  species lane sits LAST in the stream, so P3's positions did not re-roll; `species_at`
  reads a world-anchored plot field at the FOLDED z — twins grow the same grass, and
  costume B inherits species height + tone through the shared stream automatically. Rule 2
  made structural: `species_tinted_albedo` lands over-saturated straw EXACTLY on the 0.449
  cap (found live: raw straw shift × Prokhorovka's warm plots hit 0.56). Locks: per-species
  pointed/arced/serrated/capped + `tallest_mesh_m` is the real apex, wind lane monotone per
  station run (elevated spikelet seats ride their carrier), species proportions + twin
  species equality, muted-ground rule, unification lock now species-aware. Measured: scene
  work ~16.9 ms p50 (no regression; this series' noise ±1.4 ms), conjure 772 µs (within
  the historical 647–848 spread).
- 2026-08-05 — **P4a landed**: the seam is a coastline. `grass_handoff_stand` in
  `scene.wgsl` is the ONE function both costumes read — the near ring folds by its value,
  the far meadow stands by its complement (sum ≡ 1 by construction), and the radius
  undulates ±5.5 m on world-anchored noise (~14 m features), so no ring line exists to
  see. Reach tops at 47 m, inside the 48 m shader-ring contract of the anti-streaming
  lock. Locks: shader-text contract (function + both call sites + constants), naga
  validation. Cost: one `value_noise` per grass vertex in VS — deltas in the noise floor.
  D3 (zoom) split to P4b: the far collapse + dressing cutoff can scale with magnification,
  the near ring's CPU cache cannot (instance count grows with zoom²).
- 2026-08-05 — **P4b landed (D3 closed)**: the scope stretches the grass bands, derived from
  the PROJECTION rather than from camera state — `renderer_api::grass_zoom_band_scale`
  reads `P[1][1]` (which `draw.rs` already recovers every frame for SSAO), so the CPU chunk
  cutoff and the shader's collapse band read ONE number with nothing to synchronise, and
  every off-game camera (probe, garage, review) lands on exactly 1.0 by construction.
  Stretched: the far collapse (260→330 m × zoom) and `DRESSING_CUTOFF_M`. NOT stretched:
  the near hand-off — the ring is a CPU cache of fixed world radius whose instance count
  grows with the square of any stretch. Cap 4.0 (the 3° step wants ~20×).
  **Measured, and it inverts the intuition**: the scope at 18° (3.29×) renders **6.48 ms
  p50 CHEAPER** than the wide battle view — a 18° frustum culls far more than the longer
  band adds. The cap is generous, not tight. Locks: `wgsl_layout` now PARSES the shader's
  constants (`wgsl_const`) instead of restating literals — the hand-off reach and both zoom
  constants are checked against the CPU's own values, and a lock forbids stretching the
  near ring. Probe: `perf_capture` rotates a 4th config (`scope 18deg`) so the scope is
  measured in the same thermal rotation as the battle view.
- 2026-08-05 — **P5 landed (costume C)**: the ground carries the meadow. New shared shader
  fragment `meadow_common.wgsl` composed into BOTH the scene pass (which draws the tufts)
  and the terrain pass (which draws what they stand on) — one definition of the zoom scale
  and of `meadow_far_stand`, so the two cannot drift into a visible horizon. The ground's
  albedo now takes a vegetation-weighted share of the meadow's own darkness: 5 % while far
  tufts still stand in front of it, 17 % once they have folded, interpolated on their own
  collapse curve. Roads, rock and the riverbed are untouched (splat-weighted), and because
  it rides a MIPMAPPED texture rather than a procedural octave, the far field gains no
  shimmer (rule 5's no-noise clause).
  **Method note**: the original plan (a baked meadow-density map) was dropped after reading
  the code — CPU `terrain::value_noise` (splitmix64) and the WGSL one (32-bit hash) are
  different functions, so the analytic route would have put the dark patches in the wrong
  places, and a new texture + binding was a system the effect did not need. The splat is
  already shared by both sides; that is the honest common ground. Measured: scene work
  ~16.5 ms p50, no regression. Locks: one-copy-per-pass of the shared fragment, terrain's
  call site, shade constants matched to the CPU's, take-over linear with no step.
- 2026-08-05 — **P7 landed (Wind 2.0)**: the field is laid by the wind the SKY already has —
  the storm front's heading (`cloud2_params.z`) that drives the cloud sheet, so there is no
  second wind to keep in sync and no new uniform. A gust is a FRONT, not a hum: one
  low-frequency sample advected ALONG the wind and stretched across it (~28 m along, ~85 m
  across), so waves visibly roll over the meadow. The bend is an ARC pinned at the root —
  the tip drops with the SQUARE of its deflection (the chord of a bent blade), so grass
  lies down instead of skating downwind. Per-species stiffness rides in free: the mesh's
  sway lane already carries P2's `sway_mult`, so carpet barely stirs while seed heads swing
  deep. Per-tuft flutter decorrelates neighbours. A calm day still breathes
  (`MEADOW_WIND_BASE` > 0, locked); a storm front adds on top.
  Cost: nil within noise (near-ring Δ −0.65 ms, unchanged from P5's −0.35).
  **Measurement lesson (worth more than the feature)**: the first P7 run read the ring at
  −4.35 ms and looked like a 4 ms regression. It was machine load — the same run showed the
  scope's p95 at 128 ms, and a re-run showed REMOVING the card meadow "costing" +4.86 ms,
  which is physically impossible. Even the in-process rotation instrument lies when the box
  is busy: **read p95/p99 first as the series' credibility, and only then trust its p50
  deltas**. (The phase hash was simplified from a second lattice sample to a trig hash while
  chasing this — kept, because it is cheaper for an effect that only needs decorrelation.)
