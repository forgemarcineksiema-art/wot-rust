# Procedural Geometry Kernel Program

This document is the live taxonomy for the renderer-neutral procedural geometry kernels that build
armored vehicles. It supersedes the spike table in
[hybrid-geometry-spike.md](hybrid-geometry-spike.md) as the source of truth for *which generator
builds which part, and why*.

**Starting point:** commit `2efa773` — the T-54 cast turret shell is a designed **cast loft**, not a
metaball `sdf_mesh`. SDF remains available for other cast work (fluid unions, sockets, organic local
transitions), but is no longer the production turret-shell technique.

## Core rule

A part's **physical construction selects its generator — not convenience.** Rolled plates are built
as exact plates; designed castings are built as designed lofted shells; round parts are revolved.
Shape generators are interchangeable behind one common audited mesh contract, and gameplay remains
independent of the rendering technology that consumes the baked result.

## Kernel selection matrix

| Part nature | Production kernel | Why |
|---|---|---|
| Rolled/welded armour, glacis, casemate plates | `solid` + future `panel` | Exact plane normals, sharp seams, exact armour angle, low triangle cost |
| Designed cast shells: turrets, masks, rounded housings | `cast_loft` (renamed from `loft`) | Direct silhouette control at every station; avoids the "metaball lump" failure mode |
| Fluid unions, sockets, organic local transitions | `sdf` + `sdf_mesh` | CSG and smooth blending where a controlled station model would be awkward |
| Barrels, wheels, rollers, drums, sprockets | `revolve` | Exact axial symmetry and efficient radial resolution |
| Closed paths: tracks; later hoses, rails, welded bead paths | new `sweep` | A cross-section follows a stable path frame |
| Thin fabricated parts | new `panel` and `shell` | Thickness, hems, flanges, bends and hard seams without abusing cuboids |
| Bolts, weld seams, handles, casting marks | new `detail` and `scatter` | Semantic, deterministic decoration with LOD policy |
| Local visual asymmetry and wear | new bake-only `deform` | Controlled visual change that never changes collision or armour truth |
| Mesh boolean / subdivision | bake-only experimental CAD lane | Useful later, but never a mandatory runtime or mainstream vehicle dependency |

## Explicit exclusions for normal production work

- No universal "one kernel for every tank part."
- No runtime rebuild of complete vehicle geometry.
- No general-purpose scene graph, skeletal system, DCC dependency, or GPU dependency in
  geometry/Forge crates.
- No SDF meshing of armour plates merely to obtain one unified representation.
- No mesh boolean or subdivision dependency for T-54, early vehicle migrations, collision, hit
  detection, or the main Forge path.
- No visual deformation that changes hitboxes, armour facets, module locations, mount frames, or
  authoritative replay state.

## Naming distinction: generic loft vs cast loft

- **generic loft** (`vehicle_geometry::LoftSpec`) = arbitrary convex 2D sections along a cardinal
  axis, for fabrication and hull-like solids.
- **cast loft** (`cast_loft`) = superelliptic horizontal stations plus localized cast shaping, for
  designed castings such as turret shells and masks.

These are deliberately distinct so no author puts a cast turret in a generic convex-hull loft, nor
uses a cast-specific superellipse API for flat armour plates.

## T-54 "do not regress" reference

The T-54-3 obr. 1951 is the canonical quality benchmark. Every later kernel change must preserve:

- low, broad, front-heavy cast turret;
- narrow turret ring and visible low overhang;
- separate cupola;
- mask covering the gun embrasure;
- five large wheels per side;
- continuous track belt and distinct end hardware;
- sharp glacis and truthful armour normals.

The T-54 is the executable reference, not just "a model that happens to render": its topology,
silhouette, hitbox honesty, mount frames, LOD ladder and artifact determinism are all test-locked.
