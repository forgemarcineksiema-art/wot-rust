# Terrain And Large World Policy

Maps are core gameplay systems for this project. A WoT-like battlefield is not a
single glTF mesh dropped into a scene.

## Required Map Systems

Every production map plan must account for the 12 systems below. Since the Map
Forge program (M1-M7) each one has explicit ownership; a map is ONE RON
blueprint (`map_forge`) compiled deterministically into the runtime truth, and
the editor (`crates/apps/editor`) authors every layer through the same document:

- heightmap / terrain chunks — the blueprint's terrain program + the D1 sculpt
  layer; compiled by `map_forge`, sampled by `terrain::HeightMap`,
- collision terrain — the same heightfield (one grounding truth),
- render terrain LOD — `scene_build` ground + statics meshes,
- splat maps — baked from map truth + the blueprint's `materials` palette,
- roads — blueprint polylines painted into the splat (editor `L` tool),
- decorations — seeded mirrored scatters + fixed spots (editor palette),
- cover objects — blueprint cover boxes; a building's box IS the object,
- spawn points — blueprint spawns (editor `G`; fair maps mirror by construction),
- capture zones — blueprint data since M7 (sim capture rules arrive separately),
- navmesh / bot navigation — `StrategicPoint`s as the deterministic scaffold;
  the report's playability checks (drive-graph reachability, named crossings,
  skeleton density) guard it; full navmesh remains a later milestone,
- occlusion/visibility sectors — the editor's turret-eye viewshed is the
  authoring instrument; runtime sectors remain a later milestone,
- minimap data — built from the compiled map by the client.

The contract report blocks shipping on Errors, golden hashes make every map
change a reviewed change, and `ServerHello.map_content_hash` (protocol v35)
makes both ends of the wire PROVE they compiled the same world.

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
