# Vehicle Fidelity Masterplan ("Genialna Flota")

This document is the live plan for bringing every vehicle model to museum-grade quality
under a hard reference discipline: **data first, model second**. Every shape decision must
trace to a real tank — museum specimens, factory drawings, manuals, photographs — encoded
as machine-checked targets *before* anyone touches the blueprint. It extends
[vehicle-forge-policy.md](vehicle-forge-policy.md) and supersedes the fleet-status parts of
[vehicle-geometry-photo-analysis.md](vehicle-geometry-photo-analysis.md) (rewritten in
PR-05 of this program).

## Why

The 2026-07-17 fleet audit found five leagues of finish inside one roster:

1. **T-54 1951** — bespoke CAD+SDF hybrid, full interior, 18-part graph. The benchmark.
2. **IS-3 / Centurion** — blueprint recipes with bespoke 17-part tables and reference packs.
3. **German line (Tiger I, Tiger II, Jagdtiger, Panther II)** — solid geometry and the
   densest anatomy cages, but a coarse 7-part graph, one shared wide ratio gate
   (turret-height tolerance 0.25 catches nothing), and visible debt: interleaved wheels
   read as a dark slab, the Tiger II turret is a low-poly wedge, the Jagdtiger casemate
   has no encoded source proportions.
4. **T-34-85** — a 42-line Forge Studio demonstrator, not a model.
5. **T-55A** — a pipeline orphan and a T-54 clone. Removed entirely by this program
   (roster rule: **no clones**).

The workshop itself has gaps that make "data first" unenforceable today: only 5 silhouette
ratios exist, there is **no absolute-dimension gate** (ratios pass at the wrong scale), the
Studio report shows measured bounds with no target/Δ%/source columns, and armor thickness
lives in two hand-maintained copies with no equality lock.

## Standing invariants

- **One source of shape.** The RON blueprint feeds hitbox, mount frames, armor volumes and
  the visual mesh. "What you see is what you shoot" locks stay green through every PR.
- **Data before geometry.** A vehicle's dossier (real dimensions + sources) and its
  DimensionTargets/RatioTargets land in a PR *before* the PR that edits its RON. The
  target gate is written down before the shape moves.
- **No clones in the roster.** Variants that share a silhouette and role with an existing
  vehicle do not enter the fleet.
- **One look, measured budgets.** Min spec is MX330 @ 60 FPS with no quality options.
  Triangle budgets rise only per-vehicle, only with a `detail_cost_probe` measurement in
  the PR description, never as a fleet-wide envelope bump. LOD discipline
  (LOD1 ≤ 0.75·LOD0, LOD2 ≤ 0.45·LOD0) holds after every raise.
- **The model must make mechanical sense at garage distance.** Every hatch passes a human,
  every barrel has a bore, the engine breathes, tracks carry the wheels, nothing floats,
  and no two vehicles share a fitting the real vehicles did not share. The close-up
  functional review (docs/vehicle-model-logic-audit.md) IS the seal gate; number gates are
  the floor, never the bar. Declaring success from green gates alone is the failure mode
  this rule exists to kill.
- **Team tint is sacred.** Any material/palette work applies nation identity under the
  team tint, never at its expense; a ΔE contrast test locks today's readability floor.

## Program structure (~29 PRs, 5 waves)

```
W0  Foundations (sequential):  PR-00 this doc → PR-01 T-55A removal (+protocol v33) →
                               PR-02 thickness lock → PR-03 absolute-dimension gate →
                               PR-04 new ratios + per-vehicle packs → PR-05 docs v2
W1  German line:               Tiger I → Tiger II → Jagdtiger → Panther II   (3 PRs each)
W2  Bespoke polish:            IS-3 → Centurion → T-34-85                    (2–3 PRs each)
W3  T-54 reconciliation:       1–2 PRs (budget formalization, workshop parity)
W4  Fleet honesty (parallel with W2/W3):
                               DamageLayout from RON → cupola weakspot → nation palettes
```

Critical path: PR-01 → PR-03 → PR-04 → W1. PR-02/PR-05 and W4 hang loose.

**W0 status: COMPLETE (2026-07-17)** — PR #217 (this doc), #218 (T-55A out, protocol v33),
#219 (asset↔catalog parity lock; snapshots were months stale and got regenerated),
#220 (absolute-dimension gate; the T-54 pilot immediately caught the track width baking
+7 cm/side past the documented 3.27 m — TODO(W3-t54)), #221 (ratio family +3 kinds,
per-vehicle tolerances; caught the T-54 dome plan being wider than long), #222 (docs v2 +
dossier template). Next: W1 Tiger I dossier (PR-T1.1).

