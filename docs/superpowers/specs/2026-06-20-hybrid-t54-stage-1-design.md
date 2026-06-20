# Hybrid T-54 — Stage 1 Design

## Purpose

Make the hybrid T-54's moving gun assembly geometrically and transform-correct before it is
connected to the production Forge bake. The stage fixes authoring-space errors and mesh
degeneracy; it does not migrate ForgeArtifact or the client to the hybrid bake.

## Scope

- Author the barrel in the vehicle's shared local space from the authoritative gun trunnion and
  muzzle frames.
- Separate the fixed turret socket from the moving, wide T-54 mantlet.
- Put the moving mantlet and barrel in the `Gun` submesh so both follow gun elevation.
- Make surface-of-revolution caps use a single pole vertex and omit degenerate triangles.
- Add behavioral regression tests for mounting and generated-mesh validity.

## Non-goals

- Do not switch `ForgeArtifact`, artifact freshness checks, or the runtime fallback to
  `vehicle_build`.
- Do not redesign the complete hull, running gear, SDF turret silhouette, materials, or LOD
  algorithm.
- Do not alter simulation, ballistic, armour, networking, or mount-frame authority.

## Design

`revolve` gains an origin-aware profile builder. A zero-radius profile point emits one pole
vertex, while a nonzero-radius point emits one ring. Adjacent rows are stitched as either a
triangle fan (pole-to-ring) or quads split into triangles (ring-to-ring). This preserves caps
without allocating coincident vertices or producing degenerate triangles.

The T-54 barrel generator receives the gun trunnion and muzzle locations. It builds its axis
through the trunnion's Y coordinate and begins inside the moving mantlet near the trunnion Z
coordinate. The muzzle endpoint matches the authoritative muzzle frame. The stock gun's module
length remains a validated variant input, but it may not move the stock muzzle away from its
authoritative frame.

The turret SDF retains a fixed recessed socket in the cast front. The moving mantlet becomes an
oval mesh in `SubmeshKind::Gun`, placed on the trunnion frame; the barrel begins inside it. Thus
the hull/turret/gun pose chain remains hull origin -> turret ring -> trunnion -> muzzle.

## Acceptance checks

- A T-54 gun mesh has a centreline at `MountFrames::gun_trunnion.translation.y`.
- Its muzzle bounds reach the authoritative `MountFrames::muzzle.translation.z` within a small
  geometric tolerance.
- The moving mantlet is in the gun submesh and has a wide oval cross-section.
- Revolved capped geometry contains no triangles with repeated indices or near-zero area.
- Existing module-length behavior remains covered by a test.
- Focused crate tests, formatting, clippy, and the repository verification gate pass.

## Follow-up

Once this stage's renderer-free contact sheet verifies the corrected assembly, a separate design
will wire `vehicle_build` through Forge artifacts, their source hashes, and the client loader.
