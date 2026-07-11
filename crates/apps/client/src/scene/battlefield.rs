use glam::{Mat3, Vec3};
use renderer_api::SceneVertex;
use terrain::{
    BattlefieldMap, HeightMap, Road, RoadSurface, StaticCoverKind, StaticCoverObject, WaterBody,
};

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
    let (ground, statics) = battlefield_ground_and_statics_meshes(battlefield, cover_states);
    let (mut vertices, mut indices) = ground;
    let base = vertices.len() as u32;
    vertices.extend(statics.0);
    indices.extend(statics.1.into_iter().map(|index| index + base));
    (vertices, indices)
}

/// One indexed scene mesh: vertices plus triangle indices.
pub type SceneMeshData = (Vec<SceneVertex>, Vec<u32>);

/// The battlefield split for Terrain Material 2.0: the GROUND (the heightfield the terrain
/// pipeline shades with splat layers + macro normals) separately from the STATICS (cover,
/// backdrop skirt, scenery — the generic scene pipeline). Same content as
/// [`battlefield_scene_mesh_with_cover_states`], split at the pipeline seam.
pub fn battlefield_ground_and_statics_meshes(
    battlefield: &BattlefieldMap,
    cover_states: &[u8],
) -> (SceneMeshData, SceneMeshData) {
    let ground =
        terrain_scene_mesh_full(&battlefield.heightmap, battlefield.water, &battlefield.roads);
    (ground, battlefield_statics_mesh(battlefield, cover_states))
}

/// The statics alone (cover, backdrop skirt, scenery) — what a cover-state change rebuilds.
/// The ground and its baked maps never depend on cover phases, so a collapsing building costs
/// only this mesh, never a 1024^2 map rebake.
pub fn battlefield_statics_mesh(
    battlefield: &BattlefieldMap,
    cover_states: &[u8],
) -> (Vec<SceneVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
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
    // The base slab: lower than the sim mound, the settled mass the chunks poke out of.
    let slab_half_y = mound_half_y * 0.55;
    let slab_center = Vec3::new(center.x, ground_y + slab_half_y, center.z);
    let slab_half = Vec3::new(half.x * 0.9, slab_half_y, half.z * 0.9);
    // Dull broken masonry: grey-brown, matte.
    push_surfaced_box(vertices, indices, slab_center, slab_half, [0.38, 0.34, 0.30], 0.04);

    // Broken slabs and wall fragments, tilted in plan, seeded from the building id so the same
    // ruin always collapses the same way. Every chunk stays inside the collision AABB and under
    // the sim's rubble top: what the eye reads as the pile is what a hull stops against.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in cover.id.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    let mut next = move || {
        hash ^= hash << 13;
        hash ^= hash >> 7;
        hash ^= hash << 17;
        (hash >> 40) as f32 / ((1u64 << 24) - 1) as f32
    };
    let chunk_tones = [[0.42, 0.38, 0.33], [0.34, 0.30, 0.26], [0.45, 0.40, 0.33]];
    let count = 4 + (next() * 3.0) as usize;
    for index in 0..count {
        let plan = Vec3::new(half.x, 0.0, half.z);
        let offset = Vec3::new((next() - 0.5) * 1.3, 0.0, (next() - 0.5) * 1.3) * plan * 0.52;
        let chunk_half = Vec3::new(
            (0.10 + next() * 0.12) * half.x.max(1.0),
            mound_half_y * (0.35 + next() * 0.2),
            (0.10 + next() * 0.12) * half.z.max(1.0),
        );
        let chunk_center = Vec3::new(
            center.x + offset.x,
            ground_y + slab_half_y * 2.0 + chunk_half.y * (0.2 + next() * 0.4) - chunk_half.y,
            center.z + offset.z,
        );
        let yaw = next() * std::f32::consts::TAU;
        let start = vertices.len();
        push_oriented_box(
            vertices,
            indices,
            chunk_center,
            chunk_half,
            Mat3::from_rotation_y(yaw),
            chunk_tones[index % chunk_tones.len()],
        );
        for vertex in &mut vertices[start..] {
            vertex.gloss = 0.05;
        }
    }
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
    // The wall body recesses a few centimetres so windows and the door can sit proud of the
    // PLASTER while every vertex stays inside the collision AABB — the honesty rule holds.
    let wall_half = Vec3::new(half.x - WALL_RECESS_M, half.y, half.z - WALL_RECESS_M);

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
        Vec3::new(wall_half.x, (eaves_y - plinth_y) * 0.5, wall_half.z),
        wall,
        0.10,
    );
    append_joinery(vertices, indices, center, half, plinth_y, eaves_y);
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

