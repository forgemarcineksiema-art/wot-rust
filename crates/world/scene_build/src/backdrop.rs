//! The world beyond the border — render-only. The GROUND out there is the border apron now
//! (`battlefield::append_border_apron`): the real terrain pipeline continued past the red
//! line, so the land beyond the border is shaded exactly like the playfield. This module
//! keeps what the apron cannot carry: the distant tree ring on the enclosing hills and the
//! river's continuation strips. Physics is untouched: everything here is baked into the
//! static battle upload and nothing samples it but the eye.
//!
//! Inny Poziom F1: the ring stands on Drzewa 3.0. Every tree is the species' impostor — the
//! same crossed quads over the same atlas sprite the instanced ladder draws past 150 m — at
//! a mature individual's size, and the species mix is the map's own (`HorizonSpec::flora`):
//! pine on the pass, willow along the river valley, poplar around the steppe town. The
//! painted-frustum kit that used to stand here (hexagonal cones at 3.4× scale, 25–38 m tall,
//! 40 m past the red line on every map) is deleted.

use renderer_api::{SceneVertex, WaterVertex};
use terrain::{BattlefieldMap, SceneryInstance, SceneryKind};

/// The far river strips still march the skirt's old extents at its coarse step.
const SKIRT_MIN_M: f32 = -1500.0;
const SKIRT_MAX_M: f32 = 2500.0;
const SKIRT_CELL_M: f32 = 40.0;

/// The band the ring stands in: `RING_BAND_MIN_M..RING_BAND_MIN_M + RING_BAND_SPAN_M` past
/// the border, on each of the four sides.
pub const RING_BAND_MIN_M: f32 = 40.0;
pub const RING_BAND_SPAN_M: f32 = 340.0;
/// The ring's instance scale: a mature individual, up to a third larger. The F1 lock — no
/// backdrop tree towers over its species — is this ceiling, because an impostor's rendered
/// tip is the species' baked tip times the instance scale, by construction.
pub const RING_SCALE_MIN: f32 = 1.0;
pub const RING_SCALE_MAX: f32 = 1.3;
/// Trees per ring (Immersja A3.2, was 180): 180 trees on a ~5.6 km perimeter read as a hedge
/// with gaps, not a treeline.
const RING_TREES: usize = 450;
/// The mix a horizon without authored flora falls back to — the pre-F1 ring.
const DEFAULT_FLORA: [(SceneryKind, f32); 2] =
    [(SceneryKind::Oak, 0.65), (SceneryKind::Poplar, 0.35)];

/// True for maps whose blueprint authors a horizon enclosure (the distant tree bands stand
/// on it). Maps without one keep their bare (apron) horizon.
fn backdrop_blueprint(
    battlefield: &BattlefieldMap,
) -> Option<&'static map_forge::blueprint::MapBlueprint> {
    map_forge::cached_blueprint_by_id(&battlefield.id).filter(|bp| bp.horizon.is_some())
}

/// The ring's trees as scenery instances: deterministic bands just past the border, drawn
/// from the horizon's species mix, standing on the blueprint's backdrop height — the same
/// surface the border apron meshes — so the trunks root in the visible ground.
pub fn backdrop_tree_instances(battlefield: &BattlefieldMap) -> Vec<SceneryInstance> {
    let Some(blueprint) = backdrop_blueprint(battlefield) else {
        return Vec::new();
    };
    let horizon = blueprint.horizon.as_ref().expect("backdrop_blueprint filters on horizon");
    let flora: Vec<(SceneryKind, f32)> =
        if horizon.flora.is_empty() { DEFAULT_FLORA.to_vec() } else { horizon.flora.clone() };
    let total_weight: f32 = flora.iter().map(|(_, weight)| weight.max(0.0)).sum();
    let map = battlefield.heightmap.extent_m();
    let mut seed = 0x8ACD_0D11_u64;
    let mut instances = Vec::with_capacity(RING_TREES);
    for _ in 0..RING_TREES {
        let hx = backdrop_hash(&mut seed);
        let hz = backdrop_hash(&mut seed);
        let side = backdrop_hash(&mut seed);
        // A band past the border, on one of the four sides.
        let along = -400.0 + hx * (map[0] + 800.0);
        let out = RING_BAND_MIN_M + hz * RING_BAND_SPAN_M;
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
        let kind = weighted_kind(&flora, backdrop_hash(&mut seed) * total_weight);
        let ground = map_forge::backdrop_height(blueprint, x, z);
        instances.push(SceneryInstance {
            kind,
            position: [x, ground, z],
            yaw_rad: backdrop_hash(&mut seed) * std::f32::consts::TAU,
            scale: RING_SCALE_MIN + backdrop_hash(&mut seed) * (RING_SCALE_MAX - RING_SCALE_MIN),
        });
    }
    instances
}

