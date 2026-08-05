//! The mid-field meadow, costume B (Jedna Trawa P3): FAR TUFTS baked statically for the
//! whole map and drawn through the renderer's dressing slot (color pass only, chunk-culled,
//! distance cut). The old solid-trapezoid "tents" are dead. A far tuft is two crossed
//! serrated planes — peak, valley, peak — whose tallest tooth is EXACTLY the near tuft's
//! tallest blade at that candidate's scale, in the exact albedo of the ground it stands on.
//! Above all: costume B reads the SAME per-cell candidate stream as the near ring
//! (`grass::CellStream`) and applies the SAME acceptance (`grass::tuft_ground`), so a far
//! card can only stand where a near tuft stands — the hand-off between costumes swaps the
//! silhouette of one object instead of revealing two unrelated populations.
//! Deterministic (map + cell hash) and mirror-fair by construction: the scatter is generated
//! for the south half and emitted with its exact mirrored twin.

use glam::Vec3;
use renderer_api::{SceneVertex, TerrainGroundMaps, TerrainMaterialSet, surface_role};
use terrain::BattlefieldMap;

use crate::grass::{
    BALD_CUT, CELL_M, CELL_TUFT_CANDIDATES, CRATER_KILL_FACTOR, CellStream, GrassSpecies,
    MeadowGround, meadow_baldness, species_at, vegetation_weight,
};

/// Far tufts a fully-vegetated cell keeps: the first N standing candidates of the cell's
/// stream. The near ring grows all 28; the far costume keeps the silhouette-carrying few.
const CELL_FAR_TUFTS: f32 = 4.5;
/// The sway lane doubles as height-over-root for the vertex-stage collapse: sway = h * this.
const SWAY_PER_HEIGHT: f32 = 0.3;

/// Bake the whole map's far-tuft meadow. Pure function of (map, splat, materials) — every
/// client bakes the identical field; a crater re-bake goes through here too.
pub fn grass_card_dressing_mesh(
    battlefield: &BattlefieldMap,
    maps: &TerrainGroundMaps,
    materials: &TerrainMaterialSet,
) -> (Vec<SceneVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let heightmap = &battlefield.heightmap;
    let [extent_x, extent_z] = heightmap.extent_m();
    let mirror_z = extent_z * 0.5;
    let craters: Vec<(f32, f32, f32)> = heightmap
        .crater_records()
        .iter()
        .map(|crater| (crater.x_m(), crater.z_m(), crater.radius_m() * CRATER_KILL_FACTOR))
        .collect();

    let meadow = MeadowGround {
        maps,
        heightmap,
        water: battlefield.water,
        cover: &battlefield.static_cover,
    };
    let cols = (extent_x / CELL_M).floor() as i32;
    let south_rows = (mirror_z / CELL_M).ceil() as i32;
    for cz in 0..south_rows {
        for cx in 0..cols {
            // The budget reads the cell centre (a bare-centre cell keeps no far tufts);
            // STANDING is still decided per candidate position below, so a street through
            // a grassy cell stays bare (D19) — the same split the near ring uses.
            let veg_centre =
                vegetation_weight(maps, (cx as f32 + 0.5) * CELL_M, (cz as f32 + 0.5) * CELL_M);
            let budget = (veg_centre * CELL_FAR_TUFTS).round() as usize;
            if budget == 0 {
                continue;
            }
            let mut stream = CellStream::new(cx, cz);
            let mut taken = 0usize;
            for _ in 0..CELL_TUFT_CANDIDATES {
                if taken >= budget {
                    break;
                }
                let candidate = stream.next_candidate();
                // The mid row seeds only its own half, so the fold never doubles a card.
                if candidate.z >= mirror_z {
                    continue;
                }
                // Bald patches (D7): the source z IS the folded coordinate, so the mirrored
                // twin inherits exactly the near ring's field.
                if meadow_baldness(candidate.x, candidate.z) < BALD_CUT {
                    continue;
                }
                // Only silhouette carriers keep a far costume; the carpet is sub-pixel out
                // here — its far costume is the ground itself. Skipping it consumes no
                // budget, so the tall species still fill the cell's quota.
                let species =
                    species_at(candidate.x, candidate.z, stream.cell_dry, candidate.species_lane);
                if !species.wears_far_costume() {
                    continue;
                }
                // THE unification gate: the identical acceptance the near ring applies, at
                // the identical position, with the identical stochastic lane. A candidate
                // consumes budget only when its south original stands — the far meadow is
                // the first-N standing prefix of the near ring's own population.
                let Some(ground) = meadow.tuft_ground(
                    &craters,
                    candidate.x,
                    candidate.z,
                    candidate.vegetation_lane,
                ) else {
                    continue;
                };
                let Some(albedo) = card_albedo(
                    maps,
                    materials,
                    candidate.x,
                    candidate.z,
                    stream.cell_dry * 0.5 + candidate.tone * 0.5,
                ) else {
                    continue;
                };
                taken += 1;
                push_far_tuft(
                    &mut vertices,
                    &mut indices,
                    Vec3::new(candidate.x, ground, candidate.z),
                    candidate.yaw,
                    candidate.size,
                    candidate.tone,
                    species,
                    albedo,
                );
                // The twin stands (or falls) on its own mirrored ground.
                let (tx, tz) = (candidate.x, extent_z - candidate.z);
                let tyaw = std::f32::consts::TAU - candidate.yaw;
                if let (Some(tground), Some(talbedo)) = (
                    meadow.tuft_ground(&craters, tx, tz, candidate.vegetation_lane),
                    card_albedo(
                        maps,
                        materials,
                        tx,
                        tz,
                        stream.cell_dry * 0.5 + candidate.tone * 0.5,
                    ),
                ) {
                    push_far_tuft(
                        &mut vertices,
                        &mut indices,
                        Vec3::new(tx, tground, tz),
                        tyaw,
                        candidate.size,
                        candidate.tone,
                        species,
                        talbedo,
                    );
                }
            }
        }
    }
    (vertices, indices)
}