/// How far the plaster wall sits inside the collision box, making room for the joinery.
const WALL_RECESS_M: f32 = 0.04;
/// Window glass: near-black with a glazed sheen — the one thing on a wall that answers the sky.
const WINDOW: ([f32; 3], f32) = ([0.07, 0.09, 0.11], 0.45);
/// Plank door: dark weathered timber, matte.
const DOOR: ([f32; 3], f32) = ([0.16, 0.11, 0.07], 0.06);

/// Windows along both long walls and a door on one gable end — flat quads floating just
/// outside the recessed wall (still inside the collision AABB), so a house reads as a house
/// and not a plastered crate. The layout is pure geometry, identical for a given box.
fn append_joinery(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: Vec3,
    half: Vec3,
    plinth_y: f32,
    eaves_y: f32,
) {
    let along_x = half.x >= half.z;
    let (long_axis, side_axis) = if along_x { (Vec3::X, Vec3::Z) } else { (Vec3::Z, Vec3::X) };
    let long_half = if along_x { half.x } else { half.z };
    let side_half = if along_x { half.z } else { half.x };
    let face_offset = side_half - WALL_RECESS_M * 0.5;

    // Windows: a metre-rhythm row under the eaves on both long faces.
    let count = ((long_half * 2.0 - 1.6) / 2.4).floor().max(1.0) as i32;
    let window_half_w = 0.42;
    let window_half_h = ((eaves_y - plinth_y) * 0.22).clamp(0.25, 0.5);
    let window_y = plinth_y + (eaves_y - plinth_y) * 0.58;
    for sign in [-1.0_f32, 1.0] {
        for index in 0..count {
            let t = (index as f32 + 0.5) / count as f32 - 0.5;
            let along = t * (long_half * 2.0 - 1.6);
            let position = center + long_axis * along + side_axis * face_offset * sign
                - Vec3::Y * (center.y - window_y);
            push_face_quad(
                vertices,
                indices,
                position,
                long_axis * window_half_w,
                Vec3::Y * window_half_h,
                side_axis * sign,
                WINDOW,
            );
        }
    }

    // The door: one gable end, grounded on the plinth.
    let door_half_h = ((eaves_y - (center.y - half.y)) * 0.5 * 0.62).clamp(0.6, 1.05);
    let door_face = long_half - WALL_RECESS_M * 0.5;
    let position = center + long_axis * door_face - Vec3::Y * (half.y - door_half_h);
    push_face_quad(
        vertices,
        indices,
        position,
        side_axis * 0.48,
        Vec3::Y * door_half_h,
        long_axis,
        DOOR,
    );
}

/// A flat rectangle on a wall plane: `center ± u ± v`, facing `normal` (unit axis), wound to it.
fn push_face_quad(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: Vec3,
    u: Vec3,
    v: Vec3,
    normal: Vec3,
    (color, gloss): ([f32; 3], f32),
) {
    let start = vertices.len() as u32;
    let corners = [center - u - v, center + u - v, center + u + v, center - u + v];
    for corner in corners {
        vertices.push(SceneVertex::surfaced(corner.to_array(), normal.to_array(), color, gloss));
    }
    push_winding(indices, start, &[0, 1, 2, 0, 2, 3], {
        (corners[1] - corners[0]).cross(corners[2] - corners[0]).dot(normal) > 0.0
    });
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
    terrain_scene_mesh_full(heightmap, water, &[])
}

