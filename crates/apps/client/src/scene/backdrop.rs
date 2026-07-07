//! The world beyond the border — render-only. Without it the valley ends at a cliff into sky;
//! with it the Bystra flows in from beyond the horizon, the enclosing hills close the view,
//! and distant trees give the fog something to melt. Physics is untouched: the skirt is baked
//! into the static battle upload and nothing samples it but the eye.

use glam::Vec3;
use renderer_api::{SceneVertex, WaterVertex};
use terrain::{BattlefieldMap, SceneryInstance, SceneryKind, bystra_backdrop_height};

/// Skirt extents and resolution: 40 m cells over −1500..2500 keep the whole ring under the
/// triangle budget; at these viewing distances (and under the aerial fog) finer would be
/// spent on pixels nobody can resolve.
const SKIRT_MIN_M: f32 = -1500.0;
const SKIRT_MAX_M: f32 = 2500.0;
const SKIRT_CELL_M: f32 = 40.0;
/// Cells fully deeper than this inside the map are skipped (the real terrain covers them);
/// nearer cells are kept but tucked under it, so the seam is an overlap, never a crack.
const INTERIOR_SKIP_MARGIN_M: f32 = 60.0;
const INTERIOR_TUCK_M: f32 = 0.6;

/// True only for the map the backdrop is authored for. Other maps keep their bare horizon
/// until they get their own enclosure.
fn has_backdrop(battlefield: &BattlefieldMap) -> bool {
    battlefield.id == "bystra_valley"
}

/// The skirt mesh: the analytic surface continued beyond the border, colored like the terrain
/// but slightly darkened and desaturated so the playfield stays the brightest thing in view.
pub fn backdrop_scene_mesh(battlefield: &BattlefieldMap) -> (Vec<SceneVertex>, Vec<u32>) {
    if !has_backdrop(battlefield) {
        return (Vec::new(), Vec::new());
    }
    let map = battlefield.heightmap.extent_m();
    let n = ((SKIRT_MAX_M - SKIRT_MIN_M) / SKIRT_CELL_M) as usize + 1;
    let inside = |x: f32, z: f32, margin: f32| -> bool {
        x > margin && x < map[0] - margin && z > margin && z < map[1] - margin
    };

    let mut vertex_index = vec![u32::MAX; n * n];
    let mut vertices: Vec<SceneVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let position_of = |ix: usize, iz: usize| -> (f32, f32) {
        (SKIRT_MIN_M + ix as f32 * SKIRT_CELL_M, SKIRT_MIN_M + iz as f32 * SKIRT_CELL_M)
    };
    let corner =
        |vertex_index: &mut Vec<u32>, vertices: &mut Vec<SceneVertex>, ix: usize, iz: usize| {
            let slot = iz * n + ix;
            if vertex_index[slot] == u32::MAX {
                let (x, z) = position_of(ix, iz);
                let mut y = bystra_backdrop_height(x, z);
                if inside(x, z, 0.0) {
                    // Overlap zone: ride just under the real terrain so any T-junction with the
                    // finer heightmap mesh shows backdrop behind it, never a slit of sky.
                    y -= INTERIOR_TUCK_M;
                }
                vertex_index[slot] = vertices.len() as u32;
                vertices.push(SceneVertex::new(
                    [x, y, z],
                    backdrop_normal(x, z),
                    backdrop_color(x, y, z),
                ));
            }
            vertex_index[slot]
        };

    for iz in 0..n - 1 {
        for ix in 0..n - 1 {
            let (x0, z0) = position_of(ix, iz);
            let (x1, z1) = position_of(ix + 1, iz + 1);
            // Skip cells buried deep under the playfield.
            if inside(x0, z0, INTERIOR_SKIP_MARGIN_M) && inside(x1, z1, INTERIOR_SKIP_MARGIN_M) {
                continue;
            }
            let i00 = corner(&mut vertex_index, &mut vertices, ix, iz);
            let i10 = corner(&mut vertex_index, &mut vertices, ix + 1, iz);
            let i01 = corner(&mut vertex_index, &mut vertices, ix, iz + 1);
            let i11 = corner(&mut vertex_index, &mut vertices, ix + 1, iz + 1);
            indices.extend_from_slice(&[i00, i01, i10, i10, i01, i11]);
        }
    }

    // Distant trees on the enclosing hills: deterministic bands just past the border, dark
    // silhouettes for the fog to work with. Reuses the foliage kit; the aerial perspective
    // does the desaturation.
    let mut seed = 0x8ACD_0D11_u64;
    for _ in 0..180 {
        let hx = backdrop_hash(&mut seed);
        let hz = backdrop_hash(&mut seed);
        let side = backdrop_hash(&mut seed);
        // A band 40..380 m outside the border, on one of the four sides.
        let along = -400.0 + hx * (map[0] + 800.0);
        let out = 40.0 + hz * 340.0;
        let (x, z) = match (side * 4.0) as u32 {
            0 => (along, -out),
            1 => (along, map[1] + out),
            2 => (-out, along),
            _ => (map[0] + out, along),
        };
        // Keep the river's exit corridor clear of trunks.
        if (x - terrain::bystra_river_center_x(z)).abs() < 60.0 {
            continue;
        }
        let kind =
            if backdrop_hash(&mut seed) > 0.35 { SceneryKind::Oak } else { SceneryKind::Poplar };
        let ground = bystra_backdrop_height(x, z);
        crate::scene::foliage::push_scenery_instance(
            &mut vertices,
            &mut indices,
            &SceneryInstance {
                kind,
                position: [x, ground, z],
                yaw_rad: backdrop_hash(&mut seed) * std::f32::consts::TAU,
                scale: 1.1 + backdrop_hash(&mut seed) * 0.6,
            },
        );
    }
    (vertices, indices)
}

