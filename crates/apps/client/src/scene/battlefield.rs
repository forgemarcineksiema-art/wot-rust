use glam::{Mat3, Vec3};
use renderer_api::SceneVertex;
use terrain::{BattlefieldMap, HeightMap, StaticCoverKind, StaticCoverObject, WaterBody};

use crate::tank_mesh::push_oriented_box;

/// Build the static battlefield mesh: the terrain plus every static cover object. Cover is
/// gameplay state (it blocks movement, shells, and the camera), so whatever the simulation
/// collides must be visible — rendering the exact sim boxes keeps the world honest.
pub fn battlefield_scene_mesh(battlefield: &BattlefieldMap) -> (Vec<SceneVertex>, Vec<u32>) {
    battlefield_scene_mesh_with_cover_states(battlefield, &[])
}

/// As [`battlefield_scene_mesh`], dressing each cover object by its replicated phase (protocol
/// v21): intact objects as-authored, a collapsed building as a low rubble mound, a destroyed
/// object (flattened foliage/cleared ground) drawn as nothing — and the scenery trees standing
/// inside a cleared tree line vanish with it. `cover_states` is index-aligned with the map's
/// cover (a phase byte each: 0 intact, 1 rubble, 2 gone); an empty slice is all-intact. The
/// client rebuilds this and re-uploads the scene whenever the states change.
pub fn battlefield_scene_mesh_with_cover_states(
    battlefield: &BattlefieldMap,
    cover_states: &[u8],
) -> (Vec<SceneVertex>, Vec<u32>) {
    let (mut vertices, mut indices) =
        terrain_scene_mesh_with_water(&battlefield.heightmap, battlefield.water);
    for (index, cover) in battlefield.static_cover.iter().enumerate() {
        match cover_states.get(index).copied().unwrap_or(0) {
            0 => append_cover_box(&mut vertices, &mut indices, cover),
            1 => append_rubble_mound(&mut vertices, &mut indices, cover),
            _ => {} // gone: the object is cleared, draw nothing
        }
    }
    // The world beyond the border (render-only skirt + distant trees), then the dressing:
    // both baked into the same static upload.
    {
        let (skirt_vertices, skirt_indices) =
            crate::scene::backdrop::backdrop_scene_mesh(battlefield);
        let base = vertices.len() as u32;
        vertices.extend(skirt_vertices);
        indices.extend(skirt_indices.into_iter().map(|index| index + base));
    }
    // Render-only dressing: trees and rocks baked into the same static upload — a dressed
    // valley costs the frame nothing (see scene::foliage). A tree standing inside a cleared
    // cover box fell with it, so it is left out of the rebuilt scene.
    for instance in &battlefield.scenery {
        if scenery_stands_in_cleared_cover(instance, &battlefield.static_cover, cover_states) {
            continue;
        }
        crate::scene::foliage::push_scenery_instance(&mut vertices, &mut indices, instance);
    }
    (vertices, indices)
}

/// Whether a scenery instance stands inside a cover box that has been cleared (phase gone), so it
/// should vanish with the tree line it dressed. Tested in plan (XZ) — a canopy's trunk is what
/// anchors it to the cleared footprint.
fn scenery_stands_in_cleared_cover(
    instance: &terrain::SceneryInstance,
    cover: &[StaticCoverObject],
    cover_states: &[u8],
) -> bool {
    let p = instance.position;
    cover.iter().enumerate().any(|(index, object)| {
        cover_states.get(index).copied().unwrap_or(0) == 2
            && (p[0] - object.center[0]).abs() <= object.half_extents_m[0]
            && (p[2] - object.center[2]).abs() <= object.half_extents_m[2]
    })
}

