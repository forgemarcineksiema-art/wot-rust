//! The world beyond the border — render-only. The GROUND out there is the border apron now
//! (`battlefield::append_border_apron`): the real terrain pipeline continued past the red
//! line, so the land beyond the border is shaded exactly like the playfield. This module
//! keeps what the apron cannot carry: the distant tree bands on the enclosing hills and the
//! river's continuation strips. Physics is untouched: everything here is baked into the
//! static battle upload and nothing samples it but the eye.

use renderer_api::{SceneVertex, WaterVertex};
use terrain::{BattlefieldMap, SceneryInstance, SceneryKind};

/// The far river strips still march the skirt's old extents at its coarse step.
const SKIRT_MIN_M: f32 = -1500.0;
const SKIRT_MAX_M: f32 = 2500.0;
const SKIRT_CELL_M: f32 = 40.0;

/// True for maps whose blueprint authors a horizon enclosure (the distant tree bands stand
/// on it). Maps without one keep their bare (apron) horizon.
fn backdrop_blueprint(
    battlefield: &BattlefieldMap,
) -> Option<&'static map_forge::blueprint::MapBlueprint> {
    map_forge::cached_blueprint_by_id(&battlefield.id).filter(|bp| bp.horizon.is_some())
}

/// Distant trees on the enclosing hills: deterministic bands just past the border, dark
/// silhouettes for the fog to work with. Reuses the foliage kit; the aerial perspective does
/// the desaturation. They stand on the blueprint's backdrop height — the same surface the
/// border apron meshes — so the trunks root in the visible ground.
pub fn backdrop_scene_mesh(battlefield: &BattlefieldMap) -> (Vec<SceneVertex>, Vec<u32>) {
    let Some(blueprint) = backdrop_blueprint(battlefield) else {
        return (Vec::new(), Vec::new());
    };
    let map = battlefield.heightmap.extent_m();
    let mut vertices: Vec<SceneVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
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
        if let Some(river) = blueprint.river
            && (x - river.center_x(z)).abs() < 60.0
        {
            continue;
        }
        let kind =
            if backdrop_hash(&mut seed) > 0.35 { SceneryKind::Oak } else { SceneryKind::Poplar };
        let ground = map_forge::backdrop_height(blueprint, x, z);
        crate::foliage::push_scenery_instance_far(
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
    let Some(blueprint) = backdrop_blueprint(battlefield) else {
        return (Vec::new(), Vec::new());
    };
    let (Some(water), Some(river)) = (battlefield.water, blueprint.river) else {
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
                let center = river.center_x(zz);
                // Downstream tangent of the meander center line (biased +Z, the flow direction).
                let eps = 2.0;
                let dcx = river.center_x(zz + eps) - river.center_x(zz - eps);
                let flow = glam::Vec2::new(dcx, 2.0 * eps).normalize();
                for x in [center - half, center + half] {
                    vertices.push(WaterVertex::flowing(
                        [x, water.surface_level_m, zz],
                        2.0,
                        [flow.x, flow.y],
                    ));
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

fn backdrop_hash(state: &mut u64) -> f32 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut v = *state;
    v = (v ^ (v >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    v = (v ^ (v >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((v ^ (v >> 31)) >> 40) as f32 / ((1u64 << 24) - 1) as f32
}

#[cfg(test)]
mod tests {
    use terrain::MapId;

    use super::*;

    #[test]
    fn only_the_authored_map_gets_a_skirt() {
        let (vertices, indices) =
            backdrop_scene_mesh(&map_forge::battlefield(MapId::ProkhorovkaHill252_2));
        assert!(vertices.is_empty() && indices.is_empty());
        let (water_vertices, _) =
            backdrop_water_mesh(&map_forge::battlefield(MapId::ProkhorovkaHill252_2));
        assert!(water_vertices.is_empty());
    }

    #[test]
    fn the_backdrop_is_trees_only_and_stays_under_budget() {
        let map = map_forge::battlefield(MapId::BystraValley);
        let (vertices, indices) = backdrop_scene_mesh(&map);
        assert!(!indices.is_empty());
        assert!(indices.len().is_multiple_of(3));
        let tris = indices.len() / 3;
        // The ground plate moved to the border apron (the real terrain pipeline continued
        // past the red line); what remains here is the distant tree bands.
        assert!(
            (1_000..40_000).contains(&tris),
            "the tree bands should be a real ring under budget, got {tris} tris"
        );
        assert!(indices.iter().all(|&index| (index as usize) < vertices.len()));
        // Determinism: the same map builds the same horizon.
        assert_eq!(vertices.len(), backdrop_scene_mesh(&map).0.len());
    }

    #[test]
    fn the_river_flows_in_from_beyond_both_borders() {
        let map = map_forge::battlefield(MapId::BystraValley);
        let (vertices, indices) = backdrop_water_mesh(&map);
        assert!(!vertices.is_empty() && indices.len().is_multiple_of(3));
        let level = map.water.expect("bystra carries its river").surface_level_m;
        assert!(vertices.iter().all(|v| v.position[1] == level));
        let (min_z, max_z) = vertices.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| {
            (lo.min(v.position[2]), hi.max(v.position[2]))
        });
        assert!(min_z < -1000.0 && max_z > 2000.0, "strips must reach well past both borders");
    }
}
