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

## W1.0 — What playing the game revealed (highest priority)

From [`playtest-2026-08-01.md`](playtest-2026-08-01.md). Three of these are invisible in the source.

**a. Diagnose the zero-ricochet result.** 53 hits, 53 penetrations, no bounces, across a full
7-minute battle. Angling, hull-down and aim-point choice are dead mechanics until this is understood.
Separate the hypotheses first (bot engagement geometry vs fleet penetration vs the 70° hard
threshold) — do not tune blind.

**b. Battle pace.** 7 of 14 destroyed inside a 7-minute limit means every battle ends on the clock.
No balance judgement is possible until a 7v7 resolves inside its own timer.

**c. Bot aggression.** Thirteen bots, sixty seconds, zero kills except the player. Movement and
gunnery are already good; the decision layer is `bot_nearest_engageable_enemy` and nothing else.

**d. Fix the camera freezing on player death.** `tick_death_spectate` exists but does not drive this
path.

---

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

**2.1 Terrain resolution — the largest single lever in the project.** `cell_m: 5.0` on every map
means the tank is one cell and no terrain feature is smaller than a tank. Go to 2.5 m, then 1.25 m.
**Prototype on one map first** and measure before committing the fleet: 16× the samples touches
meshing, collision, authoring, goldens and every map at once, against a 233 ms scene bake.

**2.2 Armor depth** — normalization as a function of caliber against thickness; damage that depends
on what was hit.

**2.3 Author the ammunition** — the derived ×1.20/×1.25/×0.85 rounds break the doctrine the module
catalog explicitly upholds.

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
2. **W1.0a** — the zero-ricochet diagnosis. The product's identity is not firing; nothing else
   matters as much.
3. **W1.0b/c** — battle pace and bot aggression.
4. **W1.5** — frame-time measurement. Blocking for W2.1.
5. **W1.6** — the cheap visual fixes.
6. Everything else in the order written.

The first three answer one question: **does a battle in this game do what the design says it does?**
The measured answer today is no — and it is not a structural problem. It is a gameplay one, invisible
in the source and visible in the first minute of play.