/// A collapsed building: a low, rough rubble mound filling the footprint at the sim's reduced
/// height (`rubble_height_frac`), so what the eye reads as a blocking mound matches the box a hull
/// still stops against and a turret-height shot clears.
fn append_rubble_mound(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    cover: &StaticCoverObject,
) {
    let center = Vec3::from_array(cover.center);
    let half = Vec3::from_array(cover.half_extents_m);
    let ground_y = center.y - half.y;
    let mound_half_y = half.y * cover.kind.rubble_height_frac();
    let mound_center = Vec3::new(center.x, ground_y + mound_half_y, center.z);
    // Slightly inset in plan so the pile reads as slumped rubble, not a shrunk building.
    let mound_half = Vec3::new(half.x * 0.9, mound_half_y, half.z * 0.9);
    // Dull broken masonry: grey-brown, matte.
    push_surfaced_box(vertices, indices, mound_center, mound_half, [0.38, 0.34, 0.30], 0.04);
}

/// Every visual stays INSIDE the collision AABB — a building may look like walls and a roof,
/// but nothing it shows can be shot through or hidden behind that the sim box does not honor
/// (locked by `cover_visuals_never_leave_the_collision_box`).
fn append_cover_box(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    cover: &StaticCoverObject,
) {
    let center = Vec3::from_array(cover.center);
    let half = Vec3::from_array(cover.half_extents_m);
    match cover.kind {
        StaticCoverKind::FarmBuilding => append_building(vertices, indices, cover, center, half),
        StaticCoverKind::RailCover => {
            // Stone: walls, parapets, log revetments — a cool masonry tone with a worn sheen.
            push_surfaced_box(vertices, indices, center, half, [0.40, 0.38, 0.34], 0.16);
        }
        StaticCoverKind::TreeLine => {
            // The solid undergrowth mass; real trees (scenery) fill it visually, so the box
            // itself darkens into their shadow instead of competing with the canopies.
            push_surfaced_box(vertices, indices, center, half, [0.11, 0.20, 0.10], 0.05);
        }
        StaticCoverKind::Wreck => {
            // Burnt steel: the glossiest thing on the field short of water.
            push_surfaced_box(vertices, indices, center, half, [0.25, 0.20, 0.17], 0.30);
        }
    }
}

/// [`push_oriented_box`] plus a material finish: the box helper predates the material lane, so
/// the gloss is stamped onto the vertices it just appended.
fn push_surfaced_box(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: Vec3,
    half: Vec3,
    color: [f32; 3],
    gloss: f32,
) {
    let start = vertices.len();
    push_oriented_box(vertices, indices, center, half, Mat3::IDENTITY, color);
    for vertex in &mut vertices[start..] {
        vertex.gloss = gloss;
    }
}

/// A building inside its box: a dark plinth course, plastered walls (palette varied per
/// building id so the town is a town, not a barracks), and a gable roof whose ridge runs the
/// long axis — eaves at the box's sides, ridge at the box's top, so the silhouette fills the
/// collision volume exactly.
fn append_building(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    cover: &StaticCoverObject,
    center: Vec3,
    half: Vec3,
) {
    let (wall, roof, roof_gloss) = building_palette(&cover.id);
    let base_y = center.y - half.y;
    let eaves_y = base_y + half.y * 2.0 * 0.62;
    let plinth_y = base_y + half.y * 2.0 * 0.10;

    // Plinth course (dressed stone, slightly polished by weather), then plaster walls up to
    // the eaves.
    push_surfaced_box(
        vertices,
        indices,
        Vec3::new(center.x, (base_y + plinth_y) * 0.5, center.z),
        Vec3::new(half.x, (plinth_y - base_y) * 0.5, half.z),
        [0.24, 0.22, 0.20],
        0.15,
    );
    push_surfaced_box(
        vertices,
        indices,
        Vec3::new(center.x, (plinth_y + eaves_y) * 0.5, center.z),
        Vec3::new(half.x, (eaves_y - plinth_y) * 0.5, half.z),
        wall,
        0.10,
    );
    push_gable_roof(
        vertices,
        indices,
        center,
        half,
        eaves_y,
        center.y + half.y,
        (roof, roof_gloss),
    );
}

