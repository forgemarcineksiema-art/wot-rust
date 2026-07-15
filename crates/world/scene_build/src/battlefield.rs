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
    (battlefield_ground_mesh(battlefield), battlefield_statics_mesh(battlefield, cover_states))
}

/// The ground alone — what a crater-ledger change rebuilds (protocol v31). The heightmap's
/// crater overlay is already folded into `sample_height`, so the re-mesh below simply reads the
/// same deformed truth physics stands on; the baked splat/macro maps never depend on craters.
pub fn battlefield_ground_mesh(battlefield: &BattlefieldMap) -> SceneMeshData {
    terrain_scene_mesh_full(&battlefield.heightmap, battlefield.water, &battlefield.roads)
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
        let (skirt_vertices, skirt_indices) = crate::backdrop::backdrop_scene_mesh(battlefield);
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
        crate::foliage::push_scenery_instance(&mut vertices, &mut indices, instance);
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

/// A building inside its box — FORGED (B4): the world_forge generator picks a style from the
/// box's proportions (a long low box is a barn, a tall one a townhouse, the rest cottages),
/// seeds the joinery from the building id, and the bake is scaled per axis into the collision
/// AABB — the generator's own honesty lock guarantees every vertex stays inside, so the rule
/// "what blocks the shell blocks the eye" survives the swap. Palette stays per id: a town is a
/// town, not a barracks.
fn append_building(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    cover: &StaticCoverObject,
    center: Vec3,
    half: Vec3,
) {
    let (wall, roof, roof_gloss) = building_palette(&cover.id);
    let mut seed = 0xcbf2_9ce4_8422_2325_u64;
    for byte in cover.id.bytes() {
        seed ^= u64::from(byte);
        seed = seed.wrapping_mul(0x0100_0000_01b3);
    }
    // Landmarks stand by NAME (B4 cz.2) — one church, one windmill per map; every other
    // building keeps reading its style off the collision box's proportions.
    let elongation = half.x.max(half.z) / half.x.min(half.z).max(0.1);
    let style = if cover.id.contains("church") {
        world_forge::building::BuildingStyle::Church
    } else if cover.id.contains("windmill") {
        world_forge::building::BuildingStyle::Windmill
    } else if elongation > 1.45 && half.y < 2.9 {
        world_forge::building::BuildingStyle::Barn
    } else if half.y >= 2.9 {
        world_forge::building::BuildingStyle::Townhouse
    } else {
        world_forge::building::BuildingStyle::Cottage
    };
    let baked = world_forge::building::bake_building(
        style,
        seed,
        world_forge::building::StructureForm::Intact,
    );
    // The generator's ridge runs +Z; run it down the box's LONG axis like the roofs always did.
    let rotate = half.x > half.z;
    let footprint = baked.footprint_half;
    let ground = Vec3::new(center.x, center.y - half.y, center.z);
    let scale = if rotate {
        Vec3::new(half.z / footprint.x, half.y / footprint.y, half.x / footprint.z)
    } else {
        Vec3::new(half.x / footprint.x, half.y / footprint.y, half.z / footprint.z)
    };
    let plinth = ([0.24_f32, 0.22, 0.20], 0.15_f32);
    for (mesh, is_roof) in [(&baked.walls, false), (&baked.roof, true)] {
        let base = vertices.len() as u32;
        for vertex in mesh.vertices() {
            let scaled = vertex.position * scale;
            // A true 90-degree rotation (det +1), never an axis swap: a swap is a REFLECTION
            // and would wind every triangle inside-out.
            let local = if rotate { Vec3::new(scaled.z, scaled.y, -scaled.x) } else { scaled };
            let n = vertex.normal / scale;
            let n = if rotate { Vec3::new(n.z, n.y, -n.x) } else { n };
            // Colour names the palette; the surface-role lane names the MATERIAL the scene
            // shader dresses it in (Materia Świata 3): rendered walls, coursed roofs, plank
            // doors. Glass and plinth stone keep the untreated look.
            use renderer_api::surface_role;
            let (color, gloss, role) = match (is_roof, vertex.material) {
                (true, vehicle_geometry::MaterialRole::CastArmor) => {
                    (roof, roof_gloss, surface_role::SLATE)
                }
                (true, _) => (wall, 0.10, surface_role::PLASTER),
                (false, vehicle_geometry::MaterialRole::CastArmor) => {
                    (plinth.0, plinth.1, surface_role::LEGACY)
                }
                (false, vehicle_geometry::MaterialRole::InteriorMachinery) => {
                    (WINDOW.0, WINDOW.1, surface_role::LEGACY)
                }
                (false, vehicle_geometry::MaterialRole::InteriorPrimer) => {
                    (DOOR.0, DOOR.1, surface_role::PLANK)
                }
                (false, _) if style == world_forge::building::BuildingStyle::Windmill => {
                    // A windmill is timber-clad: dark boards, not render.
                    ([0.30, 0.25, 0.19], 0.06, surface_role::PLANK)
                }
                (false, _) => (wall, 0.10, surface_role::PLASTER),
            };
            let scene_vertex = SceneVertex::surfaced(
                (ground + local).to_array(),
                n.normalize_or_zero().to_array(),
                color,
                gloss,
            )
            .with_surface(role);
            vertices.push(scene_vertex);
        }
        indices.extend(mesh.indices().iter().map(|index| index + base));
    }
}

/// Window glass: near-black with a glazed sheen — the one thing on a wall that answers the sky.
const WINDOW: ([f32; 3], f32) = ([0.07, 0.09, 0.11], 0.45);
/// Plank door: dark weathered timber, matte.
const DOOR: ([f32; 3], f32) = ([0.16, 0.11, 0.07], 0.06);

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
///
/// True deformation (protocol v31): cells inside a crater's influence are CUT from the base
/// grid and re-meshed at sub-cell resolution from `sample_height` — the exact deformed truth
/// physics stands on. On a cell edge the bilinear sample degenerates to the linear lerp the
/// neighbouring base triangles draw, and the crater delta is zero at every cut boundary (an
/// uncut cell is untouched by construction), so the patchwork is watertight with no stitching.
fn terrain_scene_mesh_full(
    heightmap: &HeightMap,
    water: Option<WaterBody>,
    roads: &[Road],
) -> (Vec<SceneVertex>, Vec<u32>) {
    let w = heightmap.width();
    let h = heightmap.height();
    let cell = heightmap.cell_size_m();
    let stats = heightmap.stats();

    let make_vertex = |wx: f32, wz: f32, y: f32, normal: Vec3| -> SceneVertex {
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
        SceneVertex {
            position: [wx, y, wz],
            normal: normal.to_array(),
            color,
            tint_weight: vertex_color_dominance,
            gloss,
            surface: 0.0,
            sway: 0.0,
        }
    };

    let mut vertices = Vec::with_capacity(w * h);
    for z in 0..h {
        for x in 0..w {
            let y = heightmap.sample_at_index(x, z);
            let (wx, wz) = (x as f32 * cell, z as f32 * cell);
            let normal = vertex_normal(heightmap, x, z, cell);
            vertices.push(make_vertex(wx, wz, y, normal));
        }
    }

    let cut = cratered_cells(heightmap);
    let mut indices = Vec::with_capacity((w - 1) * (h - 1) * 6);
    for z in 0..h - 1 {
        for x in 0..w - 1 {
            if cut.contains(&(x, z)) {
                continue;
            }
            let i = (z * w + x) as u32;
            let right = i + 1;
            let down = i + w as u32;
            indices.extend_from_slice(&[i, down, right, right, down, down + 1]);
        }
    }
    for &(x, z) in &cut {
        append_crater_cell(&mut vertices, &mut indices, heightmap, x, z, &make_vertex);
    }
    (vertices, indices)
}

/// Sub-cell resolution of a crater patch: 8 subdivisions of the 5 m battlefield cell give a
/// 0.625 m mesh step — fine enough for the smallest ledger crater (0.8 m radius bowl).
const CRATER_CELL_SUBDIVISIONS: usize = 8;

/// The base-grid cells whose ground any crater in the ledger touches (influence AABB overlap),
/// in deterministic order. These are cut from the coarse grid and re-meshed finely.
fn cratered_cells(heightmap: &HeightMap) -> std::collections::BTreeSet<(usize, usize)> {
    let mut cut = std::collections::BTreeSet::new();
    let cell = heightmap.cell_size_m();
    let max_x = heightmap.width() - 2;
    let max_z = heightmap.height() - 2;
    for record in heightmap.crater_records() {
        let reach = record.influence_radius_m();
        let lo_x = (((record.x_m() - reach) / cell).floor().max(0.0) as usize).min(max_x);
        let hi_x = (((record.x_m() + reach) / cell).floor().max(0.0) as usize).min(max_x);
        let lo_z = (((record.z_m() - reach) / cell).floor().max(0.0) as usize).min(max_z);
        let hi_z = (((record.z_m() + reach) / cell).floor().max(0.0) as usize).min(max_z);
        for z in lo_z..=hi_z {
            for x in lo_x..=hi_x {
                cut.insert((x, z));
            }
        }
    }
    cut
}

/// Re-mesh ONE cut cell at sub-cell resolution, heights and normals read from `sample_height`
/// — the same deformed ground the sim's physics, spotting and predictor stand on.
fn append_crater_cell(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    heightmap: &HeightMap,
    cell_x: usize,
    cell_z: usize,
    make_vertex: &impl Fn(f32, f32, f32, Vec3) -> SceneVertex,
) {
    let cell = heightmap.cell_size_m();
    let sub = CRATER_CELL_SUBDIVISIONS;
    let step = cell / sub as f32;
    let base = vertices.len() as u32;
    let sample = |wx: f32, wz: f32| -> f32 {
        let [ex, ez] = heightmap.extent_m();
        heightmap
            .sample_height(wx.clamp(0.0, ex), wz.clamp(0.0, ez))
            .expect("clamped sample stays in domain")
    };
    for sz in 0..=sub {
        for sx in 0..=sub {
            let wx = cell_x as f32 * cell + sx as f32 * step;
            let wz = cell_z as f32 * cell + sz as f32 * step;
            let y = sample(wx, wz);
            let normal = Vec3::new(
                sample(wx - step, wz) - sample(wx + step, wz),
                2.0 * step,
                sample(wx, wz - step) - sample(wx, wz + step),
            )
            .normalize();
            vertices.push(make_vertex(wx, wz, y, normal));
        }
    }
    let row = (sub + 1) as u32;
    for sz in 0..sub as u32 {
        for sx in 0..sub as u32 {
            let i = base + sz * row + sx;
            let right = i + 1;
            let down = i + row;
            indices.extend_from_slice(&[i, down, right, right, down, down + 1]);
        }
    }
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
    fn a_crater_re_meshes_the_ground_the_physics_stands_in() {
        let mut flat = HeightMap::flat(64, 64, 5.0, 10.0).expect("flat map");
        let crater = terrain::CraterRecord::from_world(
            150.0,
            150.0,
            2.2,
            0.8,
            terrain::CRATER_KIND_HIGH_EXPLOSIVE,
        );
        flat.set_craters(&[crater]);

        let (vertices, indices) = terrain_scene_mesh(&flat);

        // The bowl floor is genuinely sunk in the render mesh — the eye sees the same hole
        // the tracks stand in (sub-cell patch resolution, so the full depth is reached).
        let lowest = vertices.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min);
        assert!(
            (lowest - (10.0 - crater.depth_m())).abs() < 0.05,
            "the mesh reaches the bowl floor: {lowest}"
        );
        // The spoil rim rises above grade.
        let highest = vertices.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
        assert!(highest > 10.0 + crater.depth_m() * 0.15, "the rim shows: {highest}");
        assert_eq!(indices.len() % 3, 0);
    }

    #[test]
    fn virgin_ground_meshes_exactly_as_before_deformation_existed() {
        let map = prokhorovka_hill_252_2();
        let (vertices, indices) = terrain_scene_mesh(&map.heightmap);
        let mut with_empty_ledger = map.heightmap.clone();
        with_empty_ledger.set_craters(&[]);
        let (twin_vertices, twin_indices) = terrain_scene_mesh(&with_empty_ledger);
        assert!(vertices == twin_vertices && indices == twin_indices);
    }

    #[test]
    fn the_crater_patchwork_is_watertight_at_its_seams() {
        // A sloped field, so a seam mismatch cannot hide behind flatness: every fine-patch
        // vertex on a cut-cell boundary must land exactly on the base grid's linear edge.
        let cell = 5.0;
        let samples: Vec<f32> =
            (0..64 * 64).map(|i| (i % 64) as f32 * 0.35 + (i / 64) as f32 * 0.2).collect();
        let mut sloped = HeightMap::new(64, 64, cell, samples).expect("sloped map");
        let crater = terrain::CraterRecord::from_world(
            150.0,
            150.0,
            2.2,
            0.8,
            terrain::CRATER_KIND_HIGH_EXPLOSIVE,
        );
        sloped.set_craters(&[crater]);

        let (vertices, _) = terrain_scene_mesh(&sloped);
        let reach = crater.influence_radius_m();
        for vertex in &vertices {
            let [wx, y, wz] = vertex.position;
            let dx = wx - crater.x_m();
            let dz = wz - crater.z_m();
            if dx * dx + dz * dz >= reach * reach {
                // Outside the crater's influence EVERY vertex — base or patch — sits on the
                // authored bilinear surface; on cell edges that is the base triangle edge.
                let expected = sloped.sample_height(wx, wz).expect("in domain");
                assert!(
                    (y - expected).abs() < 1.0e-4,
                    "no lips or cracks at the patch boundary: {y} vs {expected} at ({wx},{wz})"
                );
            }
        }
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
        assert!(doors >= 4, "a door stands proud of the plaster, got {doors} verts");
        // Glass answers the sky harder than the plaster around it.
        assert!(WINDOW.1 > 0.10, "window glaze outshines the wall");
    }

    /// Materia Świata 3: a building names its materials down the surface lane — rendered
    /// walls, coursed roofs, a plank door — while glass keeps the untreated look. The lane
    /// is what the scene shader dispatches its procedural detail on.
    #[test]
    fn buildings_name_their_surfaces_down_the_lane() {
        use renderer_api::surface_role;
        let map = prokhorovka_hill_252_2();
        let barn = map
            .static_cover
            .iter()
            .find(|c| c.kind == StaticCoverKind::FarmBuilding)
            .expect("prokhorovka has barns");
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        append_cover_box(&mut vertices, &mut indices, barn);
        let count = |role: f32| vertices.iter().filter(|v| (v.surface - role).abs() < 0.01).count();
        assert!(count(surface_role::PLASTER) > 0, "walls wear plaster");
        assert!(count(surface_role::SLATE) > 0, "the roof runs in courses");
        assert!(count(surface_role::PLANK) >= 4, "the door is sawn boards");
        for vertex in &vertices {
            if vertex.color == WINDOW.0 {
                assert_eq!(vertex.surface, surface_role::LEGACY, "glass takes no treatment");
            }
        }
    }

    /// B4 cz.2: the named landmarks stand as themselves — the church's spire is the tallest
    /// point in Kamienna (well above any townhouse ridge), and the windmill wears timber
    /// boards where every rendered wall wears plaster.
    #[test]
    fn the_church_towers_over_town_and_the_windmill_wears_timber() {
        use renderer_api::surface_role;
        let map = terrain::bystra_valley();
        let bake = |id: &str| {
            let cover = map
                .static_cover
                .iter()
                .find(|c| c.id.contains(id))
                .unwrap_or_else(|| panic!("bystra has {id}"));
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            append_cover_box(&mut vertices, &mut indices, cover);
            (cover, vertices)
        };

        let (church_cover, church) = bake("church");
        let church_top = church.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
        let cover_top = church_cover.center[1] + church_cover.half_extents_m[1];
        assert!(
            church_top > cover_top - 1.0,
            "the spire fills its collision box: mesh top {church_top} vs box top {cover_top}"
        );

        let (_, windmill) = bake("windmill");
        let planks =
            windmill.iter().filter(|v| (v.surface - surface_role::PLANK).abs() < 0.01).count();
        assert!(planks > windmill.len() / 3, "the windmill body is timber-clad");
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
