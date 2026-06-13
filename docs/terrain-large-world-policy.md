# Terrain And Large World Policy

Maps are core gameplay systems for this project. A WoT-like battlefield is not a
single glTF mesh dropped into a scene.

## Required Map Systems

Every production map plan must account for:

- heightmap / terrain chunks,
- collision terrain,
- render terrain LOD,
- splat maps,
- roads,
- decorations,
- cover objects,
- spawn points,
- capture zones,
- navmesh / bot navigation,
- occlusion/visibility sectors,
- minimap data.

These systems may start as data stubs, but they must have explicit ownership in
the terrain/server/client/render pipeline before map content grows.

## Coordinate Precision

Normal battle maps use `f32` for world/game simulation coordinates and `f32` for
renderer coordinates. Current WoT-like maps are bounded battlefields, not
open-world streaming spaces, so `f64` everywhere would add complexity too early.

The escape hatch is origin rebasing for maps above the configured threshold. The
initial policy keeps maps up to 4096 m extent in direct `f32` coordinates and
requires origin rebasing beyond that. If the project later needs much larger
worlds, add a dedicated world-coordinate layer instead of leaking `f64` through
gameplay and renderer code.

## Renderer Depth

wgpu follows the WebGPU/D3D/Metal depth convention: clip-space depth maps to
depth range [0, 1]. Camera projection helpers, shader assumptions, and depth
tests must use that convention from the start.

The default camera policy uses a 0.5 m near plane and 2000 m far plane. Terrain
and vehicle render paths can tighten these per view later, but they must not
silently switch to an OpenGL-style [-1, 1] depth assumption.
