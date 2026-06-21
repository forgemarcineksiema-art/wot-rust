# Hybrid Vehicle Geometry — Spike And Decision

Status: spike landed 2026-06-14, all gates green (`./scripts/verify.ps1`). The T-54 hybrid is the
production-selected source for Forge artifacts and client fallback; other vehicles remain on legacy
recipes until their own Forge migration.

**Post-loft baseline (commit `2efa773`).** The T-54 cast turret shell no longer uses the metaball
`sdf_mesh` generator: it is now a designed **cast loft** (`vehicle_build::t54_turret_loft` over the
`loft` kernel). This commit is the starting point of the procedural kernel program
([procedural-kernel-program.md](procedural-kernel-program.md)). SDF and Surface Nets remain available
for other cast work — fluid unions, sockets, organic local transitions — but are no longer the
production turret-shell technique. The table below records the original spike decision; the kernel
selection matrix in the program document supersedes it as the live taxonomy.

## Why

The original procedural kernel (`vehicle_geometry`) builds vehicles from convex extrude/loft sweeps
under a ~3,600-triangle budget. The result reads as a proportionally-correct **greybox** — painted-on
disc wheels, a faceted blob turret — far below the target look (beta-era World of Tanks). The ceiling
is geometry and budget, not shading: lighting/materials cannot rescue geometry this coarse.

Constraints (owner): zero artists; sculpt only in an in-engine editor (not a DCC); determinism may
relax; visual modularity goes far; tanks must look like tanks with **armour angles that match what
is rendered**.

## Decision: one technique per part nature, behind one description

A tank physically *is* a mix of flat rolled/welded plates, cast castings, and round running gear.
No single representation does all three well, so the geometry is a **small library of generators**,
each chosen for the part it builds, all emitting `vehicle_geometry::GeometryMesh`:

| Part nature | Generator | Crate | Why |
|-------------|-----------|-------|-----|
| Flat armour plates (glacis, hull) | exact convex solid from half-spaces | `solid` | razor-sharp edges, exact slope, ~tens of tris |
| Cast castings (turret, mantlet) | SDF + Surface Nets meshing | `sdf`, `sdf_mesh` | smooth organic cast; budget by grid resolution |
| Round parts (barrel, road wheels) | surface of revolution + repetition | `revolve` | clean rims at low cost |
| Track belt | rectangular cross-section swept along a closed loop | `revolve` (`track`) | continuous band wrapping the wheels |

The spike measured the two hard cases on the T-54:
- **Cast turret:** SDF + Surface Nets reads as real cast steel, compresses gracefully to ~9k tris.
- **Glacis plate:** convex CAD is **16 triangles with razor-sharp edges**; the SDF glacis needed
  3,864 triangles and still stair-stepped its edges. CAD wins decisively for flat plates.

So the kernel question resolved to a **hybrid**, not a single kernel — and not 2–3 *independent*
kernels either: one `solid` + one `sdf` + one `revolve`, unified by the description layer below.

## The spine: `vehicle_build`

`vehicle_build` is the parametric description layer. A vehicle is a list of `VehiclePart`s, each with
a `PartShape` (`Plates` → CAD, `Cast` → SDF, `Mesh` → any prebuilt generator output). `build()`
routes every part to its generator, merges by submesh kind (hull/turret/gun), and returns one
`BakedVehicle` — which flows straight through the existing `reduce_vehicle` LOD path and the Forge.

**Gameplay coherence is by construction:** the glacis plate normal is built from the same slope the
armour facet reads, locked by a test (`glacis_geometry_slope_matches_the_armour_blueprint`). The gun
barrel length comes from the installed `GunModule`, not a post-bake scale of a fixed mesh — replacing
the `barrel_scale` hack.

## Result

Hybrid T-54: CAD hull plates + revolved 5-wheel train + swept track belt + SDF cast turret + revolved
barrel = ~10.8k tris (LOD0), reducing to ~1.3k (LOD1) / ~0.5k (LOD2) through the existing pipeline.
Renders (renderer-free CPU raster, z-buffered) live in `target/spike_sdf/`.

## Open / next

- Separate, animated running-gear submesh + damage states tied to `ModuleSlot::Suspension`.
- Adaptive (octree) dual contouring *only where it shows* — sharp plates meshed from SDF — if any
  cast/plate boundary ever needs it (the spike showed uniform Surface Nets is otherwise enough).
- Real blueprint proportions across the full hull plate set (the spike hull is a single clipped box).
## Resolved

- **Sloped hull sides/rear (2026-06-14).** The hull is now a convex solid whose side (hull_side 10°)
  and rear (hull_rear 5°) plate normals are sloped to the blueprint angles in the armour convention,
  alongside the glacis. The `every_hull_facet_carries_its_blueprint_armour_angle` test recovers each
  facet's angle from the built mesh normals and asserts it equals the blueprint — so "what you see is
  what you shoot" holds on every hull facet, not just the glacis. (Pure visual coherence; the armour
  model was authoritative and unchanged.)

- **Glacis angle convention (2026-06-14).** `game_core::armor` computes effective = nominal /
  cos(impact + `slope_degrees`), i.e. `slope_degrees` is the angle of the plate **normal from
  horizontal** (the shell direction). The CAD/SDF glacis originally used `(0, cos, sin)` — a 30°
  normal — so the *visible* rake disagreed with the 60° the penetration model used. Fixed to
  `(0, sin, cos)` (a true 60° normal); the coherence test now recovers `atan2(n.y, n.z)` from the
  built mesh and asserts it equals the blueprint facet slope. The armour model was authoritative and
  untouched, so this changed only the visible geometry — no balance impact.