/// The species at `pick` (in `0..total_weight`) along the cumulative weight line.
fn weighted_kind(flora: &[(SceneryKind, f32)], pick: f32) -> SceneryKind {
    let mut accumulated = 0.0_f32;
    for (kind, weight) in flora {
        accumulated += weight.max(0.0);
        if pick < accumulated {
            return *kind;
        }
    }
    flora.last().map(|(kind, _)| *kind).unwrap_or(SceneryKind::Oak)
}

/// Distant trees on the enclosing hills, baked as the species' impostors: dark silhouettes
/// for the fog to work with, lit live like every leaf card in the world.
pub fn backdrop_scene_mesh(battlefield: &BattlefieldMap) -> (Vec<SceneVertex>, Vec<u32>) {
    let mut vertices: Vec<SceneVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for instance in backdrop_tree_instances(battlefield) {
        crate::foliage::push_impostor_tree(&mut vertices, &mut indices, &instance);
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
    use std::collections::BTreeMap;

    use terrain::MapId;

    use super::*;

    /// RENEGOTIATED (Immersja A3.1): the old lock (`only_the_authored_map_gets_a_skirt`)
    /// protected the days when Prokhorovka authored no horizon and its world honestly
    /// ended at the apron. Every shipped map declares its horizon form now, so the law
    /// flips: EVERY map grows its distant tree ring — and only a map with a river sends
    /// water past the border.
    #[test]
    fn every_map_grows_its_tree_ring_and_only_rivers_flow_past_the_border() {
        for id in MapId::SHIPPED.iter().copied() {
            let map = map_forge::battlefield(id);
            let (vertices, indices) = backdrop_scene_mesh(&map);
            assert!(
                !vertices.is_empty() && !indices.is_empty(),
                "{id:?}: every horizon carries its tree ring"
            );
            let (water_vertices, _) = backdrop_water_mesh(&map);
            assert_eq!(
                map.water.is_some() && cached_river(&map),
                !water_vertices.is_empty(),
                "{id:?}: water past the border needs a river, and a river demands it"
            );
        }
    }

    fn cached_river(map: &terrain::BattlefieldMap) -> bool {
        map_forge::cached_blueprint_by_id(&map.id).is_some_and(|bp| bp.river.is_some())
    }

    fn horizon_flora(map: &terrain::BattlefieldMap) -> Vec<(SceneryKind, f32)> {
        map_forge::cached_blueprint_by_id(&map.id)
            .and_then(|bp| bp.horizon.as_ref())
            .map(|horizon| horizon.flora.clone())
            .expect("every shipped map authors a horizon")
    }

    /// Immersja A3.2: the far plane must REACH the world it is shown. The border apron
    /// continues the ground `APRON_FAR_OUT_M` past the red line, and the farthest a
    /// camera can stand from that rim is the map's own long side away — computed here
    /// from the real constants, so neither number can drift past the other in prose.
    #[test]
    fn the_far_plane_reaches_the_aprons_outer_rim_on_every_map() {
        let far = renderer_api::CameraProjectionPolicy::webgpu_default().far_plane_m();
        for id in MapId::SHIPPED.iter().copied() {
            let map = map_forge::battlefield(id);
            let extent = map.heightmap.extent_m();
            let reach = crate::battlefield::APRON_FAR_OUT_M + extent[0].max(extent[1]);
            assert!(
                far >= reach,
                "{id:?}: the far plane ({far} m) clips the apron's rim ({reach} m)"
            );
        }
    }

    /// The ring is impostors and nothing else (F1): every vertex rides the FOLIAGE role with
    /// a real atlas uv — no bark frusta, no painted cones — and the whole ring stays a
    /// fraction of one playfield oak: eight triangles a tree (two crossed quads, two faces
    /// each).
    #[test]
    fn the_backdrop_is_impostor_trees_only_and_stays_under_budget() {
        let map = map_forge::battlefield(MapId::BystraValley);
        let (vertices, indices) = backdrop_scene_mesh(&map);
        assert!(!indices.is_empty());
        assert!(indices.len().is_multiple_of(3));
        let tris = indices.len() / 3;
        let trees = backdrop_tree_instances(&map).len();
        assert_eq!(tris, trees * 8, "eight triangles a tree, no kit left over");
        assert!(
            (2_000..8_000).contains(&tris),
            "the ring should be a real ring under budget, got {tris} tris"
        );
        assert!(indices.iter().all(|&index| (index as usize) < vertices.len()));
        assert!(
            vertices.iter().all(|vertex| vertex.surface == renderer_api::surface_role::FOLIAGE
                && vertex.uv != [0.0, 0.0]),
            "a ring vertex that is not a sprite-sampling foliage quad is the frustum kit"
        );
        // Determinism: the same map builds the same horizon.
        assert_eq!(vertices.len(), backdrop_scene_mesh(&map).0.len());
    }

    /// The species mix is the map's own (F1): every ring tree is a kind the horizon names,
    /// at least two kinds stand on every map, and no single kind is the whole ring — the
    /// monoculture this row was opened for cannot come back through the blueprint.
    #[test]
    fn the_ring_grows_the_species_its_horizon_names_and_never_one_alone() {
        for id in MapId::SHIPPED.iter().copied() {
            let map = map_forge::battlefield(id);
            let flora = horizon_flora(&map);
            assert!(flora.len() >= 2, "{id:?}: a horizon authors at least two species");
            let named: Vec<SceneryKind> = flora.iter().map(|(kind, _)| *kind).collect();
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            let instances = backdrop_tree_instances(&map);
            for instance in &instances {
                assert!(
                    named.contains(&instance.kind),
                    "{id:?}: the ring grew {:?}, which the horizon never named",
                    instance.kind
                );
                *counts.entry(format!("{:?}", instance.kind)).or_default() += 1;
            }
            assert!(counts.len() >= 2, "{id:?}: the ring is a monoculture: {counts:?}");
            let largest = counts.values().copied().max().unwrap_or(0);
            assert!(
                largest * 100 <= instances.len() * 80,
                "{id:?}: one species is {largest} of {} ring trees: {counts:?}",
                instances.len()
            );
        }
    }

    /// No backdrop tree towers over its species (F1): the ring's scale never leaves
    /// `RING_SCALE_MIN..=RING_SCALE_MAX`, and the rendered tip of every tree is its species'
    /// baked tip times that scale — measured on the vertices, not the constants.
    #[test]
    fn no_ring_tree_towers_over_its_species() {
        for id in MapId::SHIPPED.iter().copied() {
            let map = map_forge::battlefield(id);
            for instance in backdrop_tree_instances(&map) {
                assert!(
                    (RING_SCALE_MIN..=RING_SCALE_MAX).contains(&instance.scale),
                    "{id:?}: ring scale {} left the mature band",
                    instance.scale
                );
                let species = crate::foliage::tree_species(instance.kind).expect("a ring tree");
                let tip = crate::foliage_atlas_paint::impostor_window(species).top_m;
                let (mut vertices, mut indices) = (Vec::new(), Vec::new());
                crate::foliage::push_impostor_tree(&mut vertices, &mut indices, &instance);
                let top = vertices.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
                let rendered = top - instance.position[1];
                assert!(
                    rendered <= tip * RING_SCALE_MAX + 1.0e-3,
                    "{id:?}: a {:?} renders {rendered:.2} m tall against a {tip:.2} m species",
                    instance.kind
                );
            }
        }
    }

    /// The ring stands in its band, outside the red line on all four sides: every tree hangs
    /// off one side of the map rectangle by a distance inside the band (a corner tree may
    /// also overhang the other axis — that is the ring wrapping the corner, not a stray).
    #[test]
    fn the_ring_stands_in_its_band_past_the_border() {
        let band = RING_BAND_MIN_M..=RING_BAND_MIN_M + RING_BAND_SPAN_M;
        for id in MapId::SHIPPED.iter().copied() {
            let map = map_forge::battlefield(id);
            let extent = map.heightmap.extent_m();
            for instance in backdrop_tree_instances(&map) {
                let [x, _, z] = instance.position;
                let dx = (-x).max(x - extent[0]);
                let dz = (-z).max(z - extent[1]);
                assert!(
                    band.contains(&dx) || band.contains(&dz),
                    "{id:?}: a ring tree stands {dx:.1} / {dz:.1} m off the border, outside the band"
                );
            }
        }
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
