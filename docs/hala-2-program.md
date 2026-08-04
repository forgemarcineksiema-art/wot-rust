# Hala 2.0 — the garage stops being a blockout

Approved 2026-08-04, after a screenshot review whose verdict was one sentence: *"wszystko tak
prymitywnie, pusto, bez szczegółów"*. The diagnosis had arithmetic behind it: the T-54 carries
~15,000 triangles of construction logic through the model-logic bar, and stood in a hall built
from 1,551 lines of flat boxes with no UVs, no materials, no indirect light and about a dozen
props for a 36 × 36 × 12.6 m volume. A museum-grade tank parked in a greybox.

Five named causes, in order of what each impression came from:

1. **Blockout geometry** — walls as bare sheets, a railing of toothpicks, a crane that is an
   empty frame, crates without hardware, a fire extinguisher that is a red capsule.
2. **No materials** — one flat albedo per surface; the floor has no aggregate, joints, tire
   scuffs; the battlefield's Materia Świata lanes (UV, gloss, surface role) reach the hangar
   not at all.
3. **No indirect light** — the engine's one genuine interior ceiling: no bounce, shipped bloom
   at 0 mips so emitters cannot glow, no reflections. D20 recorded the result (0.3% bright).
4. **No reflections** — a showroom floor that mirrors nothing.
5. **Density and story** — ~13 props, a black ceiling void, no zones that tell the work.

The investment split follows the diagnosis: **~40% engine (interior-only gaps), ~60% content
pipeline** — a renderer-only spend would produce a beautifully lit blockout.

## Decisions (user, 2026-08-04 — binding)

- **Props are procedural kernels** through the model-logic bar, not CC0 imports (CC0 stays a
  filler-only escape valve). Identity and compounding over speed.
- **The garage may render "a bit richer" than the battle.** Reading of the one-look policy:
  it forbids player-facing options and battle inequality, not differences between scenes with
  no fairness surface. "A bit" is the taste bar — subtle, not a fairground.
- **The program runs now**, before the return to the networking track.

## Phases

| Phase | Scope | Buys | Status |
|---|---|---|---|
| **T1 Interior light** (engine) | Per-vertex GI bake into the appended `bounce` lane (build-time, deterministic); emissive panes + garage-only bloom chain; planar floor reflection; god-ray dust shafts under the skylights; lamps already paired with pools | Closes D20 by mechanism: windows become windows, walls catch their spill, the floor mirrors the hero | **T1a landed** (bake + emission + bloom); T1b reflection, T1c dust shafts open |
| **T2 Interior materials** (pipeline) | Trim-sheet + decal lane for architecture (corrugation, grating, hazard stripes, stencils); floor material (aggregate, joints, scuffs, stains, roughness variation) | Walls and floor stop being sheets of colour; feeds Ostrogorsk | open |
| **T3 Workshop kernels** (content) | ~20 props through the model-logic bar: angle-iron shelving, drums with hoops and bungs, gas bottles in cages, hose reel, cable drum, bench vice, tool wall, **crane with trolley, hook and ropes**, pallets, jerrycans, lockers, welding cart | Density ×10 in the fleet's own style; props reusable across maps | open |
| **T4 Hall architecture** | Portal-frame trusses from I-beams (sweep kernel), framed skylights with glass, segmented roller gate with chain box, mezzanine with grating and stairs, fans/ducting | The ceiling stops being a void; "bijatyka w blasze" gets a house of blacha | open |
| **T5 Composition** | Zones: ammo store behind mesh, fuel corner, an engine on a stand (engines exist as gameplay modules), gate numbering, original safety posters | The hall tells the work, not just stores the tank | open |

Estimated ~20–27 PRs across the phases — the scale of Genialna Flota or Inna Liga.

## Deliberately not bought

- **A commercial engine** — it would burn the moat (18 ratchet gates, golden determinism, the
  honesty doctrine, procedural identity) for generic technology; the gaps are interior-scoped
  and addressable in-engine.
- **Realtime GI / RT / a deferred rewrite** — wrong hardware target (MX330 @ 60), wrong
  identity; a build-time bake gives a static room ~90% of the effect at ~1% of the cost.
- **Photoreal textures on vehicles** — the procedural-style commitment stands.

## T1a — what landed and how it is locked

- `SceneVertex` grew a `bounce` lane by APPEND (18 floats, 72 B; offsets pinned in
  `scene_vertex_lanes.rs`). Zeros everywhere → every existing mesh renders identically; the
  battlefield goldens moved by nothing.
- `hangar_bake.rs`: conformal longest-edge subdivision (panels only — a lath earns no light
  gradient, and midpoints on rails broke the vertex-proxy clearance test), a median-split BVH,
  a fixed cosine hemisphere of 16 rays per vertex; sun through the REAL skylight openings,
  worklamp pools, emitters at authored radiance. Deterministic; cached once per process.
- The frosted panes became true emitters (channel > 1.0 — the hall's existing hot-face
  convention) and the garage runs a 3-mip bloom chain (`hangar_bloom_mips`), set per scene and
  restored to the tier default in battle. The review harness sets the same number — the locked
  picture is the played picture.
- Locks: bake determinism, finite/bounded lane, spill-where-the-room-emits, subdivision
  budget, pane placement, and the re-recorded garage goldens.

## Measurement discipline

Every garage-only feature still lands with a number (one-look: budgets per item). The garage
scene is small — one vehicle, no sim — so the headroom is real, but it is claimed by
measurement, not by assumption; the min-spec proxy is the user's own laptop.