## The per-vehicle protocol

Every vehicle passes steps (a)–(h), packed into 2–3 PRs:

**PR-X.1 "Dossier and measure"** — zero geometry changes.
- (a) Research the real tank: museum specimens (Bovington, Kubinka, Munster, Patton
  Museum), factory drawings, manuals, model-kit cross-checks (the MiniArt pattern from
  T-54). Output: a dossier in `docs/vehicles/<x>.md` following the template
  (`docs/vehicles/_template.md`, PR-05) with an anchor-numbers table:
  dimension | value | source | confidence | DimensionTarget id.
- (b) DimensionTargets + tightened RatioTargets in the vehicle's pack, each with a
  ReferenceSource. Targets the current model *fails* enter with tolerance temporarily
  widened to current state plus `// TODO(W1-<x>): target ±…` — the PR stays green while
  the intended gate is already written. The discrepancy list goes in the PR description.

**PR-X.2 "Shape"**.
- (c) RON correction through the Studio loop (`tools studio --vehicle X
  --blueprint-file Y`), contact sheet compared against dossier photographs, iterate until
  Δ% sits inside the target tolerance. The RON also feeds hitbox and armor volumes —
  check `armor/vehicle_volumes.rs` behaviour after every major change.
- (d) Detail in the recipe (kernels: solid, cast_loft, revolve, sweep, panel, shell,
  deform, detail: weld_bead/bolt/scatter/handle_rail). A chamfer/fillet kernel is built
  only if two consecutive vehicles need it (separate tooling PR).
- (e) Bespoke part table in `vehicle_forge/src/part_data/<x>.rs` (IS-3/Centurion pattern),
  then tighten the temporary tolerances to their targets (delete the TODOs).