/// The gable: two sloped quads from the eaves rectangle to a ridge line along the long axis,
/// plus the two triangular gable ends.
fn push_gable_roof(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: Vec3,
    half: Vec3,
    eaves_y: f32,
    ridge_y: f32,
    (roof, roof_gloss): ([f32; 3], f32),
) {
    let along_x = half.x >= half.z;
    let (long, short) = if along_x { (half.x, half.z) } else { (half.z, half.x) };
    let axis = if along_x { Vec3::X } else { Vec3::Z };
    let side = if along_x { Vec3::Z } else { Vec3::X };

    let ridge_a = Vec3::new(center.x, ridge_y, center.z) + axis * long;
    let ridge_b = Vec3::new(center.x, ridge_y, center.z) - axis * long;
    for sign in [-1.0_f32, 1.0] {
        // The slope on this side of the ridge.
        let eave_a = Vec3::new(center.x, eaves_y, center.z) + axis * long + side * short * sign;
        let eave_b = Vec3::new(center.x, eaves_y, center.z) - axis * long + side * short * sign;
        let normal = (side * sign * (ridge_y - eaves_y) + Vec3::Y * short).normalize_or_zero();
        let start = vertices.len() as u32;
        for point in [eave_a, eave_b, ridge_b, ridge_a] {
            vertices.push(SceneVertex::surfaced(
                point.to_array(),
                normal.to_array(),
                roof,
                roof_gloss,
            ));
        }
        // Winding follows the outward normal instead of a per-side guess: swapping the ridge
        // between the X and Z axis is a reflection, which flips any hand-picked order (the
        // see-through-roof bug). Locked by `every_cover_triangle_winds_outward`.
        push_winding(indices, start, &[0, 1, 2, 0, 2, 3], {
            (eave_b - eave_a).cross(ridge_b - eave_a).dot(normal) > 0.0
        });
        // The gable-end triangle at this end of the ridge.
        let (ridge_end, outward) = if sign > 0.0 { (ridge_a, axis) } else { (ridge_b, -axis) };
        let g0 = Vec3::new(ridge_end.x, eaves_y, ridge_end.z) + side * short;
        let g1 = Vec3::new(ridge_end.x, eaves_y, ridge_end.z) - side * short;
        let gn = outward.to_array();
        let gable = vertices.len() as u32;
        for point in [g0, g1, ridge_end] {
            vertices.push(SceneVertex::surfaced(point.to_array(), gn, roof, roof_gloss));
        }
        push_winding(indices, gable, &[0, 1, 2], {
            (g1 - g0).cross(ridge_end - g0).dot(outward) > 0.0
        });
    }
}

/// Append triangles (`pattern` holds corner offsets from `start`, three per triangle) either
/// as-is or with each triangle's last two corners swapped, so the front face points where the
/// caller's normal test said it should.
fn push_winding(indices: &mut Vec<u32>, start: u32, pattern: &[u32], keep: bool) {
    for triangle in pattern.chunks(3) {
        let (a, b, c) = (triangle[0], triangle[1], triangle[2]);
        let (b, c) = if keep { (b, c) } else { (c, b) };
        indices.extend_from_slice(&[start + a, start + b, start + c]);
    }
}

/// Deterministic per-building palette from the cover id: plaster/brick walls under tile or
/// slate roofs, each roof with its material's finish (slate shines, shingle barely). The same
/// id always paints the same house.
fn building_palette(id: &str) -> ([f32; 3], [f32; 3], f32) {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in id.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    const WALLS: [[f32; 3]; 4] = [
        [0.62, 0.56, 0.46], // warm plaster
        [0.58, 0.52, 0.48], // grey render
        [0.52, 0.38, 0.28], // brick
        [0.60, 0.58, 0.52], // limewash
    ];
    const ROOFS: [([f32; 3], f32); 3] = [
        ([0.42, 0.24, 0.18], 0.22), // clay tile
        ([0.30, 0.28, 0.30], 0.35), // slate
        ([0.36, 0.30, 0.22], 0.18), // weathered shingle
    ];
    let (roof, roof_gloss) = ROOFS[((hash >> 8) % 3) as usize];
    (WALLS[(hash % 4) as usize], roof, roof_gloss)
}

