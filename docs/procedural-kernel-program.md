# Procedural Geometry Kernel Program

This document is the live taxonomy for the renderer-neutral procedural geometry kernels that build
armored vehicles. It superseded the spike table of `docs/hybrid-geometry-spike.md` (spike
concluded; the file now lives in git history) as the source of truth for *which generator
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
| Rolled/welded armour, glacis, casemate plates | `solid` | Exact plane normals, sharp seams, exact armour angle, low triangle cost |
| Designed cast shells: turrets, masks, rounded housings | `cast_loft` (renamed from `loft`) | Direct silhouette control at every station; avoids the "metaball lump" failure mode |
| Fluid unions, sockets, organic local transitions | `sdf` + `sdf_mesh` — **aspirational, no production part routes through them**: `GeneratorKind::Sdf` has zero construction sites and `PartShape::Cast` appears only at its declaration (`vehicle_build/src/part.rs:128`) and its match arm (`:152`) | CSG and smooth blending where a controlled station model would be awkward — but a capability with no proven call site is the same debt the panel/shell note below names; the next cast part either proves this row or retires it |
| Barrels, wheels, rollers, drums, sprockets | `revolve` | Exact axial symmetry and efficient radial resolution |
| Closed paths: tracks; later hoses, rails, welded bead paths | new `sweep` | A cross-section follows a stable path frame |
| Thin fabricated parts: folded pressings, hems | `panel` — back in production; ~~`shell`~~ deleted 2026-08-02 | `panel` returned the same day it was deleted, WITH the part that needed it (`073dfe1`): the T-54 fender folds plate, chamfer and return lip as one pressing with a `Hem` (`vehicle_build/src/t54_fender.rs:19`). `shell` stays in git history. See the note below. |
| Bolts, weld seams, handles, casting marks | new `detail` and `scatter` | Semantic, deterministic decoration with LOD policy |
| Local visual asymmetry and wear | new bake-only `deform` | Controlled visual change that never changes collision or armour truth |
| ~~Mesh boolean / subdivision~~ | ~~bake-only experimental CAD lane~~ — DELETED 2026-08-02 | The `experimental_geometry` slot held no capability, only the trial rule now kept below. |

### Three kernels deleted 2026-08-02 — and one came back the same day, with its part

`panel`, `shell` and `experimental_geometry` were deleted — 753 lines, three crates that compiled
on every build, linted on every gate and answered to nobody.

`panel` and `shell` were not failures: both were finished, both were contract-tested (`shell` even
carried the rule that matters — the outer visible surface is preserved EXACTLY, so thickening can
never move the surface the combat model reads), and both were built for parts nobody had yet
authored. That is the whole lesson of the row above: a capability built before the part that needs
it is a capability nobody has proven at the call site, and it ages as debt rather than as an asset.
`crate_hygiene`'s orphan allowlist recorded them as debt precisely so this decision could be taken
deliberately instead of by neglect; the allowlist is down to `quality` alone, which is the gate
itself.

The deletion note said: if thin fabricated parts come back — basket sheets, splash guards, fender
lips — they come back with the part that needs them. That prediction cashed in the same day:
commit `073dfe1` rebuilt the T-54 fender as the folded pressing it is, and `panel` returned as its
generator — 261 src + 147 test LOC, a workspace dependency again, called at
`vehicle_build/src/t54_fender.rs:19`. The lesson stands *because* of the return, not despite it:
the crate that waited was debt; the same crate summoned by a real part is an asset with a proven
call site — and it re-entered through a consumer, never through the orphan allowlist. `shell` and
`experimental_geometry` stay gone; if shells come back, they come back the same way.

**The trial rule `experimental_geometry` used to hold, kept because it is still true:** a backend
lives in a trial only while it is being tried. Declaring an optional dependency that does not build
breaks `--all-features` for the whole workspace — third-party compile errors in a standard
invocation, on code nobody asked for. Open a trial by adding the `[features]` entry and the
`optional = true` dependency together; close it by taking both out again, whichever way the trial
went.

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