/// The exact tone of the ground under a card: splat-weighted layer albedo with a small sky
/// lift — blades catch more light than the soil they stand on, but stay the SAME color
/// family, so the card field dissolves into the ground instead of contrasting with it.
pub(crate) fn card_albedo(
    maps: &TerrainGroundMaps,
    materials: &TerrainMaterialSet,
    x: f32,
    z: f32,
    tone: f32,
) -> Option<[f32; 3]> {
    let size = maps.size as usize;
    let tx = ((x / maps.extent_m[0]) * maps.size as f32).clamp(0.0, maps.size as f32 - 1.0);
    let tz = ((z / maps.extent_m[1]) * maps.size as f32).clamp(0.0, maps.size as f32 - 1.0);
    let index = (tz as usize * size + tx as usize) * 4;
    let weights = &maps.splat[index..index + 4];
    let total: u32 = weights.iter().map(|&w| u32::from(w)).sum();
    if total == 0 {
        return None;
    }
    let mut albedo = Vec3::ZERO;
    for (layer, &weight) in materials.layers.iter().zip(weights) {
        albedo += Vec3::from_array(layer.albedo) * (f32::from(weight) / total as f32);
    }
    Some((albedo * (1.02 + tone * 0.08)).to_array())
}

/// One far tuft: two crossed SERRATED planes (peak – valley – peak), both faces wound —
/// 10 vertices, 12 triangles. The tallest tooth is exactly the near tuft's tallest blade
/// at this candidate's scale (height continuity is one number per species), and
/// the tone gradient (0.72 base / 0.9 valley / 1.05 peaks) matches the near blades', so the
/// hand-off swaps silhouette detail, never colour or height. The sway lane carries
/// height-over-root for the shader's collapse AND the wind.
#[allow(clippy::too_many_arguments)]
fn push_far_tuft(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    root: Vec3,
    yaw: f32,
    size: f32,
    tooth: f32,
    species: GrassSpecies,
    albedo: [f32; 3],
) {
    let albedo = crate::grass::species_tinted_albedo(species, albedo);
    let height = species.tallest_mesh_m() * size;
    let half_w = 0.30 * size;
    let tall = height;
    let short = height * (0.62 + tooth * 0.24);
    let valley = height * (0.38 + tooth * 0.1);
    let base_tone = [albedo[0] * 0.72, albedo[1] * 0.72, albedo[2] * 0.72];
    let valley_tone = [albedo[0] * 0.9, albedo[1] * 0.9, albedo[2] * 0.9];
    let peak_tone = [albedo[0] * 1.05, albedo[1] * 1.05, albedo[2] * 1.05];
    let (sin, cos) = yaw.sin_cos();
    for second in [false, true] {
        let (dir_x, dir_z) = if second { (-sin, cos) } else { (cos, sin) };
        // Alternate which side carries the tall tooth so the crossed planes show four
        // distinct teeth from any viewing angle.
        let (right_peak, left_peak) = if second { (short, tall) } else { (tall, short) };
        let base = vertices.len() as u32;
        for (offset, y, tone) in [
            (-half_w, 0.0, base_tone),
            (half_w, 0.0, base_tone),
            (half_w * 0.55, right_peak, peak_tone),
            (0.0, valley, valley_tone),
            (-half_w * 0.55, left_peak, peak_tone),
        ] {
            vertices.push(SceneVertex {
                position: [root.x + dir_x * offset, root.y + y, root.z + dir_z * offset],
                normal: [0.0, 1.0, 0.0],
                color: tone,
                tint_weight: 0.0,
                gloss: 0.05,
                surface: surface_role::GRASS_CARD,
                sway: y * SWAY_PER_HEIGHT,
                uv: [0.0, 0.0],
                bounce: [0.0; 3],
            });
        }
        // Fan from the left base corner across the serrated top, both windings.
        let (b0, b1, tr, tv, tl) = (base, base + 1, base + 2, base + 3, base + 4);
        indices.extend_from_slice(&[b0, b1, tr, b0, tr, tv, b0, tv, tl]);
        indices.extend_from_slice(&[tr, b1, b0, tv, tr, b0, tl, tv, b0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain_maps::{bake_terrain_ground_maps, terrain_material_set_for};

    fn baked() -> (Vec<SceneVertex>, Vec<u32>) {
        let map = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let maps = bake_terrain_ground_maps(&map);
        let materials = terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2);
        grass_card_dressing_mesh(&map, &maps, &materials)
    }

    /// A flat, fully-vegetated 256 m battlefield: symmetric by construction, so mirror and
    /// unification facts are provable on it without map-content noise.
    fn flat_battlefield() -> (BattlefieldMap, TerrainGroundMaps) {
        let heightmap = terrain::HeightMap::flat(65, 65, 4.0, 1.0).expect("flat map");
        let mut splat = Vec::new();
        for _ in 0..4 {
            splat.extend_from_slice(&[255, 0, 0, 0]);
        }
        let maps = TerrainGroundMaps {
            size: 2,
            splat,
            macro_normal: vec![128; 2 * 2 * 4],
            extent_m: heightmap.extent_m(),
        };
        let battlefield = BattlefieldMap {
            id: "flat".into(),
            name: "flat".into(),
            size_m: heightmap.extent_m(),
            historical_basis: String::new(),
            design_notes: vec![],
            heightmap,
            water: None,
            river: None,
            spawn_zones: vec![],
            capture_zones: vec![],
            strategic_points: vec![],
            features: vec![],
            static_cover: vec![],
            scenery: vec![],
            roads: vec![],
        };
        (battlefield, maps)
    }

    /// THE seam guarantee (Jedna Trawa P3): every far tuft inside the near ring's cache
    /// stands EXACTLY on a near tuft — same root position, and its tallest tooth is the
    /// near tuft's tallest blade at that candidate's scale. The far meadow is a subset of
    /// the one population, not a second one.
    #[test]
    fn a_far_tuft_stands_only_where_the_near_ring_grows_one() {
        let (battlefield, maps) = flat_battlefield();
        let materials = TerrainMaterialSet::default();
        let (vertices, _) = grass_card_dressing_mesh(&battlefield, &maps, &materials);
        let eye = glam::Vec3::new(128.0, 3.0, 80.0);
        let ring = crate::grass::grass_frame_objects(
            &battlefield.heightmap,
            None,
            &[],
            &maps,
            &materials,
            eye,
        );
        let mut checked = 0;
        for card in vertices.chunks(10) {
            let root_x = (card[0].position[0] + card[1].position[0]) * 0.5;
            let root_z = (card[0].position[2] + card[1].position[2]) * 0.5;
            if (root_x - eye.x).hypot(root_z - eye.z) > crate::grass::GRASS_RADIUS_M {
                continue;
            }
            let near = ring
                .iter()
                .find(|tuft| {
                    (tuft.transform[3][0] - root_x).abs() < 1.0e-3
                        && (tuft.transform[3][2] - root_z).abs() < 1.0e-3
                })
                .unwrap_or_else(|| panic!("a far tuft at ({root_x}, {root_z}) has no near twin"));
            let species = GrassSpecies::from_mesh_handle(near.mesh)
                .expect("the near twin is a grass instance");
            assert!(species.wears_far_costume(), "the carpet keeps no far costume");
            let scale = near.transform[0][0].hypot(near.transform[0][2]);
            let peak =
                card.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max) - card[0].position[1];
            assert!(
                (peak - species.tallest_mesh_m() * scale).abs() < 1.0e-3,
                "height continuity is one number per species: far peak {peak:.4} vs \
                 near {species:?} tallest {:.4}",
                species.tallest_mesh_m() * scale
            );
            checked += 1;
        }
        assert!(checked > 40, "the probe saw a real sample: {checked}");
    }

    /// Costume B's silhouette locks: every plane of every far tuft reads peak–valley–peak
    /// (the tents are dead), and every peak sits inside the near kernel's height band and
    /// under the honesty cap (D1).
    #[test]
    fn far_tufts_are_serrated_and_stay_inside_the_near_height_band() {
        use crate::grass::{GRASS_HEIGHT_CAP_M, TUFT_SCALE_MIN, TUFT_SCALE_SPAN};
        let (vertices, _) = baked();
        for card in vertices.chunks(10).step_by(97) {
            let base_y = card[0].position[1];
            for plane in [&card[0..5], &card[5..10]] {
                let right = plane[2].position[1] - base_y;
                let valley = plane[3].position[1] - base_y;
                let left = plane[4].position[1] - base_y;
                assert!(
                    right > valley && left > valley,
                    "peak–valley–peak, never a flat tent top: {right:.3} {valley:.3} {left:.3}"
                );
            }
            let peak = card.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max) - base_y;
            // The band spans the shortest far-costume species at the smallest scale to the
            // tallest at the largest.
            let shortest_far = GrassSpecies::DrySteppe.tallest_mesh_m();
            let tallest_far = GrassSpecies::TallSeed.tallest_mesh_m();
            let band = (shortest_far * TUFT_SCALE_MIN - 1.0e-3)
                ..=(tallest_far * (TUFT_SCALE_MIN + TUFT_SCALE_SPAN) + 1.0e-3);
            assert!(
                band.contains(&peak),
                "a far peak is a near tuft's tallest blade at some legal scale: {peak:.3}"
            );
            assert!(peak <= GRASS_HEIGHT_CAP_M, "the honesty cap holds far too: {peak:.3}");
        }
    }

    #[test]
    fn the_card_meadow_is_deterministic_mirror_fair_and_built_of_cheap_cards() {
        let (vertices, indices) = baked();
        let (twin_v, twin_i) = baked();
        assert!(vertices == twin_v && indices == twin_i, "every client bakes the same field");
        // 10 vertices / 12 triangles per far tuft (two serrated planes, both faces).
        assert_eq!(vertices.len() % 10, 0);
        assert_eq!(indices.len(), (vertices.len() / 10) * 36);
        let cards = vertices.len() / 10;
        assert!(
            (20_000..90_000).contains(&cards),
            "the whole-map meadow is tens of thousands of far tufts: {cards}"
        );
    }

    /// Mirror-fairness, proven where it is provable: on symmetric ground every far tuft has
    /// its exact twin. (On a real map a twin additionally answers for its OWN position —
    /// cover, water, craters, splat — exactly like the near ring does.)
    #[test]
    fn on_symmetric_ground_every_far_tuft_has_its_mirror_twin() {
        let (battlefield, maps) = flat_battlefield();
        let extent_z = battlefield.heightmap.extent_m()[1];
        let (vertices, _) =
            grass_card_dressing_mesh(&battlefield, &maps, &TerrainMaterialSet::default());
        let roots: Vec<(f32, f32)> = vertices
            .chunks(10)
            .map(|card| {
                (
                    (card[0].position[0] + card[1].position[0]) * 0.5,
                    (card[0].position[2] + card[1].position[2]) * 0.5,
                )
            })
            .collect();
        assert!(roots.len() > 600, "enough tufts for the probe: {}", roots.len());
        for &(x, z) in roots.iter().step_by(roots.len() / 41) {
            let twin_exists = roots
                .iter()
                .any(|&(ox, oz)| (ox - x).abs() < 1.0e-3 && (oz - (extent_z - z)).abs() < 1.0e-3);
            assert!(twin_exists, "far tuft at ({x}, {z}) has no mirror twin");
        }
    }

    #[test]
    fn cards_wear_the_ground_tone_and_the_sway_lane_encodes_height() {
        let (vertices, _) = baked();
        for card in vertices.chunks(10).step_by(211) {
            let base_y = card[0].position[1];
            for vertex in card {
                assert!(
                    (vertex.surface - surface_role::GRASS_CARD).abs() < 1.0e-3,
                    "cards dispatch as GRASS_CARD"
                );
                let (r, g, b) = (vertex.color[0], vertex.color[1], vertex.color[2]);
                let max = r.max(g).max(b);
                let saturation = if max <= 0.0 { 0.0 } else { (max - r.min(g).min(b)) / max };
                assert!(saturation <= 0.45, "rule 2: ground stays muted, got {saturation}");
                // The shader's collapse contract: sway IS height-over-root × 0.3, at every
                // vertex — the serrated tops each carry their own height.
                let over_root = vertex.position[1] - base_y;
                assert!(
                    (vertex.sway - over_root * SWAY_PER_HEIGHT).abs() < 1.0e-4,
                    "sway encodes height-over-root: {} vs {}",
                    vertex.sway,
                    over_root * SWAY_PER_HEIGHT
                );
            }
        }
    }

    #[test]
    fn the_meadow_clumps_like_the_near_ring() {
        // D7, card half: the same Clark–Evans direction as the near ring's lock — the mean
        // nearest-neighbour distance of card roots sits well under the uniform-Poisson
        // expectation, because both systems share one clump rule in `crate::grass`.
        let heightmap = terrain::HeightMap::flat(65, 65, 5.0, 1.0).expect("flat map");
        let battlefield = BattlefieldMap {
            id: "flat".into(),
            name: "flat".into(),
            size_m: heightmap.extent_m(),
            historical_basis: String::new(),
            design_notes: vec![],
            heightmap,
            water: None,
            river: None,
            spawn_zones: vec![],
            capture_zones: vec![],
            strategic_points: vec![],
            features: vec![],
            static_cover: vec![],
            scenery: vec![],
            roads: vec![],
        };
        let mut splat = Vec::new();
        for _ in 0..4 {
            splat.extend_from_slice(&[255, 0, 0, 0]);
        }
        let maps = TerrainGroundMaps {
            size: 2,
            splat,
            macro_normal: vec![128; 2 * 2 * 4],
            extent_m: [320.0, 320.0],
        };
        let (vertices, _) =
            grass_card_dressing_mesh(&battlefield, &maps, &TerrainMaterialSet::default());
        let roots: Vec<(f32, f32)> = vertices
            .chunks(10)
            .map(|card| {
                (
                    (card[0].position[0] + card[1].position[0]) * 0.5,
                    (card[0].position[2] + card[1].position[2]) * 0.5,
                )
            })
            .filter(|&(_, z)| z < 160.0)
            .collect();
        assert!(roots.len() > 300, "enough cards for the statistic: {}", roots.len());
        let density = roots.len() as f32 / (320.0 * 160.0);
        let uniform_expectation = 0.5 / density.sqrt();
        let mut total = 0.0;
        for (i, &(x, z)) in roots.iter().enumerate() {
            let mut best = f32::INFINITY;
            for (j, &(ox, oz)) in roots.iter().enumerate() {
                if i != j {
                    best = best.min((x - ox).hypot(z - oz));
                }
            }
            total += best;
        }
        let clumped = total / roots.len() as f32;
        assert!(
            clumped < 0.85 * uniform_expectation,
            "the meadow must clump (mean NN {clumped:.3} vs uniform {uniform_expectation:.3})"
        );
    }

    #[test]
    fn the_city_grows_no_cards_on_streets_or_through_floors() {
        // The two mechanical roots of D19's meadow-in-the-canyon: the cell-centre gate let
        // cards straddle streets the centre missed, and nothing excluded building
        // footprints. Both are locked here on the real city.
        let map = map_forge::battlefield(terrain::MapId::Ostrogorsk);
        let maps = bake_terrain_ground_maps(&map);
        let materials = terrain_material_set_for(terrain::MapId::Ostrogorsk);
        let (vertices, _) = grass_card_dressing_mesh(&map, &maps, &materials);
        assert!(!vertices.is_empty(), "the verges and the east farmland still grow");
        let stone_roads: Vec<_> =
            map.roads.iter().filter(|road| road.surface != terrain::RoadSurface::Dirt).collect();
        assert!(!stone_roads.is_empty(), "the city keeps its cobbles and ballast");
        for card in vertices.chunks(10) {
            let root_x = (card[0].position[0] + card[1].position[0]) * 0.5;
            let root_z = (card[0].position[2] + card[1].position[2]) * 0.5;
            for road in &stone_roads {
                assert!(
                    road.distance_to(root_x, root_z) > road.width_m * 0.2,
                    "a card rooted on the {} at ({root_x:.1}, {root_z:.1})",
                    road.id
                );
            }
            assert!(
                !terrain::inside_any_cover(&map.static_cover, root_x, root_z, 0.0),
                "a card rooted through a building floor at ({root_x:.1}, {root_z:.1})"
            );
        }
    }

    #[test]
    fn a_fresh_crater_mows_its_patch_out_of_the_card_meadow() {
        let mut map = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let maps = bake_terrain_ground_maps(&map);
        let materials = terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2);
        let crater = terrain::CraterRecord::from_world(
            500.0,
            480.0,
            3.0,
            1.0,
            terrain::CRATER_KIND_HIGH_EXPLOSIVE,
        );
        map.heightmap.set_craters(&[crater]);
        let (vertices, _) = grass_card_dressing_mesh(&map, &maps, &materials);
        let kill = crater.radius_m() * CRATER_KILL_FACTOR;
        for card in vertices.chunks(10) {
            // The ROOT is the midpoint of the base edge (vertex 0 and 1 are offset by the
            // card's half-width); the kill zone is measured from where the clump grows.
            let root_x = (card[0].position[0] + card[1].position[0]) * 0.5;
            let root_z = (card[0].position[2] + card[1].position[2]) * 0.5;
            assert!(
                (root_x - crater.x_m()).hypot(root_z - crater.z_m()) >= kill - 1.0e-3,
                "no card grows in the burst: ({root_x}, {root_z})"
            );
        }
    }
}
