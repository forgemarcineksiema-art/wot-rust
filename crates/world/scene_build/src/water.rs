//! Build the river-surface mesh from the map's water body: a flat grid at `surface_level_m`,
//! clipped to the cells that are actually wet, with the real depth baked per vertex (the
//! shader's shore fade and shallow→deep tint read it). One source of truth: the same
//! `WaterBody::depth_over(heightmap)` rule that drives wading, drowning, and shell splashes.

use renderer_api::WaterVertex;
use terrain::{BattlefieldMap, HeightMap, WaterBody};

/// Ignore film-thin sheets — same spirit as the shell splash's minimum depth.
const MIN_WET_DEPTH_M: f32 = 0.05;

/// The battlefield's water mesh; empty on dry maps (`water: None`), so callers can always
/// upload the result and the renderer skips the draw when there is nothing to draw.
pub fn battlefield_water_mesh(battlefield: &BattlefieldMap) -> (Vec<WaterVertex>, Vec<u32>) {
    let (mut vertices, mut indices) = match battlefield.water {
        Some(water) => water_surface_mesh(&battlefield.heightmap, water),
        None => (Vec::new(), Vec::new()),
    };
    // The river keeps flowing past the horizon: the backdrop strips render with this same mesh.
    let (skirt_vertices, skirt_indices) = crate::backdrop::backdrop_water_mesh(battlefield);
    let base = vertices.len() as u32;
    vertices.extend(skirt_vertices);
    indices.extend(skirt_indices.into_iter().map(|index| index + base));
    (vertices, indices)
}