/// Build a lit triangle mesh for the whole heightmap, colored by height and slope so
/// the terrain reads clearly: grass in the lowlands, rock on the heights and steeps.
pub fn terrain_scene_mesh(heightmap: &HeightMap) -> (Vec<SceneVertex>, Vec<u32>) {
    terrain_scene_mesh_with_water(heightmap, None)
}

/// Like [`terrain_scene_mesh`], with submerged ground tinted river-blue by depth — the
/// stopgap water read until the real animated surface pass lands. Gameplay water (drag,
/// drowning, splashes) is already live; the tint keeps the danger visible in the meantime.
pub fn terrain_scene_mesh_with_water(
    heightmap: &HeightMap,
    water: Option<WaterBody>,
) -> (Vec<SceneVertex>, Vec<u32>) {
    let w = heightmap.width();
    let h = heightmap.height();
    let cell = heightmap.cell_size_m();
    let stats = heightmap.stats();

    let mut vertices = Vec::with_capacity(w * h);
    for z in 0..h {
        for x in 0..w {
            let y = heightmap.sample_at_index(x, z);
            let normal = vertex_normal(heightmap, x, z, cell);
            let mut color = terrain_color(y, stats.min_m, stats.max_m, normal.y);
            // Grass is near-matte; exposed rock on steep faces takes a mineral sheen; the
            // riverbed under water is permanently wet and reads glossiest of all.
            let mut gloss = 0.03 + (1.0 - normal.y).clamp(0.0, 1.0) * 0.12;
            if let Some(water) = water {
                let depth = water.depth_over(y);
                color = water_tint(color, depth);
                if depth > 0.02 {
                    gloss = 0.35;
                }
            }
            vertices.push(SceneVertex::surfaced(
                [x as f32 * cell, y, z as f32 * cell],
                normal.to_array(),
                color,
                gloss,
            ));
        }
    }

    let mut indices = Vec::with_capacity((w - 1) * (h - 1) * 6);
    for z in 0..h - 1 {
        for x in 0..w - 1 {
            let i = (z * w + x) as u32;
            let right = i + 1;
            let down = i + w as u32;
            indices.extend_from_slice(&[i, down, right, right, down, down + 1]);
        }
    }
    (vertices, indices)
}

fn sample_clamped(heightmap: &HeightMap, x: i32, z: i32) -> f32 {
    let cx = x.clamp(0, heightmap.width() as i32 - 1) as usize;
    let cz = z.clamp(0, heightmap.height() as i32 - 1) as usize;
    heightmap.sample_at_index(cx, cz)
}

fn vertex_normal(heightmap: &HeightMap, x: usize, z: usize, cell: f32) -> Vec3 {
    let (xi, zi) = (x as i32, z as i32);
    let left = sample_clamped(heightmap, xi - 1, zi);
    let right = sample_clamped(heightmap, xi + 1, zi);
    let down = sample_clamped(heightmap, xi, zi - 1);
    let up = sample_clamped(heightmap, xi, zi + 1);
    Vec3::new(left - right, 2.0 * cell, down - up).normalize()
}

/// Blend submerged ground toward river water: a pale shallow band at the margins, deepening
/// to a dark channel blue where the current drowns.
fn water_tint(color: [f32; 3], depth_m: f32) -> [f32; 3] {
    if depth_m <= 0.02 {
        return color;
    }
    let shallow = Vec3::new(0.18, 0.32, 0.34);
    let deep = Vec3::new(0.05, 0.13, 0.22);
    let t = (depth_m / 2.4).clamp(0.0, 1.0);
    let water = shallow.lerp(deep, t);
    Vec3::from_array(color).lerp(water, (depth_m / 0.35).clamp(0.35, 1.0)).to_array()
}