/// The full terrain surface: height/slope base color, the grass patchwork, painted roads,
/// then the water tint — later layers win where they overlap.
fn terrain_scene_mesh_full(
    heightmap: &HeightMap,
    water: Option<WaterBody>,
    roads: &[Road],
) -> (Vec<SceneVertex>, Vec<u32>) {
    let w = heightmap.width();
    let h = heightmap.height();
    let cell = heightmap.cell_size_m();
    let stats = heightmap.stats();

    let mut vertices = Vec::with_capacity(w * h);
    for z in 0..h {
        for x in 0..w {
            let y = heightmap.sample_at_index(x, z);
            let (wx, wz) = (x as f32 * cell, z as f32 * cell);
            let normal = vertex_normal(heightmap, x, z, cell);
            let mut color = terrain_color(y, stats.min_m, stats.max_m, normal.y, wx, wz);
            // Grass is near-matte; exposed rock on steep faces takes a mineral sheen; the
            // riverbed under water is permanently wet and reads glossiest of all.
            let mut gloss = 0.03 + (1.0 - normal.y).clamp(0.0, 1.0) * 0.12;
            if let Some((tone, road_gloss, blend)) = road_paint(roads, wx, wz) {
                color = Vec3::from_array(color).lerp(tone, blend).to_array();
                gloss = gloss + (road_gloss - gloss) * blend;
            }
            // The ground pipeline reads its albedo from the splat layers; the vertex colour
            // wins only where the tint lane says so — the submerged riverbed, whose depth
            // tint has no splat equivalent. Dry ground carries 0 (splat rules).
            let mut vertex_color_dominance = 0.0;
            if let Some(water) = water {
                let depth = water.depth_over(y);
                color = water_tint(color, depth);
                if depth > 0.02 {
                    gloss = 0.35;
                    vertex_color_dominance = (depth / 0.35).clamp(0.35, 1.0);
                }
            }
            vertices.push(SceneVertex {
                position: [wx, y, wz],
                normal: normal.to_array(),
                color,
                tint_weight: vertex_color_dominance,
                gloss,
            });
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

fn terrain_color(y: f32, min_y: f32, max_y: f32, normal_y: f32, wx: f32, wz: f32) -> [f32; 3] {
    let span = (max_y - min_y).max(1.0);
    let t = ((y - min_y) / span).clamp(0.0, 1.0);
    // The living grass is a patchwork, not a lawn: broad drifts of sun-dried straw and
    // deeper lush pockets over the base green, driven by low-frequency world noise.
    let grass = grass_patchwork(wx, wz);
    let rock = Vec3::new(0.46, 0.41, 0.34);
    let mut color = grass.lerp(rock, t * t);
    // Steep faces drift toward bare rock so slopes read as relief, not flat shading.
    let steep = (1.0 - normal_y).clamp(0.0, 1.0);
    color = color.lerp(Vec3::new(0.33, 0.29, 0.26), steep * 0.6);
    color.to_array()
}

/// The grass endpoint of the terrain palette at a world point: base green pushed toward
/// dry straw where the broad noise runs high and toward a lush pocket where it runs low,
/// with a second octave breaking the drift edges up. Deterministic — the same point always
/// grows the same grass.
fn grass_patchwork(wx: f32, wz: f32) -> Vec3 {
    let base = Vec3::new(0.26, 0.44, 0.20);
    let dry = Vec3::new(0.40, 0.40, 0.21);
    let lush = Vec3::new(0.18, 0.36, 0.15);
    let n = grass_patchwork_noise(wx, wz);
    if n > 0.5 {
        base.lerp(dry, ((n - 0.5) * 2.4).min(1.0))
    } else {
        base.lerp(lush, ((0.5 - n) * 2.4).min(1.0) * 0.85)
    }
}

/// The patchwork drift noise itself, shared with the splat bake so the per-pixel grass/straw
/// split lands exactly where the old vertex palette drifted: broad drifts (~65 m) shaped by a
/// finer octave (~19 m), deterministic in the world point.
pub(crate) fn grass_patchwork_noise(wx: f32, wz: f32) -> f32 {
    value_noise(wx / 65.0, wz / 65.0) * 0.72 + value_noise(wx / 19.0, wz / 19.0) * 0.28
}

/// The strongest road-paint blend at a world point, 0 where no road reaches — the splat bake's
/// dirt-layer source, sharing `road_paint`'s feathering exactly.
pub(crate) fn road_blend_at(roads: &[Road], wx: f32, wz: f32) -> f32 {
    road_paint(roads, wx, wz).map(|(_, _, blend)| blend).unwrap_or(0.0)
}

/// Deterministic 2D value noise in [0, 1]: hashed lattice corners, smoothstep-blended.
fn value_noise(x: f32, z: f32) -> f32 {
    let (x0, z0) = (x.floor(), z.floor());
    let (fx, fz) = (x - x0, z - z0);
    let (sx, sz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let corner = |dx: f32, dz: f32| -> f32 {
        let (ix, iz) = ((x0 + dx) as i64, (z0 + dz) as i64);
        // splitmix64 over the packed lattice coordinates.
        let mut h = (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (iz as u64).rotate_left(32);
        h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((h ^ (h >> 31)) >> 40) as f32 / ((1u64 << 24) - 1) as f32
    };
    let top = corner(0.0, 0.0) + (corner(1.0, 0.0) - corner(0.0, 0.0)) * sx;
    let bottom = corner(0.0, 1.0) + (corner(1.0, 1.0) - corner(0.0, 1.0)) * sx;
    top + (bottom - top) * sz
}

/// The road tone at a world point, if any road reaches it: `(tone, gloss, blend)` with the
/// blend feathering from full paint over the core to nothing at the authored edge.
fn road_paint(roads: &[Road], wx: f32, wz: f32) -> Option<(Vec3, f32, f32)> {
    let mut best: Option<(Vec3, f32, f32)> = None;
    for road in roads {
        let half = road.width_m * 0.5;
        let distance = road.distance_to(wx, wz);
        if distance >= half {
            continue;
        }
        // Full tone over the inner core, feathered out to the grass at the edge.
        let fade = ((half - distance) / (half * 0.45)).clamp(0.0, 1.0);
        let blend = fade * fade * (3.0 - 2.0 * fade);
        let (tone, gloss) = match road.surface {
            RoadSurface::Dirt => (Vec3::new(0.40, 0.34, 0.24), 0.05),
            RoadSurface::Ballast => (Vec3::new(0.34, 0.31, 0.28), 0.08),
        };
        if best.map(|(_, _, b)| blend > b).unwrap_or(true) {
            best = Some((tone, gloss, blend));
        }
    }
    best
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
        // A pile, not a crate: the slab plus tilted chunks — and every chunk inside the box.
        assert!(vertices.len() > 24, "rubble reads as broken chunks, got {}", vertices.len());
        for vertex in &vertices {
            assert!(
                (vertex.position[0] - 0.0).abs() <= 5.0 + 1.0e-3
                    && (vertex.position[2] - 0.0).abs() <= 4.0 + 1.0e-3,
                "rubble stays inside the collision footprint, got {:?}",
                vertex.position
            );
        }
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

    /// A farm building carries its joinery: window glass (glazed, outshining the plaster) and
    /// a plank door, all inside the collision AABB (the walls recess to make the room).
    #[test]
    fn buildings_wear_windows_and_a_door() {
        let map = prokhorovka_hill_252_2();
        let barn = map
            .static_cover
            .iter()
            .find(|c| c.kind == StaticCoverKind::FarmBuilding)
            .expect("prokhorovka has barns");
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        append_cover_box(&mut vertices, &mut indices, barn);
        let windows = vertices.iter().filter(|v| v.color == WINDOW.0).count();
        let doors = vertices.iter().filter(|v| v.color == DOOR.0).count();
        assert!(windows >= 8, "a barn wall carries windows, got {windows} verts");
        assert_eq!(doors, 4, "one door on a gable end");
        // Glass answers the sky harder than the plaster around it.
        assert!(WINDOW.1 > 0.10, "window glaze outshines the wall");
    }

    /// The steppe roads are painted ground, not decals: a vertex on a dirt road reads as
    /// earth (red over green), a vertex in the open grass reads as grass (green over red),
    /// and neither breaks the near-matte bound the material lane promises for dry ground.
    #[test]
    fn roads_paint_worn_earth_into_the_grass() {
        let map = prokhorovka_hill_252_2();
        let dirt = map
            .static_cover
            .iter()
            .find(|c| c.id == "oktyabrskiy_barn_south")
            .map(|_| ())
            .and_then(|_| map.roads.iter().find(|road| road.id == "farm_road_south"))
            .expect("prokhorovka authors a farm road");
        let (vertices, _) = battlefield_scene_mesh(&map);

        // The vertex nearest the road's first waypoint is painted dirt; a probe pulled 40 m
        // to the side keeps its grass.
        let probe_on = dirt.points[1];
        let probe_off = [probe_on[0], probe_on[1] - 40.0];
        let nearest = |probe: [f32; 2]| {
            vertices
                .iter()
                .filter(|v| v.tint_weight == 0.0)
                .min_by(|a, b| {
                    let da =
                        (a.position[0] - probe[0]).powi(2) + (a.position[2] - probe[1]).powi(2);
                    let db =
                        (b.position[0] - probe[0]).powi(2) + (b.position[2] - probe[1]).powi(2);
                    da.partial_cmp(&db).unwrap()
                })
                .expect("terrain has vertices")
        };
        let on_road = nearest(probe_on);
        let off_road = nearest(probe_off);
        assert!(
            on_road.color[0] > on_road.color[1],
            "on-road vertex must read as earth, got {:?}",
            on_road.color
        );
        assert!(
            off_road.color[1] > off_road.color[0],
            "off-road vertex must stay grass, got {:?}",
            off_road.color
        );
        assert!(on_road.gloss < 0.1, "a dirt road stays matte, got {}", on_road.gloss);
    }

    /// The grass is a patchwork, not a lawn: across the open steppe the green varies by
    /// visible drifts, deterministically — the same map builds the same field every time.
    #[test]
    fn grass_patchwork_varies_and_is_deterministic() {
        let map = prokhorovka_hill_252_2();
        let (first, _) = terrain_scene_mesh(&map.heightmap);
        let (second, _) = terrain_scene_mesh(&map.heightmap);
        assert_eq!(first.len(), second.len());
        assert!(
            first.iter().zip(&second).all(|(a, b)| a.color == b.color),
            "the patchwork must be deterministic"
        );

        let greens: Vec<f32> = first
            .iter()
            .filter(|v| v.normal[1] > 0.995 && v.position[1] < 12.0)
            .map(|v| v.color[1])
            .collect();
        assert!(greens.len() > 100, "the steppe has flat grassland");
        let (lo, hi) =
            greens.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &g| (lo.min(g), hi.max(g)));
        assert!(
            hi - lo > 0.03,
            "flat grass must vary between dry and lush drifts (spread {})",
            hi - lo
        );
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