/// Grid the heightmap at its own cell resolution and keep every quad with at least one wet
/// corner (so the surface plane reaches the exact shoreline where depth crosses zero, and the
/// shader's alpha fade dissolves the dry corners). Vertices sit ON the water plane.
pub fn water_surface_mesh(heightmap: &HeightMap, water: WaterBody) -> (Vec<WaterVertex>, Vec<u32>) {
    let w = heightmap.width();
    let h = heightmap.height();
    let cell = heightmap.cell_size_m();
    let depth_at = |x: usize, z: usize| water.depth_over(heightmap.sample_at_index(x, z));

    // The downstream current at a grid node: the river flows ALONG its channel, i.e.
    // perpendicular to the cross-channel depth gradient (which points toward the deep line).
    // Map-agnostic - it reads only the depth field, so it follows any meander for free.
    // Oriented toward +Z (Bystra's downstream); a flat pool (no gradient) defaults downstream.
    let flow_at = |x: usize, z: usize| -> [f32; 2] {
        let sample = |xi: i32, zi: i32| {
            let cx = xi.clamp(0, w as i32 - 1) as usize;
            let cz = zi.clamp(0, h as i32 - 1) as usize;
            depth_at(cx, cz)
        };
        let (xi, zi) = (x as i32, z as i32);
        let grad = glam::Vec2::new(
            sample(xi + 1, zi) - sample(xi - 1, zi),
            sample(xi, zi + 1) - sample(xi, zi - 1),
        );
        // Perpendicular to the depth gradient = along the channel. Rotate +90 degrees.
        let mut along = glam::Vec2::new(-grad.y, grad.x);
        if along.length_squared() < 1.0e-8 {
            return [0.0, 1.0];
        }
        along = along.normalize();
        if along.y < 0.0 {
            along = -along; // bias to +Z downstream
        }
        [along.x, along.y]
    };

    // Vertex indices are allocated lazily so a mostly-dry map costs only its river corridor.
    let mut vertex_index = vec![u32::MAX; w * h];
    let mut vertices: Vec<WaterVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let corner =
        |vertex_index: &mut Vec<u32>, vertices: &mut Vec<WaterVertex>, x: usize, z: usize| {
            let slot = z * w + x;
            if vertex_index[slot] == u32::MAX {
                vertex_index[slot] = vertices.len() as u32;
                vertices.push(WaterVertex::flowing(
                    [x as f32 * cell, water.surface_level_m, z as f32 * cell],
                    depth_at(x, z).max(0.0),
                    flow_at(x, z),
                ));
            }
            vertex_index[slot]
        };

    for z in 0..h - 1 {
        for x in 0..w - 1 {
            let wet = depth_at(x, z) > MIN_WET_DEPTH_M
                || depth_at(x + 1, z) > MIN_WET_DEPTH_M
                || depth_at(x, z + 1) > MIN_WET_DEPTH_M
                || depth_at(x + 1, z + 1) > MIN_WET_DEPTH_M;
            if !wet {
                continue;
            }
            let i00 = corner(&mut vertex_index, &mut vertices, x, z);
            let i10 = corner(&mut vertex_index, &mut vertices, x + 1, z);
            let i01 = corner(&mut vertex_index, &mut vertices, x, z + 1);
            let i11 = corner(&mut vertex_index, &mut vertices, x + 1, z + 1);
            indices.extend_from_slice(&[i00, i01, i10, i10, i01, i11]);
        }
    }
    (vertices, indices)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn the_baked_current_flows_downstream_along_the_channel() {
        let map = map_forge::battlefield(terrain::MapId::BystraValley);
        let water = map.water.expect("the Bystra is the map");
        let (vertices, _) = water_surface_mesh(&map.heightmap, water);

        let mut flowing = 0usize;
        for vertex in &vertices {
            let flow = glam::Vec2::from(vertex.flow);
            let len = flow.length();
            // Every flow is either still (0) or a unit direction — never a stray magnitude.
            assert!(len < 1.0e-3 || (0.99..=1.01).contains(&len), "flow not unit: {len}");
            if len > 0.5 {
                flowing += 1;
                // The Bystra runs +Z: the current carries a real downstream component, never
                // pointing back upstream (the depth-gradient perpendicular is oriented to +Z).
                assert!(
                    flow.y >= -1.0e-3,
                    "current must not point upstream at {:?}: {:?}",
                    vertex.position,
                    vertex.flow
                );
            }
        }
        assert!(
            flowing * 3 > vertices.len(),
            "most of the river must carry a current: {flowing}/{}",
            vertices.len()
        );
    }

    #[test]
    fn a_dry_map_builds_no_water_mesh() {
        let (vertices, indices) =
            battlefield_water_mesh(&map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2));
        assert!(vertices.is_empty() && indices.is_empty());
    }

    #[test]
    fn the_bystra_mesh_covers_the_river_and_only_the_river() {
        let map = map_forge::battlefield(terrain::MapId::BystraValley);
        let water = map.water.expect("the Bystra is the map");
        // The corridor contract is about the PLAYFIELD surface; the backdrop continuations
        // are covered by scene::backdrop tests.
        let (vertices, indices) = water_surface_mesh(&map.heightmap, water);

        assert!(!indices.is_empty(), "the river corridor must produce a surface");
        assert!(indices.len().is_multiple_of(3));
        assert!(
            vertices.len() < 12_000,
            "the mesh is clipped to the corridor, not the whole map ({} verts)",
            vertices.len()
        );
        for vertex in &vertices {
            assert!(
                (vertex.position[1] - water.surface_level_m).abs() < 1.0e-6,
                "every vertex sits ON the still-water plane"
            );
            let ground =
                map.heightmap.sample_height(vertex.position[0], vertex.position[2]).unwrap();
            assert!(
                (vertex.depth_m - water.depth_over(ground)).abs() < 1.0e-4,
                "baked depth must equal the gameplay depth rule"
            );
            let d = (vertex.position[0] - terrain::bystra_river_center_x(vertex.position[2])).abs();
            assert!(
                d <= terrain::RIVER_CORRIDOR_HALF_WIDTH_M + map.heightmap.cell_size_m() + 3.0,
                "surface vertex outside the river corridor at {:?}",
                vertex.position
            );
        }
        for index in &indices {
            assert!((*index as usize) < vertices.len());
        }
    }
}
