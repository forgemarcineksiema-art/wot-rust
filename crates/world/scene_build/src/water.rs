//! Build the water-surface mesh from the map's COMPLETE water — the global table and the
//! bounded standing sheets (teren W6): flat grids at each surface level, clipped to the
//! cells that are actually wet, with the real depth baked per vertex (the shader's shore
//! fade and shallow→deep tint read it). One source of truth: the same `level − ground`
//! rule that drives wading, drowning, and shell splashes.

use renderer_api::WaterVertex;
use terrain::{BattlefieldMap, HeightMap, StandingWater, WaterBody};

/// Ignore film-thin sheets — same spirit as the shell splash's minimum depth.
const MIN_WET_DEPTH_M: f32 = 0.05;

/// The battlefield's water mesh; empty on dry maps (`water: None`), so callers can always
/// upload the result and the renderer skips the draw when there is nothing to draw.
pub fn battlefield_water_mesh(battlefield: &BattlefieldMap) -> (Vec<WaterVertex>, Vec<u32>) {
    let (mut vertices, mut indices) = match battlefield.water {
        Some(water) => water_surface_mesh(&battlefield.heightmap, water),
        None => (Vec::new(), Vec::new()),
    };
    // Each standing sheet is its own little table: same walk, its rect, its level. The
    // report gates keep sheets edge-dry and non-overlapping, so the meshes cannot fight.
    for sheet in &battlefield.standing_water {
        let (sheet_vertices, sheet_indices) = sheet_surface_mesh(&battlefield.heightmap, *sheet);
        let base = vertices.len() as u32;
        vertices.extend(sheet_vertices);
        indices.extend(sheet_indices.into_iter().map(|index| index + base));
    }
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
    surface_mesh_in(heightmap, water.surface_level_m, (0, w - 1), (0, h - 1))
}

/// One standing sheet's mesh: the same walk, bounded to the sheet's rect at its own level.
pub fn sheet_surface_mesh(
    heightmap: &HeightMap,
    sheet: StandingWater,
) -> (Vec<WaterVertex>, Vec<u32>) {
    let cell = heightmap.cell_size_m();
    let clamp_x = |value: f32| (value / cell).floor().max(0.0) as usize;
    let x_range = (
        clamp_x(sheet.rect[0]),
        ((sheet.rect[2] / cell).ceil() as usize).min(heightmap.width() - 1),
    );
    let z_range = (
        clamp_x(sheet.rect[1]),
        ((sheet.rect[3] / cell).ceil() as usize).min(heightmap.height() - 1),
    );
    surface_mesh_in(heightmap, sheet.surface_level_m, x_range, z_range)
}

/// The shared quad walk over `[x0..x1] x [z0..z1]` cells at one still level.
fn surface_mesh_in(
    heightmap: &HeightMap,
    surface_level_m: f32,
    (x0, x1): (usize, usize),
    (z0, z1): (usize, usize),
) -> (Vec<WaterVertex>, Vec<u32>) {
    let w = heightmap.width();
    let h = heightmap.height();
    let cell = heightmap.cell_size_m();
    let depth_at =
        |x: usize, z: usize| (surface_level_m - heightmap.sample_at_index(x, z)).max(0.0);

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
                    [x as f32 * cell, surface_level_m, z as f32 * cell],
                    depth_at(x, z).max(0.0),
                    flow_at(x, z),
                ));
            }
            vertex_index[slot]
        };

    for z in z0..z1.min(h - 1) {
        for x in x0..x1.min(w - 1) {
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

    /// Teren W6: two standing sheets mesh at their OWN levels, each surface confined to
    /// its rect — the render side of the one resolution rule.
    #[test]
    fn two_sheets_mesh_at_their_own_levels_inside_their_rects() {
        let heightmap = terrain::heightmap_from_fn(61, 5.0, |x, z| {
            let bowl = |cx: f32, cz: f32| {
                let d2 = (x - cx) * (x - cx) + (z - cz) * (z - cz);
                6.0 * (-d2 / (2.0 * 12.0 * 12.0)).exp()
            };
            10.0 - bowl(80.0, 80.0) - bowl(220.0, 220.0)
        });
        let sheets = [
            terrain::StandingWater { rect: [50.0, 50.0, 110.0, 110.0], surface_level_m: 8.0 },
            terrain::StandingWater { rect: [190.0, 190.0, 250.0, 250.0], surface_level_m: 6.0 },
        ];
        let (mut vertices, _) = sheet_surface_mesh(&heightmap, sheets[0]);
        let (pond_vertices, _) = sheet_surface_mesh(&heightmap, sheets[1]);
        assert!(!vertices.is_empty() && !pond_vertices.is_empty(), "both pools draw");
        vertices.extend(pond_vertices);
        for vertex in &vertices {
            let [x, y, z] = vertex.position;
            if (y - 8.0).abs() < 1.0e-4 {
                assert!(
                    (45.0..=115.0).contains(&x) && (45.0..=115.0).contains(&z),
                    "tarn surface stays by its rect, got ({x}, {z})"
                );
            } else if (y - 6.0).abs() < 1.0e-4 {
                assert!(
                    (185.0..=255.0).contains(&x) && (185.0..=255.0).contains(&z),
                    "pond surface stays by its rect, got ({x}, {z})"
                );
            } else {
                panic!("a water vertex at neither level: {y}");
            }
        }
    }

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
        // Prokhorovka carried this probe until teren W5 gave it the Psel; the mountain
        // pass is the roster's dry map now (and dry BY DESIGN - its dossier says why).
        let (vertices, indices) =
            battlefield_water_mesh(&map_forge::battlefield(terrain::MapId::OrlinyPereval));
        assert!(vertices.is_empty() && indices.is_empty());
    }

    #[test]
    fn the_psel_mesh_stays_in_the_western_lowland() {
        // The W5 river: the wet quads hug the west edge and reach nothing east of the
        // lowland - the render surface obeys the same bound the map contract locks.
        let map = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let water = map.water.expect("the Psel ships");
        let (vertices, indices) = water_surface_mesh(&map.heightmap, water);
        assert!(!indices.is_empty(), "the Psel must produce a surface");
        for vertex in &vertices {
            assert!(
                vertex.position[0] < 132.0,
                "water mesh east of the Psel lowland at x {}",
                vertex.position[0]
            );
        }
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