**PR-X.3 "Cage and seal"**.
- (f) Benchmark cage extended to Tiger-class density (17–28 anatomy asserts, each
  quoting the dossier's real dimensions in a comment).
- (g) GOLDEN_BAKE_HASHES re-record + LOD/budget gates
  (`vehicle_budgets.rs`, `vehicle_lod.rs`, `vehicle_dimensions.rs`).
- (h) Verification renders in the PR description: Studio contact sheet (deterministic CPU
  raster), a PBR studio render (`is3_studio.rs` pattern), and the fleet lineup.

### Budget procedure (raising is conscious, never drift)

1. Measure the candidate detail with `detail_cost_probe` (client examples) before deciding.
2. Exhaust the cheap paths first: running-gear instancing (interleaved wheels are more
   *instances*, not more unique triangles), detail kernels, harder LOD1/LOD2.
3. Only then raise the budget **per-vehicle** in `vehicle_budgets.rs` (precedent: T-54 at
   22k), with the probe number in the PR description and a row update in the status table.
4. LOD ratios keep holding after the raise.

## Wave notes

**W1 — German line** (order fixed): Tiger I first — its Schachtellaufwerk (three
interleaved wheel rows via instancing offsets) sets the pattern Tiger II / Jagdtiger /
Panther II reuse; Bovington 131 gives the strongest dossier in existence. Tiger II:
Henschel turret mass (cast_loft front curvature, weld_bead seams, Turmblende); Zimmerit is
explicitly out (geometry cost, note in dossier). Jagdtiger after Tiger II (shared hull =
cross-check): casemate proportions get their own ratio gates. Panther II last —
**decision locked: the museum specimen** (Panther II hull + Panther G turret, Patton
Museum / Fort Benning), the only photographable configuration.

**W2 — bespoke polish**: IS-3 (convert existing "Shape locks" into DimensionTargets,
verify the pike, handle_rail grab bars, DShK), Centurion (turret stowage bins, skirts,
searchlight), T-34-85 (full protocol: cast_loft turret, revolve fuel drums, rails).

**W3 — T-54 reconciliation**: formalize its budget exception with a min-spec probe, give
it full DimensionTargets, then a lineup check of detail parity against the post-W1/W2
fleet. If the fleet still looks uneven, the user decides (raise fleet vs trim T-54) from
lineup renders.

**W4 — fleet honesty** (parallel with W2/W3, depends only on PR-01):
- PR-F1: `internal_modules` section in the RON (Engine/FuelTank/AmmoRack/Crew OBBs) — the
  fourth thing the blueprint feeds; generic builder in `damage_layout.rs`; explicit
  heuristic fallback; fleet test: every playable vehicle has a non-empty layout inside its
  hitbox, and a rear-plate penetration reaches the engine.
- PR-F2: commander's cupola as a real armor volume in `vehicle_volumes.rs`, thickness in
  catalog AND JSON (the PR-02 lock guards the pair). Best after W1, when cupolas are modeled.
- PR-F3: `NationPalette` modulating only the base albedo of material roles inside a narrow
  envelope (4BO green / Dunkelgelb / bronze green); PBR parameters stay shared (one look).
  Team tint applies after the palette; a ΔE test locks today's team-contrast floor.

## Decision register

| Decision | Status |
| --- | --- |
| T-55A: full enum removal + protocol v33 | **DECIDED 2026-07-17** (user: no clones) |
| Triangle budgets: raise per-vehicle with measurement | **DECIDED 2026-07-17** |
| Wave order: German line first | **DECIDED 2026-07-17** |
| Panther II configuration: museum specimen (G turret) | **DECIDED 2026-07-17** |
| German palette era: Dunkelgelb (late-war fleet) | default; revisit at PR-F3 |
| Thickness single-source codegen (JSON→Rust) | optional follow-up after PR-02 |
| Chamfer/fillet kernel | only if two consecutive vehicles need it |
| T-54 vs fleet detail parity | decide after W2 from lineup renders |

## Fleet status

Updated as part of every PR's definition of done.

| Vehicle | Dossier | Targets | Shape | Detail | Part table | Cage | Sealed |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Tiger I | ✓ PR-T1.1 | ✓ anchors+ratios | ✓ width + Schachtellaufwerk | ✓ registry passes #233–#238 | ✓ bespoke 15 parts | dense | REOPENED items since fixed (bore, decks, tracks — registry clear #239); re-seal awaits user review |
| Tiger II | ✓ PR-T2.1 (#240) | ✓ 5 dims + 7 ratios tightened | ✓ Turmblende + Schürzen + bow flaps (#241) | ✓ fleet passes #233–#238 (front drive, bore+brake, bow set, cupola, Kgs/steel-dish, exhausts) | — (shares blueprint part path) | ✓ 7 tests / ~35 asserts incl. W1 dressing lock | close-up reviewed (deck/bow/profile/contact sheet); NOT declared — user review pending |
| Jagdtiger | ✓ PR-JT.1 (#244) | ✓ 5 dims + 7 ratios tightened | ✓ plain muzzle + cast collar + side racks + guards + Kugelblende (#245) | ✓ fleet passes #233–#238 | — (shares blueprint part path) | ✓ 8 tests incl. JT.2 dressing lock | close-up reviewed (flank/front/casemate); NOT declared — user review pending |
| Panther II | ✓ PR-PII.1 (#247) | ✓ 5 dims + 7 ratios (final after PII.2) | ✓ G turret + G-blende + 8.86 m (#248); bow pack: Kugelblende + periscopes + glacis Bosch + curved sweeps | ✓ fleet passes #233–#238 | — (shares blueprint part path) | ✓ 7 tests incl. G-plan + PII.3 dressing locks | close-up reviewed; NOT declared — user review pending |
| IS-3 | partial (`is-3.md`) | — | — | — | bespoke 17 | dense | — |
| Centurion | — | — | — | — | bespoke 17 | dense | — |
| T-34-85 | — | — | — | — | — | — | — |
| T-54 1951 | rich (`t-54.md`) | pilot (PR-03) | benchmark | benchmark | bespoke 18 | dense | budget exception |

## Verification

- Per PR: `scripts/verify.ps1` is the merge gate (CI is down). Geometry PRs additionally:
  fleet gates (budgets/LOD/dimensions) + the vehicle's cage; golden hash quoted in the
  commit body when re-recorded intentionally.
- Wire PRs (PR-01 only): protocol snapshots and replay fixtures regenerated, version bumped.
- Visual PRs: contact sheet + PBR render + lineup attached to the PR description.
- After every wave: full fleet lineup (scale and detail parity — the program's main risk
  is an uneven fleet mid-flight); after W1 a four-sheet comparison session against dossier
  photographs; after PR-F3 a two-team lineup on a min-spec proxy.