fn terrain_color(y: f32, min_y: f32, max_y: f32, normal_y: f32) -> [f32; 3] {
    let span = (max_y - min_y).max(1.0);
    let t = ((y - min_y) / span).clamp(0.0, 1.0);
    let grass = Vec3::new(0.26, 0.44, 0.20);
    let rock = Vec3::new(0.46, 0.41, 0.34);
    let mut color = grass.lerp(rock, t * t);
    // Steep faces drift toward bare rock so slopes read as relief, not flat shading.
    let steep = (1.0 - normal_y).clamp(0.0, 1.0);
    color = color.lerp(Vec3::new(0.33, 0.29, 0.26), steep * 0.6);
    color.to_array()
}

#[cfg(test)]
mod tests {
    use terrain::prokhorovka_hill_252_2;

    use super::*;

    #[test]
    fn a_collapsed_building_slumps_below_its_intact_height() {
        let barn = StaticCoverObject {
            id: "barn".into(),
            name: "barn".into(),
            kind: StaticCoverKind::FarmBuilding,
            center: [0.0, 3.0, 0.0],
            half_extents_m: [5.0, 3.0, 4.0],
        };
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        append_rubble_mound(&mut vertices, &mut indices, &barn);
        assert!(!vertices.is_empty(), "a rubble mound draws geometry");
        let top = vertices.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
        let intact_top = 3.0 + 3.0;
        // The mound tops out at the sim's rubble height (0.4), well under the standing building.
        assert!(top < intact_top * 0.6, "the mound is low ({top} vs intact top {intact_top})");
        // And it sits on the ground, not floating.
        let bottom = vertices.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min);
        assert!((bottom - 0.0).abs() < 1.0e-3, "the mound rests on the ground, got {bottom}");
    }

    #[test]
    fn a_cleared_tree_line_removes_its_box_and_the_trees_standing_in_it() {
        let map = prokhorovka_hill_252_2();
        let tree_line = map
            .static_cover
            .iter()
            .position(|cover| cover.kind == StaticCoverKind::TreeLine)
            .expect("prokhorovka has a tree line");

        let intact = battlefield_scene_mesh(&map);
        let mut states = vec![0u8; map.static_cover.len()];
        states[tree_line] = 2; // gone
        let cleared = battlefield_scene_mesh_with_cover_states(&map, &states);

        assert!(
            cleared.0.len() < intact.0.len(),
            "clearing a tree line removes geometry ({} vs {})",
            cleared.0.len(),
            intact.0.len()
        );
    }

    #[test]
    fn scenery_only_falls_where_the_cover_it_dressed_is_cleared() {
        let cover = vec![StaticCoverObject {
            id: "hedge".into(),
            name: "hedge".into(),
            kind: StaticCoverKind::TreeLine,
            center: [0.0, 1.0, 0.0],
            half_extents_m: [10.0, 1.0, 1.0],
        }];
        let inside = terrain::SceneryInstance {
            kind: terrain::SceneryKind::Oak,
            position: [3.0, 0.0, 0.5],
            yaw_rad: 0.0,
            scale: 1.0,
        };
        let outside = terrain::SceneryInstance { position: [40.0, 0.0, 0.0], ..inside };

        // Intact: neither tree falls. Gone: only the tree inside the box falls.
        assert!(!scenery_stands_in_cleared_cover(&inside, &cover, &[0]));
        assert!(scenery_stands_in_cleared_cover(&inside, &cover, &[2]));
        assert!(!scenery_stands_in_cleared_cover(&outside, &cover, &[2]));
    }

    /// The other half of the honesty rule: a building may LOOK like walls and a gable roof,
    /// but no visual vertex may leave the collision AABB — nothing on screen can be shot
    /// through or hidden behind that the sim box does not honor.
    #[test]
    fn cover_visuals_never_leave_the_collision_box() {
        for map in [prokhorovka_hill_252_2(), terrain::bystra_valley()] {
            for cover in &map.static_cover {
                let mut vertices = Vec::new();
                let mut indices = Vec::new();
                append_cover_box(&mut vertices, &mut indices, cover);
                let center = Vec3::from_array(cover.center);
                let half = Vec3::from_array(cover.half_extents_m);
                for vertex in &vertices {
                    let delta = (Vec3::from_array(vertex.position) - center).abs();
                    assert!(
                        delta.x <= half.x + 1.0e-3
                            && delta.y <= half.y + 1.0e-3
                            && delta.z <= half.z + 1.0e-3,
                        "cover {} draws outside its collision box at {:?}",
                        cover.id,
                        vertex.position
                    );
                }
            }
        }
    }

    /// Culling honesty: every cover triangle's geometric winding must agree with its authored
    /// normal, or the back-face cull shows the INSIDE of the surface (the gable-roof bug: both
    /// slopes wound inward, so streets saw through the roof into its underside).
    #[test]
    fn every_cover_triangle_winds_outward() {
        for map in [prokhorovka_hill_252_2(), terrain::bystra_valley()] {
            for cover in &map.static_cover {
                let mut vertices = Vec::new();
                let mut indices = Vec::new();
                append_cover_box(&mut vertices, &mut indices, cover);
                for triangle in indices.chunks(3) {
                    let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                        .map(|index| Vec3::from_array(vertices[index as usize].position));
                    let winding = (b - a).cross(c - a);
                    if winding.length_squared() < 1.0e-8 {
                        continue;
                    }
                    let normal = Vec3::from_array(vertices[triangle[0] as usize].normal);
                    assert!(
                        winding.dot(normal) > 0.0,
                        "cover {} has a triangle wound against its normal at {a:?}",
                        cover.id
                    );
                }
            }
        }
    }

    /// The material lane means something: grass stays near-matte, steep rock takes a mineral
    /// sheen, the submerged riverbed is permanently wet, and a slate roof outshines the
    /// plaster wall under it. If everything collapses back to one gloss, materials v2 is off.
    #[test]
    fn material_lane_separates_grass_rock_water_and_roofs() {
        let battlefield = terrain::bystra_valley();
        let (vertices, _) =
            terrain_scene_mesh_with_water(&battlefield.heightmap, battlefield.water);
        let water = battlefield.water.expect("bystra has a river");
        let mut dry_flat_max = 0.0_f32;
        let mut wet_min = f32::MAX;
        for vertex in &vertices {
            let depth = water.depth_over(vertex.position[1]);
            if depth > 0.02 {
                wet_min = wet_min.min(vertex.gloss);
            } else if vertex.normal[1] > 0.995 {
                dry_flat_max = dry_flat_max.max(vertex.gloss);
            }
        }
        assert!(wet_min > dry_flat_max, "riverbed ({wet_min}) must outshine dry grass");
        assert!(dry_flat_max < 0.1, "flat grassland must stay near-matte ({dry_flat_max})");

        // Every authored roof outshines the plaster walls (0.10) below it.
        for id in ["a", "b", "c", "d", "e"] {
            let (_, _, roof_gloss) = building_palette(id);
            assert!(roof_gloss > 0.10, "roof finish must beat the wall for id {id}");
        }
    }

    /// Cover is physical for movement, shells, and the camera; an unrendered cover box is an
    /// invisible wall. Locks that the battlefield mesh draws every static cover object.
    #[test]
    fn battlefield_mesh_renders_every_static_cover_object() {
        let battlefield = prokhorovka_hill_252_2();
        assert!(!battlefield.static_cover.is_empty(), "map should carry static cover");

        let (terrain_vertices, _) = terrain_scene_mesh(&battlefield.heightmap);
        let (vertices, indices) = battlefield_scene_mesh(&battlefield);

        assert!(vertices.len() > terrain_vertices.len(), "cover must add geometry");
        assert!(indices.iter().all(|&index| (index as usize) < vertices.len()));
        for cover in &battlefield.static_cover {
            let center = Vec3::from_array(cover.center);
            let half = Vec3::from_array(cover.half_extents_m);
            let rendered = vertices.iter().any(|vertex| {
                let delta = (Vec3::from_array(vertex.position) - center).abs();
                delta.x <= half.x + 1.0e-3
                    && delta.y <= half.y + 1.0e-3
                    && delta.z <= half.z + 1.0e-3
            });
            assert!(rendered, "static cover {} must be part of the battlefield mesh", cover.id);
        }
    }
}