/// The river's continuation: flat water strips along the extended centerline beyond both
/// borders, rendered by the same water pipeline as the playfield's surface.
pub fn backdrop_water_mesh(battlefield: &BattlefieldMap) -> (Vec<WaterVertex>, Vec<u32>) {
    let Some(water) = battlefield.water.filter(|_| has_backdrop(battlefield)) else {
        return (Vec::new(), Vec::new());
    };
    let mut vertices: Vec<WaterVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let map_z = battlefield.heightmap.extent_m()[1];
    for (z_from, z_to) in [(SKIRT_MIN_M, 0.0_f32), (map_z, SKIRT_MAX_M)] {
        let mut z = z_from;
        while z < z_to - 1.0e-3 {
            let z_next = (z + SKIRT_CELL_M).min(z_to);
            let start = vertices.len() as u32;
            for (zz, half) in [(z, 26.0_f32), (z_next, 26.0)] {
                let center = terrain::bystra_river_center_x(zz);
                for x in [center - half, center + half] {
                    vertices.push(WaterVertex::new([x, water.surface_level_m, zz], 2.0));
                }
            }
            indices.extend_from_slice(&[
                start,
                start + 2,
                start + 1,
                start + 1,
                start + 2,
                start + 3,
            ]);
            z = z_next;
        }
    }
    (vertices, indices)
}

fn backdrop_normal(x: f32, z: f32) -> [f32; 3] {
    let step = 8.0;
    let dx = bystra_backdrop_height(x - step, z) - bystra_backdrop_height(x + step, z);
    let dz = bystra_backdrop_height(x, z - step) - bystra_backdrop_height(x, z + step);
    Vec3::new(dx, 2.0 * step, dz).normalize().to_array()
}

/// Terrain-family colors, pulled darker and greyer with height so the enclosure reads as
/// wooded hills, not a copy of the playfield.
fn backdrop_color(_x: f32, y: f32, _z: f32) -> [f32; 3] {
    let grass = Vec3::new(0.20, 0.33, 0.16);
    let wooded = Vec3::new(0.14, 0.22, 0.13);
    let t = ((y - 8.0) / 24.0).clamp(0.0, 1.0);
    grass.lerp(wooded, t).to_array()
}

fn backdrop_hash(state: &mut u64) -> f32 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut v = *state;
    v = (v ^ (v >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    v = (v ^ (v >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((v ^ (v >> 31)) >> 40) as f32 / ((1u64 << 24) - 1) as f32
}

#[cfg(test)]
mod tests {
    use terrain::{bystra_valley, prokhorovka_hill_252_2};

    use super::*;

    #[test]
    fn only_the_authored_map_gets_a_skirt() {
        let (vertices, indices) = backdrop_scene_mesh(&prokhorovka_hill_252_2());
        assert!(vertices.is_empty() && indices.is_empty());
        let (water_vertices, _) = backdrop_water_mesh(&prokhorovka_hill_252_2());
        assert!(water_vertices.is_empty());
    }

    #[test]
    fn the_skirt_stays_inside_its_triangle_budget_and_its_indices_are_sane() {
        let map = bystra_valley();
        let (vertices, indices) = backdrop_scene_mesh(&map);
        assert!(!indices.is_empty());
        assert!(indices.len().is_multiple_of(3));
        let tris = indices.len() / 3;
        assert!(
            (8_000..60_000).contains(&tris),
            "the skirt should be a real ring under budget, got {tris} tris"
        );
        assert!(indices.iter().all(|&index| (index as usize) < vertices.len()));
        // Determinism: the same map builds the same horizon.
        assert_eq!(vertices.len(), backdrop_scene_mesh(&map).0.len());
    }

    #[test]
    fn the_river_continues_beyond_both_borders_at_the_water_level() {
        let map = bystra_valley();
        let water = map.water.unwrap();
        let (vertices, indices) = backdrop_water_mesh(&map);
        assert!(!indices.is_empty());
        assert!(vertices.iter().all(|v| (v.position[1] - water.surface_level_m).abs() < 1.0e-6));
        assert!(vertices.iter().any(|v| v.position[2] < 0.0), "south continuation exists");
        assert!(vertices.iter().any(|v| v.position[2] > 1000.0), "north continuation exists");
    }
}
