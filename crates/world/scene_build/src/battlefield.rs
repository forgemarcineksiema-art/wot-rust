use glam::{Mat3, Vec3};
use renderer_api::SceneVertex;
use terrain::{
    BattlefieldMap, HeightMap, Road, RoadSurface, StaticCoverKind, StaticCoverObject, WaterView,
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
/// Includes the border apron: the same ground pipeline continued beyond the red line, so the
/// world past the border is more steppe melting into the haze, not a different game.
pub fn battlefield_ground_mesh(battlefield: &BattlefieldMap) -> SceneMeshData {
    let beyond = beyond_border_height(battlefield);
    terrain_scene_mesh_full(
        &battlefield.heightmap,
        battlefield.water_view(),
        &battlefield.roads,
        Some(beyond.as_ref()),
    )
}

/// The analytic ground continuation the border apron stands on: a shipped (catalog) map
/// exposes the very terrain program its heightmap samples plus its horizon enclosure, so the
/// apron is exact at the border by construction; anything else falls back to clamped edge
/// heights (a flat continuation).
fn beyond_border_height(battlefield: &BattlefieldMap) -> Box<dyn Fn(f32, f32) -> f32 + '_> {
    if let Some(blueprint) = map_forge::cached_blueprint_by_id(&battlefield.id) {
        return Box::new(move |x, z| map_forge::backdrop_height(blueprint, x, z));
    }
    Box::new(|x, z| {
        let [extent_x, extent_z] = battlefield.heightmap.extent_m();
        battlefield
            .heightmap
            .sample_height(x.clamp(0.0, extent_x), z.clamp(0.0, extent_z))
            .unwrap_or(0.0)
    })
}

/// The statics alone (cover, backdrop skirt, scenery) — what a cover-state change rebuilds.
/// The ground and its baked maps never depend on cover phases, so a collapsing building costs
/// only this mesh, never a 1024^2 map rebake.
pub fn battlefield_statics_mesh(
    battlefield: &BattlefieldMap,
    cover_states: &[u8],
) -> (Vec<SceneVertex>, Vec<u32>) {
    battlefield_statics_mesh_with_scars(battlefield, cover_states, &[])
}

/// As [`battlefield_statics_mesh`], dressing each still-standing cover face with its
/// replicated shell wounds (protocol v32): a kinetic inset with its plaster burst, an HE bite
/// with rubble spilled at the wall's foot. Rubble mounds and cleared objects drop their scars
/// with the wall that carried them.
pub fn battlefield_statics_mesh_with_scars(
    battlefield: &BattlefieldMap,
    cover_states: &[u8],
    cover_scars: &[terrain::CoverScar],
) -> (Vec<SceneVertex>, Vec<u32>) {
    assemble_statics_mesh(&battlefield_statics_buckets(battlefield, cover_states, cover_scars))
}

/// How many worker lanes a scene bake may spread `items` of work across.
///
/// One core is deliberately left to whoever asked for the bake: these bakes run while the
/// garage is drawing a live 3D scene, and taking every core to shorten a background job by a
/// few percent buys the stall back as dropped frames on the screen the player is looking at.
pub(crate) fn bake_lane_count(items: usize) -> usize {
    /// Past a handful of lanes the bake stops being the bottleneck and starts competing.
    const MAX_BAKE_LANES: usize = 8;
    std::thread::available_parallelism()
        .map_or(1, |cores| cores.get().saturating_sub(1))
        .clamp(1, MAX_BAKE_LANES)
        .min(items.max(1))
}

/// `items` split into one contiguous `[start, end)` range per worker lane, in order.
///
/// The plan tiles `0..items` exactly once — no gap leaves work undone, no overlap makes the
/// concatenation longer than the thing being baked — so a caller may run the ranges on threads
/// and glue the results back together in range order without the split showing in the output.
/// Shared by the ground-map bake (rows) and the hangar's bounce gather (vertices): two bakes
/// that split different things the same way, and one of them would otherwise be a copy.
pub(crate) fn bake_lane_ranges(items: usize) -> Vec<(usize, usize)> {
    let per_lane = items.div_ceil(bake_lane_count(items)).max(1);
    (0..items).step_by(per_lane).map(|start| (start, (start + per_lane).min(items))).collect()
}

/// The statics bake is partitioned into an XZ grid of BUCKETS plus one backdrop bucket, so a
/// cover-phase change re-bakes one map cell instead of the whole statics mesh (urban-map
/// program PR-04: an urban core carries 110-140 boxes, and the full bake is the ~25 ms the
/// F7 worker exists to hide). The renderer re-chunks whatever it is handed at 80 m and
/// frustum-culls per chunk in every pass, so bucketing is purely a CPU-bake concern: the
/// assembled mesh is the same set of triangles the monolithic bake produced.
pub const STATICS_BUCKET_GRID: usize = 4;
/// Grid cells + the backdrop bucket (skirt + distant trees — never dirtied by gameplay).
pub const STATICS_BUCKET_COUNT: usize = STATICS_BUCKET_GRID * STATICS_BUCKET_GRID + 1;
/// The backdrop's bucket index (the last one).
pub const STATICS_BACKDROP_BUCKET: usize = STATICS_BUCKET_COUNT - 1;

/// The grid bucket owning an XZ position: positions are clamped into the map, so cover or
/// scenery standing exactly on the far border still lands in the last cell.
pub fn statics_bucket_of_position(battlefield: &BattlefieldMap, x: f32, z: f32) -> usize {
    let [extent_x, extent_z] = battlefield.heightmap.extent_m();
    let grid = STATICS_BUCKET_GRID as f32;
    let column = ((x / extent_x.max(1.0)) * grid).clamp(0.0, grid - 1.0) as usize;
    let row = ((z / extent_z.max(1.0)) * grid).clamp(0.0, grid - 1.0) as usize;
    row * STATICS_BUCKET_GRID + column
}

/// Every grid bucket a cover object's footprint touches. A phase change must re-bake the
/// bucket holding the object's geometry (its center cell) AND any cell its box overlaps:
/// scenery near a cell edge can stand inside a cover box whose center lives next door, and
/// its disappearance belongs to the scenery's own bucket.
pub fn statics_buckets_touched_by_cover(
    battlefield: &BattlefieldMap,
    cover: &StaticCoverObject,
) -> impl Iterator<Item = usize> {
    let min_bucket = statics_bucket_of_position(
        battlefield,
        cover.center[0] - cover.half_extents_m[0],
        cover.center[2] - cover.half_extents_m[2],
    );
    let max_bucket = statics_bucket_of_position(
        battlefield,
        cover.center[0] + cover.half_extents_m[0],
        cover.center[2] + cover.half_extents_m[2],
    );
    let (min_row, min_column) =
        (min_bucket / STATICS_BUCKET_GRID, min_bucket % STATICS_BUCKET_GRID);
    let (max_row, max_column) =
        (max_bucket / STATICS_BUCKET_GRID, max_bucket % STATICS_BUCKET_GRID);
    (min_row..=max_row).flat_map(move |row| {
        (min_column..=max_column).map(move |column| row * STATICS_BUCKET_GRID + column)
    })
}

/// Bake ONE statics bucket: the cover objects whose centers fall in its cell (as-authored,
/// scarred, rubble or felled by phase) plus the scenery standing in it; the backdrop bucket
/// carries the border skirt and distant trees. Deterministic per (battlefield, states, scars).
pub fn battlefield_statics_bucket_mesh(
    battlefield: &BattlefieldMap,
    cover_states: &[u8],
    cover_scars: &[terrain::CoverScar],
    bucket: usize,
) -> SceneMeshData {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    if bucket == STATICS_BACKDROP_BUCKET {
        // The world beyond the border (render-only skirt + distant trees). Gameplay never
        // dirties it, so it bakes once and survives every partial rebuild.
        let (skirt_vertices, skirt_indices) = crate::backdrop::backdrop_scene_mesh(battlefield);
        let base = vertices.len() as u32;
        vertices.extend(skirt_vertices);
        indices.extend(skirt_indices.into_iter().map(|index| index + base));
        return (vertices, indices);
    }
    for (index, cover) in battlefield.static_cover.iter().enumerate() {
        if statics_bucket_of_position(battlefield, cover.center[0], cover.center[2]) != bucket {
            continue;
        }
        match cover_states.get(index).copied().unwrap_or(0) {
            0 => {
                append_cover_box(&mut vertices, &mut indices, cover);
                // The standing tree line is PLANTED (Inny Poziom F3): real trees from the
                // map's species mix, fitted to the box, over the undergrowth mass the cover
                // box bake keeps. The box bake stays battlefield-free for its callers; the
                // planting needs the map for its mix.
                crate::tree_line::push_tree_line_trees(
                    &mut vertices,
                    &mut indices,
                    battlefield,
                    cover,
                );
                for scar in cover_scars.iter().filter(|scar| scar.cover as usize == index) {
                    append_cover_scar(&mut vertices, &mut indices, cover, scar);
                }
            }
            1 => append_rubble_mound(&mut vertices, &mut indices, cover),
            _ => {
                // Gone. A cleared TREE LINE is not a vacuum (Fizyczny Świat P11): the crowns
                // fell, but the crush leaves stumps where the trees stood and trunks lying
                // along the run. A breached STONE WALL (PR-10) leaves its toppled course —
                // knee-high, non-blocking, a door with bricks at its feet. Other kinds
                // (fences, foliage mass) clear to nothing.
                if matches!(cover.kind, StaticCoverKind::TreeLine | StaticCoverKind::TreeTrunk) {
                    // A felled hero oak leaves the same evidence a felled hedgerow does — a
                    // stump where it stood and its trunk lying beside it — sized to the box,
                    // which for a single bole is exactly one stump and one trunk.
                    append_felled_tree_line(&mut vertices, &mut indices, battlefield, cover);
                } else if cover.kind == StaticCoverKind::StoneWall {
                    append_toppled_wall(&mut vertices, &mut indices, cover);
                }
            }
        }
    }
    // Render-only dressing: trees and rocks baked into the same static upload — a dressed
    // valley costs the frame nothing (see scene::foliage). A tree standing inside a cleared
    // cover box fell with it, so it is left out of the rebuilt scene.
    let stone = crate::clutter::StoneTone::of_map(battlefield);
    for instance in &battlefield.scenery {
        if statics_bucket_of_position(battlefield, instance.position[0], instance.position[2])
            != bucket
        {
            continue;
        }
        if scenery_stands_in_cleared_cover(instance, &battlefield.static_cover, cover_states) {
            continue;
        }
        crate::clutter::push_scenery_instance(&mut vertices, &mut indices, instance, stone);
    }
    (vertices, indices)
}

/// All statics buckets, in assembly order.
///
/// Buckets are independent by construction (each bakes the cover and scenery whose centres fall
/// in its own cell), so a full bake spreads them across cores and collects them back in bucket
/// order — the assembly order the renderer's slot depends on. Measured 185 ms of a 517 ms map
/// swap before the split (release, Ostrogorsk). Partial rebuilds keep calling
/// [`battlefield_statics_bucket_mesh`] directly: one dirty bucket does not want a thread pool.
pub fn battlefield_statics_buckets(
    battlefield: &BattlefieldMap,
    cover_states: &[u8],
    cover_scars: &[terrain::CoverScar],
) -> Vec<SceneMeshData> {
    let bake =
        |bucket| battlefield_statics_bucket_mesh(battlefield, cover_states, cover_scars, bucket);
    let lanes = bake_lane_count(STATICS_BUCKET_COUNT);
    if lanes < 2 {
        return (0..STATICS_BUCKET_COUNT).map(bake).collect();
    }
    let per_lane = STATICS_BUCKET_COUNT.div_ceil(lanes);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..STATICS_BUCKET_COUNT)
            .step_by(per_lane)
            .map(|start| {
                let end = (start + per_lane).min(STATICS_BUCKET_COUNT);
                scope.spawn(move || (start..end).map(bake).collect::<Vec<_>>())
            })
            .collect();
        handles.into_iter().flat_map(|handle| handle.join().expect("statics bucket bake")).collect()
    })
}

/// Concatenate bucket fragments into the one statics buffer the renderer's slot takes. The
/// output is a pure function of the fragments, so replacing one dirty bucket and reassembling
/// equals a full fresh bake bit for bit (the partial-rebake lock below).
pub fn assemble_statics_mesh(buckets: &[SceneMeshData]) -> SceneMeshData {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for (bucket_vertices, bucket_indices) in buckets {
        let base = vertices.len() as u32;
        vertices.extend_from_slice(bucket_vertices);
        indices.extend(bucket_indices.iter().map(|index| index + base));
    }
    (vertices, indices)
}

/// The wreckage of a crushed tree line (Fizyczny Świat P11 / Świat 2.0 PR1), zero wire: a
/// stump where each of its scenery trees stood (the tree fell, its root did not), and one or
/// two trunks lying along the run, seeded from the cover id so the same hedge always falls
/// the same way. Stumps are sized to the species' butt — a mature oak leaves a ~1 m bole,
/// not a 26 cm diorama peg. All of it low, non-blocking dressing inside the old footprint.
fn append_felled_tree_line(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    battlefield: &BattlefieldMap,
    cover: &StaticCoverObject,
) {
    const BARK: [f32; 3] = [0.26, 0.20, 0.13];
    const HEARTWOOD: [f32; 3] = [0.45, 0.36, 0.24];
    let center = Vec3::from_array(cover.center);
    let half = Vec3::from_array(cover.half_extents_m);
    let ground_y = center.y - half.y;

    // Stumps: one at the foot of every tree the cleared box swallowed — the scenery it
    // hosted AND the trees it was planted with (Inny Poziom F3: the same stations the
    // standing line grew from, so the wreckage stands where the trees stood). Rocks and
    // furniture standing in the footprint are not trees and do not leave a stump.
    let hosted = battlefield.scenery.iter().filter(|instance| {
        let p = instance.position;
        (p[0] - cover.center[0]).abs() <= cover.half_extents_m[0]
            && (p[2] - cover.center[2]).abs() <= cover.half_extents_m[2]
    });
    let planted = crate::tree_line::tree_line_stations(battlefield, cover);
    for instance in hosted.chain(planted.iter().map(|station| &station.instance)) {
        let p = instance.position;
        let Some(species) = tree_species_for_scenery(instance.kind) else {
            continue;
        };
        let stump_half = stump_half_extents(species, instance.scale);
        push_surfaced_box(
            vertices,
            indices,
            Vec3::new(p[0], ground_y + stump_half.y, p[2]),
            stump_half,
            BARK,
            0.04,
        );
        // The sawn/torn top reads lighter — a thin heartwood cap.
        push_surfaced_box(
            vertices,
            indices,
            Vec3::new(p[0], ground_y + stump_half.y * 2.0 + 0.01, p[2]),
            Vec3::new(stump_half.x * 0.9, 0.015, stump_half.z * 0.9),
            HEARTWOOD,
            0.06,
        );
    }

    // Fallen trunks: one or two logs lying along the run, hashed from the cover id.
    // A single TreeTrunk bole gets one log sized to a mature oak; a hedgerow keeps the
    // along-run layout with bole-scale radius.
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
    let along_x = half.x >= half.z;
    let run = if along_x { half.x } else { half.z };
    let single_bole = cover.kind == StaticCoverKind::TreeTrunk;
    let logs = if single_bole { 1 } else { 1 + (next() * 1.99) as usize };
    for _ in 0..logs {
        let length = if single_bole {
            // A felled oak bole: most of the clear trunk, not a 6 m toothpick.
            (world_forge::tree::TreeSpecies::Oak.trunk_height() * (0.7 + next() * 0.25))
                .clamp(6.0, 12.0)
        } else {
            (run * (0.35 + next() * 0.3)).clamp(2.0, 10.0)
        };
        let radius = if single_bole {
            world_forge::tree::TreeSpecies::Oak.trunk_radius() * (0.85 + next() * 0.2)
        } else {
            0.35 + next() * 0.2
        };
        let slide = (next() - 0.5) * (run - length * 0.5).max(0.0) * 1.6;
        let drift = (next() - 0.5) * (if along_x { half.z } else { half.x }) * 0.9;
        let yaw = (next() - 0.5) * 0.5 + if along_x { 0.0 } else { std::f32::consts::FRAC_PI_2 };
        let log_center = if along_x {
            Vec3::new(center.x + slide, ground_y + radius, center.z + drift)
        } else {
            Vec3::new(center.x + drift, ground_y + radius, center.z + slide)
        };
        let start = vertices.len();
        push_oriented_box(
            vertices,
            indices,
            log_center,
            Vec3::new(length, radius, radius),
            Mat3::from_rotation_y(yaw),
            BARK,
        );
        for vertex in &mut vertices[start..] {
            vertex.gloss = 0.04;
        }
    }
}

/// Map a scenery kind to the procedural species that sizes its stump. Retired Flora* kinds
/// fall through to Oak (they are never authored; the arm keeps the match total).
fn tree_species_for_scenery(kind: terrain::SceneryKind) -> Option<world_forge::tree::TreeSpecies> {
    match kind {
        terrain::SceneryKind::Oak | terrain::SceneryKind::FloraTree => {
            Some(world_forge::tree::TreeSpecies::Oak)
        }
        terrain::SceneryKind::Poplar => Some(world_forge::tree::TreeSpecies::Poplar),
        terrain::SceneryKind::Willow => Some(world_forge::tree::TreeSpecies::Willow),
        terrain::SceneryKind::FruitTree => Some(world_forge::tree::TreeSpecies::FruitTree),
        terrain::SceneryKind::Bush | terrain::SceneryKind::FloraBush => {
            Some(world_forge::tree::TreeSpecies::Bush)
        }
        terrain::SceneryKind::Pine | terrain::SceneryKind::FloraPine => {
            Some(world_forge::tree::TreeSpecies::Pine)
        }
        terrain::SceneryKind::Rock
        | terrain::SceneryKind::Lamppost
        | terrain::SceneryKind::DebrisHeap => None,
    }
}

/// Half-extents of the stump a felled tree leaves: butt radius from the species table, knee-
/// high so it reads as a bole and never as cover. Locked by the stump-scale test below.
fn stump_half_extents(species: world_forge::tree::TreeSpecies, scale: f32) -> Vec3 {
    let scale = scale.max(0.8);
    let radius = species.trunk_radius() * scale;
    // ~0.5 m tall on a mature oak; bushes stay knee-high relative to their own butt.
    let half_height = (species.trunk_radius() * 1.0 * scale).clamp(0.25, 0.55);
    Vec3::new(radius, half_height, radius)
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

/// The masonry a wound exposes under the plaster: brick red-brown. Shared with the wound test.
pub(crate) const KINETIC_EXPOSED_TONE: [f32; 3] = [0.46, 0.31, 0.24];
pub(crate) const HE_EXPOSED_TONE: [f32; 3] = [0.42, 0.29, 0.22];
/// The recess's broken-masonry floor and lit side walls, and the rubbed plaster around it.
pub(crate) const WOUND_FLOOR_TONE: [f32; 3] = [0.19, 0.13, 0.10];
pub(crate) const WOUND_SIDE_TONE: [f32; 3] = [0.33, 0.22, 0.16];
pub(crate) const WOUND_SHED_TONE: [f32; 3] = [0.60, 0.57, 0.50];

/// Every visual stays INSIDE the collision AABB — a building may look like walls and a roof,
/// but nothing it shows can be shot through or hidden behind that the sim box does not honor
/// (locked by `cover_visuals_never_leave_the_collision_box`).
/// One shell wound on a standing cover face (protocol v32; rebuilt for Inny Poziom Z5 — the
/// owner's "co to za ślady?"). A wound is a RECESS dug into the box, never a stamp proud of it:
/// an irregular rim on the wall plane, a dark broken-masonry floor `depth` behind it, and lit
/// side walls between them whose normals lean into the cavity, so the sun shades the bite as a
/// hole and not as a black square. Around it the shed render: an irregular fan of exposed
/// brick and rubbed plaster whose rim vertices carry the wall's own tone, so the mark fades
/// into the wall instead of ending at a hard edge. Sizes come from the calibre through the
/// ledger's radius; a kinetic round is a small deep inset in a wide burst of shed plaster, a
/// high-explosive round a wide shallow bite with its spall heaped at the wall's foot.
fn append_cover_scar(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    cover: &StaticCoverObject,
    scar: &terrain::CoverScar,
) {
    let center = Vec3::from_array(cover.center);
    let half = Vec3::from_array(cover.half_extents_m);
    let (normal, u_axis, v_axis, half_u, half_v, half_n) = match scar.face {
        0 => (Vec3::X, Vec3::Z, Vec3::Y, half.z, half.y, half.x),
        1 => (-Vec3::X, Vec3::Z, Vec3::Y, half.z, half.y, half.x),
        2 => (Vec3::Z, Vec3::X, Vec3::Y, half.x, half.y, half.z),
        3 => (-Vec3::Z, Vec3::X, Vec3::Y, half.x, half.y, half.z),
        _ => (Vec3::Y, Vec3::X, Vec3::Z, half.x, half.z, half.y),
    };
    let unpack = |q: u8| q as f32 / 255.0 * 2.0 - 1.0;
    let mark = center
        + normal * half_n
        + u_axis * (unpack(scar.u_q) * half_u)
        + v_axis * (unpack(scar.v_q) * half_v);
    let mut seed = 0x9E37_79B9_u64
        ^ (u64::from(scar.cover) << 24)
        ^ (u64::from(scar.u_q) << 16)
        ^ (u64::from(scar.v_q) << 8)
        ^ u64::from(scar.face);
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 40) as f32 / ((1u64 << 24) - 1) as f32
    };
    let ledger_r = scar.radius_m();
    let kinetic = scar.kind == terrain::COVER_SCAR_KIND_KINETIC;
    // The bite the shell dug and the plaster it shed. A 100 mm AP round leaves ~6 cm of hole
    // in a burst of ~35 cm; a 100 mm HE round a ~45 cm bite (the ledger's 0.8 m is the blast's
    // reach, not the crater) with plaster gone for another third of that around it.
    let (bite_r, shed_r, depth) = if kinetic {
        (ledger_r, (ledger_r * 3.0).clamp(0.18, 0.45), (ledger_r * 1.2).clamp(0.03, 0.10))
    } else {
        let bite = (ledger_r * 0.55).clamp(0.22, 0.7);
        (bite, bite * 1.35, (bite * 0.45).clamp(0.10, 0.24))
    };
    // Never through the box: the floor stays a hand's width short of the plate behind.
    let depth = depth.min((half_n * 2.0 - 0.05).max(0.02));
    let phase_a = next() * std::f32::consts::TAU;
    let phase_b = next() * std::f32::consts::TAU;
    let rough = if kinetic { 0.18 } else { 0.32 };
    // An irregular radius: three and five lobes phased per scar, the same on every rebuild.
    let ring_radius = |base: f32, angle: f32| {
        base * (1.0
            + rough * ((angle * 3.0 + phase_a).sin() * 0.62 + (angle * 5.0 + phase_b).sin() * 0.38))
    };
    const SEGMENTS: usize = 16;
    let plate = FacePlate { center: mark, u: u_axis, v: v_axis, normal };

    // 1. The shed render: a fan from the bite's rim to the shed radius. Inner vertices are
    //    the exposed masonry (brick red-brown under plaster, grey-tan under stone), outer
    //    vertices the wall's own tone so the mark fades into the wall.
    let exposed = if kinetic { KINETIC_EXPOSED_TONE } else { HE_EXPOSED_TONE };
    let shed_tone = WOUND_SHED_TONE;
    push_face_ring(
        vertices,
        indices,
        plate,
        SEGMENTS,
        0.004,
        |angle| ring_radius(bite_r, angle) * 1.02,
        |angle| ring_radius(shed_r, angle),
        exposed,
        shed_tone,
        1.0,
    );

    // 2. The recess: the rim polygon on the wall plane, the floor polygon `depth` behind it
    //    and a little smaller, the side walls between them. Floor and sides are the broken
    //    masonry's dark core; the sides' normals lean into the cavity.
    let floor_tone = if kinetic { [0.16, 0.11, 0.09] } else { WOUND_FLOOR_TONE };
    let side_tone = if kinetic { [0.30, 0.20, 0.15] } else { WOUND_SIDE_TONE };
    let rim: Vec<Vec3> = (0..SEGMENTS)
        .map(|i| {
            let angle = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let r = ring_radius(bite_r, angle);
            mark + u_axis * (r * angle.cos()) + v_axis * (r * angle.sin())
        })
        .collect();
    let floor: Vec<Vec3> = rim.iter().map(|p| mark + (*p - mark) * 0.62 - normal * depth).collect();
    // Floor fan.
    let floor_base = vertices.len() as u32;
    let floor_center = mark - normal * depth;
    push_scene_vertex(vertices, floor_center, normal, floor_tone, 0.02, 0.0);
    for p in &floor {
        push_scene_vertex(vertices, *p, normal, floor_tone, 0.02, 0.0);
    }
    for i in 0..SEGMENTS as u32 {
        let a = floor_base + 1 + i;
        let b = floor_base + 1 + (i + 1) % SEGMENTS as u32;
        push_tri_facing(indices, vertices, floor_base, a, b, normal);
    }
    // Side walls: one quad per segment, its normal the outward-facing average of the wall's
    // normal and the direction toward the cavity's axis — a lit lip on the sunward side, a
    // shadowed one opposite.
    for i in 0..SEGMENTS {
        let j = (i + 1) % SEGMENTS;
        let (r0, r1, f0, f1) = (rim[i], rim[j], floor[i], floor[j]);
        let mid = (r0 + r1) * 0.5;
        let inward = (mark - mid).normalize_or_zero();
        let side_normal = (inward * 0.8 + normal * 0.6).normalize_or_zero();
        let base = vertices.len() as u32;
        for p in [r0, r1, f1, f0] {
            push_scene_vertex(vertices, p, side_normal, side_tone, 0.03, 0.0);
        }
        push_tri_facing(indices, vertices, base, base + 1, base + 2, side_normal);
        push_tri_facing(indices, vertices, base, base + 2, base + 3, side_normal);
    }

    // 3. Spalled masonry heaped at the wall's foot below an HE bite (wall faces only).
    if !kinetic && scar.face <= 3 {
        let ground_y = center.y - half.y;
        let foot = Vec3::new(mark.x, 0.0, mark.z) + Vec3::new(normal.x, 0.0, normal.z) * 0.35;
        let chunks = 2 + (next() * 1.99) as usize;
        for _ in 0..chunks {
            let size = bite_r * (0.22 + next() * 0.2);
            let chunk_half =
                Vec3::new(size * (0.8 + next() * 0.7), size, size * (0.8 + next() * 0.7));
            let offset = u_axis * ((next() - 0.5) * bite_r * 1.8)
                + Vec3::new(normal.x, 0.0, normal.z) * (next() * 0.4);
            let chunk_center = Vec3::new(foot.x, ground_y + chunk_half.y, foot.z) + offset;
            push_surfaced_box(vertices, indices, chunk_center, chunk_half, exposed, 0.06);
        }
    }
}

/// One vertex of a wound, in the scene's material lanes (`surface` 1 = the plaster
/// treatment's fine grain; rough matte).
fn push_scene_vertex(
    vertices: &mut Vec<SceneVertex>,
    position: Vec3,
    normal: Vec3,
    color: [f32; 3],
    gloss: f32,
    surface: f32,
) {
    vertices.push(SceneVertex {
        position: position.to_array(),
        normal: normal.to_array(),
        color,
        tint_weight: 0.0,
        gloss,
        surface,
        sway: 0.0,
        uv: [0.0, 0.0],
        bounce: [0.0; 3],
    });
}

/// A triangle wound so that it faces `normal` whatever order its corners came in.
fn push_tri_facing(
    indices: &mut Vec<u32>,
    vertices: &[SceneVertex],
    a: u32,
    b: u32,
    c: u32,
    normal: Vec3,
) {
    let p = |i: u32| Vec3::from_array(vertices[i as usize].position);
    let face = (p(b) - p(a)).cross(p(c) - p(a));
    if face.dot(normal) >= 0.0 {
        indices.extend_from_slice(&[a, b, c]);
    } else {
        indices.extend_from_slice(&[a, c, b]);
    }
}

/// An irregular flat ring on a face plate, `lift` proud of it, between an inner and an outer
/// radius function of the angle; inner vertices carry `inner_tone`, outer ones `outer_tone`.
#[allow(clippy::too_many_arguments)]
fn push_face_ring(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    plate: FacePlate,
    segments: usize,
    lift: f32,
    inner_radius: impl Fn(f32) -> f32,
    outer_radius: impl Fn(f32) -> f32,
    inner_tone: [f32; 3],
    outer_tone: [f32; 3],
    surface: f32,
) {
    let FacePlate { center, u, v, normal } = plate;
    let base = vertices.len() as u32;
    let origin = center + normal * lift;
    for i in 0..segments {
        let angle = i as f32 / segments as f32 * std::f32::consts::TAU;
        let dir = u * angle.cos() + v * angle.sin();
        push_scene_vertex(
            vertices,
            origin + dir * inner_radius(angle),
            normal,
            inner_tone,
            0.03,
            surface,
        );
        push_scene_vertex(
            vertices,
            origin + dir * outer_radius(angle),
            normal,
            outer_tone,
            0.03,
            surface,
        );
    }
    for i in 0..segments as u32 {
        let j = (i + 1) % segments as u32;
        let (i0, o0, i1, o1) = (base + i * 2, base + i * 2 + 1, base + j * 2, base + j * 2 + 1);
        push_tri_facing(indices, vertices, i0, o0, o1, normal);
        push_tri_facing(indices, vertices, i0, o1, i1, normal);
    }
}

/// A wooden farm fence inside its (thin, waist-high) collision box: posts every couple of
/// metres along the long axis and two rails between them — matter the eye reads as exactly
/// what a hull can crush and a shell can sweep away (Fizyczny Świat P10).
fn append_wooden_fence(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: Vec3,
    half: Vec3,
) {
    const WOOD: [f32; 3] = [0.30, 0.24, 0.17];
    const POST_SPACING_M: f32 = 1.9;
    let along_x = half.x >= half.z;
    let run = if along_x { half.x } else { half.z };
    let axis = if along_x { Vec3::X } else { Vec3::Z };
    let ground_y = center.y - half.y;
    // Posts stay INSIDE the collision box (the honesty lock: what blocks the shell blocks
    // the eye) — the end posts pull in by their own half-thickness.
    let post_run = run - 0.06;
    let posts = ((post_run * 2.0 / POST_SPACING_M).ceil() as usize).max(2) + 1;
    for index in 0..posts {
        let t = -post_run + (index as f32 / (posts - 1) as f32) * post_run * 2.0;
        let post_center = center + axis * t;
        push_surfaced_box(
            vertices,
            indices,
            Vec3::new(post_center.x, ground_y + half.y, post_center.z),
            Vec3::new(0.06, half.y, 0.06),
            WOOD,
            0.04,
        );
    }
    for rail_frac in [0.55_f32, 0.95] {
        let rail_y = ground_y + half.y * 2.0 * rail_frac - 0.04;
        let rail_half =
            if along_x { Vec3::new(half.x, 0.045, 0.03) } else { Vec3::new(0.03, 0.045, half.z) };
        push_surfaced_box(
            vertices,
            indices,
            Vec3::new(center.x, rail_y, center.z),
            rail_half,
            WOOD,
            0.04,
        );
    }
}

/// A wound's frame on a cover face: its center and the face's in-plane axes plus normal.
#[derive(Clone, Copy)]
struct FacePlate {
    center: Vec3,
    u: Vec3,
    v: Vec3,
    normal: Vec3,
}

/// One single-sided quad flush on a cover face: half-size `half_m` along both in-plane axes.
fn append_cover_box(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    cover: &StaticCoverObject,
) {
    let center = Vec3::from_array(cover.center);
    let half = Vec3::from_array(cover.half_extents_m);
    match cover.kind {
        StaticCoverKind::FarmBuilding => append_building(vertices, indices, cover, center, half),
        StaticCoverKind::RailCover => append_rail_cover(vertices, indices, center, half),
        StaticCoverKind::TreeLine => append_tree_line(vertices, indices, cover),
        // A hero oak's bole draws NOTHING here: the instanced tree mesh already stands in this
        // box, to the metre. Baking a solid as well would put a second trunk inside the first —
        // the one kind whose "the box IS the visual footprint" promise is kept by the dressing
        // rather than by a box of its own.
        StaticCoverKind::TreeTrunk => {}
        StaticCoverKind::Wreck => {
            // Burnt steel: the glossiest thing on the field short of water.
            push_surfaced_box(vertices, indices, center, half, [0.25, 0.20, 0.17], 0.30);
        }
        StaticCoverKind::WoodenFence => append_wooden_fence(vertices, indices, center, half),
        // PROVISIONAL look (urban-map PR-06): the city building rides the same forged
        // building bake (its tall box derives Townhouse until the Tenement style lands,
        // wave U). Semantics are final; only the dressing is interim.
        StaticCoverKind::CityBuilding => append_building(vertices, indices, cover, center, half),
        StaticCoverKind::StoneWall => append_stone_wall(vertices, indices, center, half),
        StaticCoverKind::Crag => append_crag(vertices, indices, cover, center, half),
        StaticCoverKind::StoneTower => append_stone_tower(vertices, indices, center, half),
    }
}

/// A Svan-style stone watchtower (teren W3b, the Orliny col landmark): a granite plinth, a
/// battered limewashed shaft rising in weathered courses, slit loopholes, a machicolated
/// crown re-widening over a shadowed throat, corner piers and a stepped slate cap. Every
/// block STRICTLY inside the collision box — the pale shaft against the granite pass is the
/// landmark read, and what stops the shell is exactly what the eye sees standing there.
fn append_stone_tower(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: Vec3,
    half: Vec3,
) {
    const LIME: ([f32; 3], f32) = ([0.56, 0.54, 0.49], 0.06);
    const LIME_SHADE: ([f32; 3], f32) = ([0.50, 0.48, 0.43], 0.06);
    const STONE: ([f32; 3], f32) = ([0.38, 0.37, 0.34], 0.10);
    const SLATE: ([f32; 3], f32) = ([0.24, 0.25, 0.27], 0.14);
    const VOID: ([f32; 3], f32) = ([0.06, 0.06, 0.07], 0.02);
    let ground_y = center.y - half.y;
    let height = half.y * 2.0;
    let foot = half.x.min(half.z);

    // The exposed granite plinth seating the full footprint.
    let plinth_h = (height * 0.06).clamp(0.4, 1.0);
    push_surfaced_box(
        vertices,
        indices,
        Vec3::new(center.x, ground_y + plinth_h * 0.5, center.z),
        Vec3::new(half.x * 0.99, plinth_h * 0.5, half.z * 0.99),
        STONE.0,
        STONE.1,
    );

    // The battered shaft: four courses tapering toward the crown, alternating limewash
    // weathering so the shaft reads as coursework, not one extrusion.
    let crown_h = (height * 0.11).clamp(1.2, 2.0);
    let throat_h = 0.3;
    let cap_h = (height * 0.06).clamp(0.6, 1.1);
    let shaft_h = height - plinth_h - throat_h - crown_h - cap_h;
    let courses = 4;
    let course_h = shaft_h / courses as f32;
    let mut course_insets = [0.0f32; 4];
    for (course, inset) in course_insets.iter_mut().enumerate() {
        let t = course as f32 / (courses - 1) as f32;
        *inset = 0.96 - t * 0.14; // 0.96 → 0.82 of the box half: the batter.
    }
    for (course, inset) in course_insets.iter().enumerate() {
        let base = ground_y + plinth_h + course as f32 * course_h;
        let tone = if course % 2 == 0 { LIME } else { LIME_SHADE };
        push_surfaced_box(
            vertices,
            indices,
            Vec3::new(center.x, base + course_h * 0.5, center.z),
            Vec3::new(half.x * inset, course_h * 0.5, half.z * inset),
            tone.0,
            tone.1,
        );
    }

    // Slit loopholes on the two upper courses: thin dark plates just proud of their
    // course's face (and still well inside the box), one per face per level.
    for level in [2usize, 3] {
        let inset = course_insets[level];
        let slit_y = ground_y + plinth_h + (level as f32 + 0.55) * course_h;
        let slit_half_h = (course_h * 0.16).clamp(0.3, 0.5);
        for (dx, dz, hx, hz) in [
            (half.x * inset + 0.03, 0.0, 0.03, foot * 0.05),
            (-half.x * inset - 0.03, 0.0, 0.03, foot * 0.05),
            (0.0, half.z * inset + 0.03, foot * 0.05, 0.03),
            (0.0, -half.z * inset - 0.03, foot * 0.05, 0.03),
        ] {
            push_surfaced_box(
                vertices,
                indices,
                Vec3::new(center.x + dx, slit_y, center.z + dz),
                Vec3::new(hx, slit_half_h, hz),
                VOID.0,
                VOID.1,
            );
        }
    }

    // The shadowed throat under the crown's overhang, then the crown re-widening past the
    // shaft — the machicolation silhouette that says "watchtower" at a kilometre.
    let throat_y = ground_y + plinth_h + shaft_h;
    push_surfaced_box(
        vertices,
        indices,
        Vec3::new(center.x, throat_y + throat_h * 0.5, center.z),
        Vec3::new(half.x * 0.76, throat_h * 0.5, half.z * 0.76),
        VOID.0,
        VOID.1,
    );
    let crown_y = throat_y + throat_h;
    push_surfaced_box(
        vertices,
        indices,
        Vec3::new(center.x, crown_y + crown_h * 0.5, center.z),
        Vec3::new(half.x * 0.94, crown_h * 0.5, half.z * 0.94),
        STONE.0,
        STONE.1,
    );
    // Machicolation mouths: dark tabs under the crown lip, three per face.
    for face in 0..4 {
        for mouth in 0..3 {
            let along = (mouth as f32 - 1.0) * foot * 0.5;
            let (dx, dz, hx, hz) = match face {
                0 => (half.x * 0.94 + 0.02, along, 0.02, foot * 0.09),
                1 => (-half.x * 0.94 - 0.02, along, 0.02, foot * 0.09),
                2 => (along, half.z * 0.94 + 0.02, foot * 0.09, 0.02),
                _ => (along, -half.z * 0.94 - 0.02, foot * 0.09, 0.02),
            };
            push_surfaced_box(
                vertices,
                indices,
                Vec3::new(center.x + dx, crown_y + crown_h * 0.22, center.z + dz),
                Vec3::new(hx, crown_h * 0.16, hz),
                VOID.0,
                VOID.1,
            );
        }
    }
    // Corner piers on the crown top carrying the cap.
    let pier_h = cap_h * 0.45;
    for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        push_surfaced_box(
            vertices,
            indices,
            Vec3::new(
                center.x + sx * half.x * 0.72,
                crown_y + crown_h + pier_h * 0.5,
                center.z + sz * half.z * 0.72,
            ),
            Vec3::new(half.x * 0.16, pier_h * 0.5, half.z * 0.16),
            STONE.0,
            STONE.1,
        );
    }
    // The stepped slate cap: two shrinking slabs reading as the low pyramid roof.
    let cap_y = crown_y + crown_h + pier_h;
    let slab_h = (cap_h - pier_h) * 0.5;
    for (step, spread) in [(0usize, 0.88f32), (1, 0.5)] {
        push_surfaced_box(
            vertices,
            indices,
            Vec3::new(center.x, cap_y + (step as f32 + 0.5) * slab_h, center.z),
            Vec3::new(half.x * spread, slab_h * 0.5, half.z * spread),
            SLATE.0,
            SLATE.1,
        );
    }
}

/// A granite crag (teren W3b): jointed blocks, not a boulder — a scree plinth carrying a
/// stagger of joint-split columns, every block STRICTLY inside the collision box (what
/// stops the shell is what the eye sees; nothing pokes past the footprint). Deterministic
/// per crag: block heights and offsets grow from the cover's own position, so mirrored
/// twins wear mirrored stone and a recompile never reshuffles a face.
fn append_crag(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    cover: &StaticCoverObject,
    center: Vec3,
    half: Vec3,
) {
    const GRANITE: [f32; 3] = [0.34, 0.33, 0.31];
    const GRANITE_LIT: [f32; 3] = [0.39, 0.38, 0.35];
    const GRANITE_SHADE: [f32; 3] = [0.29, 0.29, 0.28];
    const GLOSS: f32 = 0.12;
    let ground_y = center.y - half.y;
    let unit = |salt: u64| terrain::position_unit(cover.center[0], cover.center[2], salt);

    // The scree plinth: a broad low slab seating the whole footprint.
    let plinth_h = half.y * (0.28 + unit(0xC401) * 0.10);
    push_surfaced_box(
        vertices,
        indices,
        Vec3::new(center.x, ground_y + plinth_h, center.z),
        Vec3::new(half.x * 0.96, plinth_h, half.z * 0.93),
        GRANITE_SHADE,
        GLOSS,
    );

    // Joint-split columns: staggered along the crag's long axis, tallest near the middle,
    // each clamped inside the box in every axis.
    let along_x = half.x >= half.z;
    let columns = 4;
    for column in 0..columns {
        let t = (column as f32 + 0.5) / columns as f32;
        let salt = 0xC410 + column as u64;
        let peak = 1.0 - (t - 0.5).abs() * 0.9;
        let col_h = half.y * (0.55 + peak * 0.45 - unit(salt) * 0.12).clamp(0.4, 1.0);
        let (long_half, thin_half) = if along_x { (half.x, half.z) } else { (half.z, half.x) };
        let col_long = long_half * (0.16 + unit(salt + 8) * 0.10);
        let col_thin = thin_half * (0.62 + unit(salt + 16) * 0.25);
        let long_center = (t * 2.0 - 1.0) * (long_half - col_long) * 0.92;
        let thin_center = (unit(salt + 24) - 0.5) * (thin_half - col_thin) * 1.2;
        let (dx, dz, hx, hz) = if along_x {
            (long_center, thin_center, col_long, col_thin)
        } else {
            (thin_center, long_center, col_thin, col_long)
        };
        let tone = match column % 3 {
            0 => GRANITE,
            1 => GRANITE_LIT,
            _ => GRANITE_SHADE,
        };
        push_surfaced_box(
            vertices,
            indices,
            Vec3::new(center.x + dx, ground_y + col_h, center.z + dz),
            Vec3::new(hx, col_h, hz),
            tone,
            GLOSS,
        );
    }
}

/// The tree line as a szpaler (Świat 2.0 PR 5) — not a slab. A low undergrowth mass carries
/// the run's foot, then two STAGGERED rows of boles rise to the crown line; the crowns stay
/// the instanced oaks of the scenery in-fill. Every element lives INSIDE the collision box,
/// which now towers to the trees' own height (15–25 m): what blocks the shell is what the
/// eye sees standing there, and there is no dark slab left wearing the treeline's name.
fn append_tree_line(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    cover: &StaticCoverObject,
) {
    // The hedge body is all this bake draws (Inny Poziom F3); the trees are PLANTED over it by
    // `tree_line::push_tree_line_trees` from the map's species mix, each with a crown hull.
    // The two rows of stick boles under a run of crown-coloured boxes that stood here read as
    // what they were, while the map's own species stood fully grown twenty metres away.
    //
    // The body is the dense shrub layer every hedgerow and windbreak has under its trees —
    // a mass to `TREE_LINE_BODY_HEIGHT` of the wall (6.2–8.8 m on the shipped boxes: over
    // the tallest hull in the fleet, so a tank behind the wall is hidden by the wall the sim
    // hides it with), inset from the box faces so the silhouette stays leafy, not slab-sided.
    // The crown hulls take over from `tree_line::HULL_BOTTOM` up, so the wall is opaque from
    // the ground to the crowns' tips.
    const UNDERGROWTH: [f32; 3] = [0.10, 0.17, 0.09];
    let center = Vec3::from_array(cover.center);
    let half = Vec3::from_array(cover.half_extents_m);
    let ground_y = center.y - half.y;
    let along_x = half.x >= half.z;
    let run = if along_x { half.x } else { half.z };
    let thin = if along_x { half.z } else { half.x };
    let box_top = half.y * 2.0;
    let body_h = (box_top * TREE_LINE_BODY_HEIGHT).clamp(2.6, 9.0);
    let (ux, uz) = if along_x { (run - 0.15, thin - 0.22) } else { (thin - 0.22, run - 0.15) };
    push_surfaced_box(
        vertices,
        indices,
        Vec3::new(center.x, ground_y + body_h * 0.5, center.z),
        Vec3::new(ux, body_h * 0.5, uz),
        UNDERGROWTH,
        0.04,
    );
}

/// The hedge body reaches this fraction of the box height (clamped to 2.6–9 m): above the
/// crown hulls' bottom (`tree_line::HULL_BOTTOM`, 0.30), so body and hulls overlap; 0.35 left a sight line through where the hulls still taper.
pub(crate) const TREE_LINE_BODY_HEIGHT: f32 = 0.40;

/// RailCover 2.0 (Świat 2.0 PR 7): not a bare slab. Elongated boxes become a revetment —
/// battered stone face, earth fill behind, coping course, buttresses on the longer runs.
/// Square tall boxes (the Ostrogorsk elevator silos) become a coursed masonry tower with a
/// cap. Every element lives INSIDE the collision AABB; the kind stays one wire identity.
fn append_rail_cover(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: Vec3,
    half: Vec3,
) {
    let along_x = half.x >= half.z;
    let (run_half, thick_half) = if along_x { (half.x, half.z) } else { (half.z, half.x) };
    // A tower reads square in plan and taller than it is wide — the silo case. Everything
    // else is a run: parapet, berm, sangar, retaining wall, log cover.
    let is_tower = thick_half >= run_half * 0.85 && half.y >= run_half * 1.4;
    if is_tower {
        append_rail_tower(vertices, indices, center, half);
    } else {
        append_rail_revetment(vertices, indices, center, half, along_x, run_half, thick_half);
    }
}

fn append_rail_revetment(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: Vec3,
    half: Vec3,
    along_x: bool,
    run_half: f32,
    thick_half: f32,
) {
    const FILL: ([f32; 3], f32) = ([0.34, 0.30, 0.24], 0.05);
    const FACE: ([f32; 3], f32) = ([0.42, 0.40, 0.36], 0.14);
    const COPING: ([f32; 3], f32) = ([0.50, 0.48, 0.44], 0.18);
    let ground_y = center.y - half.y;
    let coping_half_y = (half.y * 0.14).clamp(0.05, 0.16);
    let body_half_y = (half.y - coping_half_y).max(0.08);

    // Earth/ballast fill: most of the thickness, recessed so the dressed face reads proud.
    let fill_thick = (thick_half * 0.72).max(thick_half - 0.35);
    let fill = if along_x {
        Vec3::new(run_half - 0.05, body_half_y, fill_thick)
    } else {
        Vec3::new(fill_thick, body_half_y, run_half - 0.05)
    };
    push_surfaced_box(
        vertices,
        indices,
        Vec3::new(center.x, ground_y + body_half_y, center.z),
        fill,
        FILL.0,
        FILL.1,
    );

    // Dressed stone face: a thin leaf along the long sides (both faces of a berm), inset
    // from the run ends so corners don't overhang the box.
    let face_thick = (thick_half - fill_thick).clamp(0.08, 0.35);
    let face_inset = (run_half * 0.02).clamp(0.05, 0.25);
    let face_run = (run_half - face_inset).max(0.2);
    let face_y = ground_y + body_half_y * 0.92;
    let face_half_y = body_half_y * 0.92;
    for side in [-1.0_f32, 1.0] {
        let across = side * (thick_half - face_thick);
        let (pos, he) = if along_x {
            (
                Vec3::new(center.x, face_y, center.z + across),
                Vec3::new(face_run, face_half_y, face_thick),
            )
        } else {
            (
                Vec3::new(center.x + across, face_y, center.z),
                Vec3::new(face_thick, face_half_y, face_run),
            )
        };
        push_surfaced_box(vertices, indices, pos, he, FACE.0, FACE.1);
    }

    // Coping: full thickness, the lighter crown that sheds rain along the whole run.
    let coping = if along_x {
        Vec3::new(run_half, coping_half_y, thick_half)
    } else {
        Vec3::new(thick_half, coping_half_y, run_half)
    };
    push_surfaced_box(
        vertices,
        indices,
        Vec3::new(center.x, center.y + half.y - coping_half_y, center.z),
        coping,
        COPING.0,
        COPING.1,
    );

    // Buttresses on longer, taller runs — stone piers keyed into the face, never past the box.
    if run_half >= 4.0 && half.y >= 0.8 {
        let pier_count = ((run_half * 2.0 / 4.0).round() as u32).clamp(2, 6);
        for pier in 0..pier_count {
            let t = if pier_count == 1 {
                0.0
            } else {
                (pier as f32 / (pier_count - 1) as f32) * 2.0 - 1.0
            };
            let along = t * (run_half - 0.45);
            let pier_half_y = body_half_y * 0.95;
            let pier_run = 0.28_f32.min(run_half * 0.12);
            let pier_thick = (thick_half * 0.95).min(thick_half);
            let (pos, he) = if along_x {
                (
                    Vec3::new(center.x + along, ground_y + pier_half_y, center.z),
                    Vec3::new(pier_run, pier_half_y, pier_thick),
                )
            } else {
                (
                    Vec3::new(center.x, ground_y + pier_half_y, center.z + along),
                    Vec3::new(pier_thick, pier_half_y, pier_run),
                )
            };
            push_surfaced_box(vertices, indices, pos, he, FACE.0, FACE.1);
        }
    }
}

fn append_rail_tower(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: Vec3,
    half: Vec3,
) {
    const COURSE: ([f32; 3], f32) = ([0.40, 0.38, 0.34], 0.14);
    const BAND: ([f32; 3], f32) = ([0.48, 0.46, 0.42], 0.16);
    const CAP: ([f32; 3], f32) = ([0.52, 0.50, 0.46], 0.18);
    let ground_y = center.y - half.y;
    let plan = half.x.min(half.z);
    let courses = 4_u32;
    let course_h = (half.y * 2.0 - 0.35) / courses as f32;
    for course in 0..courses {
        let inset = 0.04 * course as f32;
        let he_xz = (plan - inset).max(plan * 0.82);
        let y0 = ground_y + course as f32 * course_h;
        let tone = if course % 2 == 0 { COURSE } else { BAND };
        push_surfaced_box(
            vertices,
            indices,
            Vec3::new(center.x, y0 + course_h * 0.5, center.z),
            Vec3::new(he_xz, course_h * 0.5, he_xz),
            tone.0,
            tone.1,
        );
    }
    let cap_h = 0.18_f32.min(half.y * 0.12);
    push_surfaced_box(
        vertices,
        indices,
        Vec3::new(center.x, center.y + half.y - cap_h, center.z),
        Vec3::new(plan, cap_h, plan),
        CAP.0,
        CAP.1,
    );
}

/// The coursed masonry wall (urban-map PR-10): the brick body, a lighter COPING course
/// crowning the run, and piers every few metres standing slightly proud — the mechanical
/// story of a real compound wall (piers carry it, the coping sheds rain). Every box stays
/// inside the collision footprint: proud means toward the wall's own faces, never past them.
fn append_stone_wall(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: Vec3,
    half: Vec3,
) {
    const BRICK: ([f32; 3], f32) = ([0.42, 0.36, 0.30], 0.08);
    const COPING: ([f32; 3], f32) = ([0.52, 0.50, 0.46], 0.14);
    let along_x = half.x >= half.z;
    let (run_half, thick_half) = if along_x { (half.x, half.z) } else { (half.z, half.x) };
    let coping_half_y = (half.y * 0.12).clamp(0.04, 0.12);
    // The body: the wall run, slightly recessed in thickness so the piers read proud.
    let body_thick = (thick_half - 0.05).max(thick_half * 0.7);
    let body_half_y = half.y - coping_half_y;
    let body = if along_x {
        Vec3::new(run_half, body_half_y, body_thick)
    } else {
        Vec3::new(body_thick, body_half_y, run_half)
    };
    push_surfaced_box(
        vertices,
        indices,
        Vec3::new(center.x, center.y - coping_half_y, center.z),
        body,
        BRICK.0,
        BRICK.1,
    );
    // The coping: full thickness, the lighter stone cap along the whole run.
    let coping = if along_x {
        Vec3::new(run_half, coping_half_y, thick_half)
    } else {
        Vec3::new(thick_half, coping_half_y, run_half)
    };
    push_surfaced_box(
        vertices,
        indices,
        Vec3::new(center.x, center.y + half.y - coping_half_y, center.z),
        coping,
        COPING.0,
        COPING.1,
    );
    // Piers every ~3.2 m, full thickness and a touch of extra presence under the coping.
    let pier_count = ((run_half * 2.0 / 3.2).round() as u32).clamp(2, 8);
    for pier in 0..pier_count {
        let t =
            if pier_count == 1 { 0.0 } else { (pier as f32 / (pier_count - 1) as f32) * 2.0 - 1.0 };
        let along = t * (run_half - 0.35);
        let pier_half = if along_x {
            Vec3::new(0.22, body_half_y, thick_half)
        } else {
            Vec3::new(thick_half, body_half_y, 0.22)
        };
        let position = if along_x {
            Vec3::new(center.x + along, center.y - coping_half_y, center.z)
        } else {
            Vec3::new(center.x, center.y - coping_half_y, center.z + along)
        };
        push_surfaced_box(vertices, indices, position, pier_half, COPING.0, BRICK.1);
    }
}

/// The breach (urban-map PR-10), zero wire: a destroyed or crushed wall goes GONE for the
/// sim — a clear door — and the eye gets its toppled course: a knee-high run of tumbled
/// brick slabs seeded from the cover id, all inside the old footprint and far below any
/// height that could read as cover. The felled-tree-line pattern, in masonry.
fn append_toppled_wall(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    cover: &StaticCoverObject,
) {
    const TUMBLE: ([f32; 3], f32) = ([0.45, 0.40, 0.34], 0.07);
    let mut seed = 0xcbf2_9ce4_8422_2325_u64;
    for byte in cover.id.bytes() {
        seed ^= u64::from(byte);
        seed = seed.wrapping_mul(0x0100_0000_01b3);
    }
    let center = Vec3::from_array(cover.center);
    let half = Vec3::from_array(cover.half_extents_m);
    let ground_y = center.y - half.y;
    let along_x = half.x >= half.z;
    let run_half = if along_x { half.x } else { half.z };
    let thick_half = if along_x { half.z } else { half.x };
    let slabs = ((run_half * 2.0 / 1.1).round() as u32).clamp(3, 14);
    for slab in 0..slabs {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let unit = |shift: u32| ((seed >> shift) & 0xFFFF) as f32 / 65535.0;
        let t = (slab as f32 + 0.5) / slabs as f32 * 2.0 - 1.0;
        let along = t * (run_half - 0.6).max(0.0);
        let slab_height = 0.10 + unit(0) * 0.14;
        let slab_run = 0.32 + unit(16) * 0.22;
        let sideways = (unit(32) - 0.5) * thick_half.max(0.2);
        let slab_half = if along_x {
            Vec3::new(slab_run, slab_height, (thick_half * 0.8).clamp(0.1, 0.5))
        } else {
            Vec3::new((thick_half * 0.8).clamp(0.1, 0.5), slab_height, slab_run)
        };
        let position = if along_x {
            Vec3::new(center.x + along, ground_y + slab_height, center.z + sideways * 0.4)
        } else {
            Vec3::new(center.x + sideways * 0.4, ground_y + slab_height, center.z + along)
        };
        push_surfaced_box(vertices, indices, position, slab_half, TUMBLE.0, TUMBLE.1);
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

/// A building inside its box — FORGED (B4), baked AT SIZE (Immersja A1.2): the world_forge
/// generator picks a style from the box's proportions (a long low box is a barn, a tall one
/// a townhouse, the rest cottages), seeds the joinery from the building id, and the bake is
/// computed IN the collision AABB's own meters — openings keep their real-world absolute
/// sizes and a longer wall earns MORE windows, never wider ones (the per-axis stretch that
/// scaled a window with its box is retired). The generator's honesty lock still guarantees
/// every vertex stays inside, so the rule "what blocks the shell blocks the eye" is
/// unchanged. Palette stays per id: a town is a town, not a barracks.
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
    let style = derived_building_style(&cover.id, half);
    // The generator's ridge runs +Z; run it down the box's LONG axis like the roofs always
    // did — the bake receives the box PRE-SWAPPED so its layout math works in the meters the
    // rotated mesh will actually occupy.
    let rotate = half.x > half.z;
    let target = if rotate { Vec3::new(half.z, half.y, half.x) } else { half };
    let baked = world_forge::building::bake_building_sized(
        style,
        seed,
        world_forge::building::StructureForm::Intact,
        target,
    );
    let ground = Vec3::new(center.x, center.y - half.y, center.z);
    let stone = stone_palette(&cover.id);
    for mesh in [&baked.walls, &baked.roof] {
        let base = vertices.len() as u32;
        for vertex in mesh.vertices() {
            // A true 90-degree rotation (det +1), never an axis swap: a swap is a REFLECTION
            // and would wind every triangle inside-out.
            let p = vertex.position;
            let local = if rotate { Vec3::new(p.z, p.y, -p.x) } else { p };
            let n = vertex.normal;
            let n = if rotate { Vec3::new(n.z, n.y, -n.x) } else { n };
            // Colour names the palette; the surface-role lane names the MATERIAL the scene
            // shader dresses it in (Materia Świata 3): rendered walls, coursed roofs, plank
            // doors, ashlar stone. Glass alone keeps the untreated look. The vertex tag
            // decodes to the world's OWN vocabulary (M2b) — no vehicle roles, no style
            // heuristics; walls and roofs keep the per-building palette, the joinery and
            // the dressed-stone trim (plinth, sills, lintel bands, lesenes) wear per-id
            // tones (Fasada 2.0 — the hard black plinth of D19 is retired).
            let (color, gloss, role) = crate::world_material::to_scene(
                world_forge::WorldMaterial::from_carrier(vertex.material),
                crate::world_material::Palette { wall, roof, roof_gloss, stone },
            );
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

/// The ONE style-derivation table (B4 + urban-map PR-08). Landmarks and the urban block
/// stand by NAME — explicit id substrings are the primary mechanism (`church`, `windmill`,
/// `tenement`); the proportion heuristic remains the fallback: a box too tall for a
/// townhouse (half-height >= 5 m) IS three storeys of masonry, elongated-and-low reads barn,
/// tall reads townhouse, the rest cottages.
pub(crate) fn derived_building_style(id: &str, half: Vec3) -> world_forge::building::BuildingStyle {
    use world_forge::building::BuildingStyle;
    let elongation = half.x.max(half.z) / half.x.min(half.z).max(0.1);
    if id.contains("church") {
        BuildingStyle::Church
    } else if id.contains("windmill") {
        BuildingStyle::Windmill
    } else if id.contains("factory") {
        // Halls stand by NAME only (PR-09): no box proportion ever invents an industrial
        // span — a map says "factory" or it gets civic masonry.
        BuildingStyle::FactoryHall
    } else if id.contains("tenement") || half.y >= 5.0 {
        BuildingStyle::Tenement
    } else if elongation > 1.45 && half.y < 2.9 {
        BuildingStyle::Barn
    } else if half.y >= 2.9 {
        BuildingStyle::Townhouse
    } else {
        BuildingStyle::Cottage
    }
}

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

/// The dressed-stone palette (Fasada 2.0): plinth, sills, lintel bands, lesenes and portals
/// share ONE stone tone per building — a wall and its trim are one masonry story. Seeded
/// apart from the wall/roof palette so the trim never tracks the plaster; every tone sits
/// far above the old hard-black plinth (D19).
fn stone_palette(id: &str) -> ([f32; 3], f32) {
    let mut hash = 0x9e37_79b9_7f4a_7c15_u64;
    for byte in id.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    const STONES: [([f32; 3], f32); 4] = [
        ([0.48, 0.44, 0.37], 0.18), // warm limestone
        ([0.42, 0.39, 0.34], 0.16), // cool sandstone
        ([0.37, 0.35, 0.32], 0.14), // grey granite
        ([0.52, 0.47, 0.38], 0.20), // pale ashlar
    ];
    STONES[(hash % 4) as usize]
}

/// Build a lit triangle mesh for the whole heightmap, colored by height and slope so
/// the terrain reads clearly: grass in the lowlands, rock on the heights and steeps.
pub fn terrain_scene_mesh(heightmap: &HeightMap) -> (Vec<SceneVertex>, Vec<u32>) {
    terrain_scene_mesh_with_water(heightmap, WaterView::DRY)
}

/// Like [`terrain_scene_mesh`], with submerged ground tinted river-blue by depth — the
/// stopgap water read until the real animated surface pass lands. Gameplay water (drag,
/// drowning, splashes) is already live; the tint keeps the danger visible in the meantime.
pub fn terrain_scene_mesh_with_water<'a>(
    heightmap: &HeightMap,
    water: impl Into<WaterView<'a>>,
) -> (Vec<SceneVertex>, Vec<u32>) {
    terrain_scene_mesh_full(heightmap, water.into(), &[], None)
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
    water: WaterView<'_>,
    roads: &[Road],
    beyond: Option<&dyn Fn(f32, f32) -> f32>,
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
        {
            let depth = water.depth_at(y, wx, wz);
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
            uv: [0.0, 0.0],
            bounce: [0.0; 3],
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
            // The diagonal is the SAME one the sim samples across
            // (`terrain::cell_splits_on_main_diagonal`): one rule, one surface — and,
            // unlike the old fixed anti-diagonal, symmetric under a fair map's mirror.
            let h00 = heightmap.sample_at_index(x, z);
            let h10 = heightmap.sample_at_index(x + 1, z);
            let h01 = heightmap.sample_at_index(x, z + 1);
            let h11 = heightmap.sample_at_index(x + 1, z + 1);
            if terrain::cell_splits_on_main_diagonal(h00, h10, h01, h11) {
                indices.extend_from_slice(&[i, down, down + 1, i, down + 1, right]);
            } else {
                indices.extend_from_slice(&[i, down, right, right, down, down + 1]);
            }
        }
    }
    for &(x, z) in &cut {
        append_crater_cell(&mut vertices, &mut indices, heightmap, x, z, &make_vertex);
    }
    if let Some(beyond) = beyond {
        append_border_apron(&mut vertices, &mut indices, heightmap, beyond, &make_vertex);
    }
    (vertices, indices)
}

/// Border apron: the seam ring runs fine enough to stand next to, the far ring coarsens
/// toward the haze. See [`append_border_apron`].
const APRON_NEAR_OUT_M: f32 = 240.0;
const APRON_NEAR_CELL_M: f32 = 12.0;
pub(crate) const APRON_FAR_OUT_M: f32 = 1500.0;
const APRON_FAR_CELL_M: f32 = 48.0;
/// Seam overlap depth: a finer surface's territory is entered a little below it, so any
/// T-junction between the grids shows ground behind it, never a slit of sky (the backdrop
/// skirt's proven trick).
const APRON_TUCK_M: f32 = 0.4;

/// The ground pipeline continued beyond the red line — two rings of quads around the
/// playfield, heights from the map's analytic continuation, coloured through the same
/// `make_vertex` as the playfield (water depth tint and all). The splat/macro samplers clamp
/// at the border so the last meadow's material carries outward, while the shader's
/// procedural work — field quilt, detail grain, micro octave — is world-space and simply
/// keeps going. The result: the world past the border is more of the same land melting into
/// the aerial haze, not a different game. Render-only; the red line keeps physics inside.
fn append_border_apron(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    heightmap: &HeightMap,
    beyond: &dyn Fn(f32, f32) -> f32,
    make_vertex: &impl Fn(f32, f32, f32, Vec3) -> SceneVertex,
) {
    let [extent_x, extent_z] = heightmap.extent_m();
    // (outer reach, cell size, hole reach): each pass emits the frame between its own outer
    // square and a hole one cell INSIDE the finer surface it must overlap-under. The near
    // pass tucks under the playfield; the far pass tucks under the near ring.
    let passes = [
        (APRON_NEAR_OUT_M, APRON_NEAR_CELL_M, 0.0),
        (APRON_FAR_OUT_M, APRON_FAR_CELL_M, APRON_NEAR_OUT_M),
    ];
    for (out_m, cell_m, hole_out_m) in passes {
        let inside_frame = |x: f32, z: f32, reach: f32| -> bool {
            x > -reach && x < extent_x + reach && z > -reach && z < extent_z + reach
        };
        let n = (((extent_x.max(extent_z) + 2.0 * out_m) / cell_m).ceil() as usize) + 1;
        let position_of = |i: usize| -> f32 { -out_m + i as f32 * cell_m };
        let mut vertex_index = vec![u32::MAX; n * n];
        let corner = |vertices: &mut Vec<SceneVertex>,
                      vertex_index: &mut Vec<u32>,
                      ix: usize,
                      iz: usize|
         -> u32 {
            let slot = iz * n + ix;
            if vertex_index[slot] == u32::MAX {
                let (x, z) = (position_of(ix), position_of(iz));
                let mut y = beyond(x, z);
                if inside_frame(x, z, hole_out_m) {
                    // The tuck seats under the LIVE surface where one exists: a shell
                    // crater digs the playfield up to 1.2 m below the analytic
                    // continuation, and an apron tucked 0.4 m under the UN-cratered
                    // height floated as a plate inside the bowl. `sample_height` carries
                    // the crater overlay, so the tuck follows every hole; off the
                    // playfield (the far ring tucking under the near ring) the analytic
                    // surface IS the finer surface and the plain tuck stands.
                    if x >= 0.0
                        && x <= extent_x
                        && z >= 0.0
                        && z <= extent_z
                        && let Some(live) = heightmap.sample_height(x, z)
                    {
                        y = live;
                    }
                    y -= APRON_TUCK_M;
                }
                let step = cell_m * 0.5;
                let normal = Vec3::new(
                    beyond(x - step, z) - beyond(x + step, z),
                    2.0 * step,
                    beyond(x, z - step) - beyond(x, z + step),
                )
                .normalize();
                vertex_index[slot] = vertices.len() as u32;
                vertices.push(make_vertex(x, z, y, normal));
            }
            vertex_index[slot]
        };
        for iz in 0..n - 1 {
            for ix in 0..n - 1 {
                let (x0, z0) = (position_of(ix), position_of(iz));
                let (x1, z1) = (position_of(ix + 1), position_of(iz + 1));
                // Skip cells fully inside the hole (buried under the finer surface, minus the
                // one-cell overlap band) and cells fully outside this pass's outer square.
                if inside_frame(x0, z0, hole_out_m - cell_m)
                    && inside_frame(x1, z1, hole_out_m - cell_m)
                {
                    continue;
                }
                if !inside_frame(x0, z0, out_m) && !inside_frame(x1, z1, out_m) {
                    continue;
                }
                let i00 = corner(vertices, &mut vertex_index, ix, iz);
                let i10 = corner(vertices, &mut vertex_index, ix + 1, iz);
                let i01 = corner(vertices, &mut vertex_index, ix, iz + 1);
                let i11 = corner(vertices, &mut vertex_index, ix + 1, iz + 1);
                indices.extend_from_slice(&[i00, i01, i10, i10, i01, i11]);
            }
        }
    }
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
            // One stencil for every patch vertex — and that is SOUND, not sloppy: since
            // the surface-parity fix the sampled ground is piecewise planar, so a central
            // difference at a NODE equals the mean of the adjacent plane slopes at ANY
            // stencil width — the patch's fine stencil and the base grid's 5 m rule agree
            // there by construction (the audit's "crater lighting crease" dissolved when
            // the sim moved onto the drawn triangles; the coincident-vertex shading lock
            // in this file's tests measured it and now stands guard).
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
    let n = terrain::grass_patchwork_noise(wx, wz);
    if n > 0.5 {
        base.lerp(dry, ((n - 0.5) * 2.4).min(1.0))
    } else {
        base.lerp(lush, ((0.5 - n) * 2.4).min(1.0) * 0.85)
    }
}

/// The painted albedo + finish for a road surface. Dirt and ballast stay near-matte earth
/// and crushed stone; cobble reads as grey granite setts with a faint sheen — still inside
/// the presentation gate's saturation window, distinct from both by tone alone.
pub(crate) fn road_surface_tone(surface: RoadSurface) -> (Vec3, f32) {
    match surface {
        RoadSurface::Dirt => (Vec3::new(0.40, 0.34, 0.24), 0.05),
        RoadSurface::Ballast => (Vec3::new(0.34, 0.31, 0.28), 0.08),
        RoadSurface::Cobble => (Vec3::new(0.31, 0.31, 0.33), 0.12),
    }
}

/// The road tone at a world point, if any road reaches it: `(tone, gloss, blend)` with the
/// blend feathering from full paint over the core to nothing at the authored edge.
fn road_paint(roads: &[Road], wx: f32, wz: f32) -> Option<(Vec3, f32, f32)> {
    let mut best: Option<(Vec3, f32, f32)> = None;
    for road in roads {
        // The bounds check before the polyline walk (terrain::road_bound): this runs per
        // ground VERTEX — playfield plus both apron rings — and past the box the blend is
        // exactly 0.0, so the skip cannot move a single painted texel.
        let bound = terrain::road_bound(road);
        if wx < bound[0] || wx > bound[2] || wz < bound[1] || wz > bound[3] {
            continue;
        }
        let half = road.width_m * 0.5;
        let distance = road.distance_to(wx, wz);
        if distance >= half {
            continue;
        }
        // Full tone over the inner core, feathered out to the grass at the edge — the SAME
        // falloff `terrain::road_blend` gives the ground rule, so how a road wears and how it
        // looks cannot drift apart.
        let blend = terrain::road_blend(road, wx, wz);
        let (tone, gloss) = road_surface_tone(road.surface);
        if best.map(|(_, _, b)| blend > b).unwrap_or(true) {
            best = Some((tone, gloss, blend));
        }
    }
    best
}

#[cfg(test)]
mod tests {

    use super::*;

    /// The shading-watertight invariant: any two triangle-referenced vertices standing at
    /// the SAME position must carry the SAME normal, or a lighting seam rings the geometry
    /// even though the positions are watertight. The audit filed the crater patch boundary
    /// as a crease (fine stencil vs the base grid's 5 m rule) — instrumenting this lock
    /// proved the surface-parity fix had DISSOLVED it: on the piecewise-planar sampled
    /// ground a node's central difference equals the mean of the adjacent plane slopes at
    /// any stencil width, so patch and base normals agree by construction. The lock stays
    /// as the guard: any future surface or stencil change that reopens the seam is a red
    /// assertion, not a subtle ring of light.
    #[test]
    fn coincident_referenced_vertices_shade_identically() {
        // A CURVED field: the crease is a stencil-width artefact, and on linear ground
        // every stencil measures the same gradient — curvature is what separates the
        // patch's 0.625 m stencil from the base grid's 5 m one (the first falsification
        // run proved it by NOT biting on a plane).
        // Curvature at STROKE scale (the sigma-3..5 lips real maps carry): on gentle
        // ground the two stencil widths agree to a fraction of a degree and the crease is
        // invisible — it is the tight authored relief where the 5 m and 0.625 m stencils
        // disagree by whole degrees, so that is what the fixture carries.
        let mut heightmap = terrain::heightmap_from_fn(41, 5.0, |x, z| {
            10.0 + (x * 0.42).sin() * 1.7 + (z * 0.37).cos() * 1.4 + x * 0.03
        });
        let records = [
            terrain::CraterRecord::from_world(60.0, 60.0, 2.0, 0.8, 0),
            // Wide enough to cut TWO adjacent cells: patch-to-patch boundary exercised.
            terrain::CraterRecord::from_world(120.0, 122.5, 4.0, 1.2, 0),
        ];
        heightmap.set_craters(&records);
        let (vertices, indices) = terrain_scene_mesh_full(&heightmap, WaterView::DRY, &[], None);
        let mut referenced = vec![false; vertices.len()];
        for &index in &indices {
            referenced[index as usize] = true;
        }
        let mut by_position: std::collections::HashMap<(i64, i64, i64), [f32; 3]> =
            Default::default();
        let mut coincident = 0;
        for (vertex, used) in vertices.iter().zip(&referenced) {
            if !used {
                continue;
            }
            let key = (
                (vertex.position[0] * 1024.0).round() as i64,
                (vertex.position[1] * 1024.0).round() as i64,
                (vertex.position[2] * 1024.0).round() as i64,
            );
            match by_position.entry(key) {
                std::collections::hash_map::Entry::Vacant(slot) => {
                    slot.insert(vertex.normal);
                }
                std::collections::hash_map::Entry::Occupied(slot) => {
                    coincident += 1;
                    let seen = slot.get();
                    let dot = seen[0] * vertex.normal[0]
                        + seen[1] * vertex.normal[1]
                        + seen[2] * vertex.normal[2];
                    assert!(
                        dot > 0.9999,
                        "a lighting crease at {:?}: {:?} vs {:?}",
                        vertex.position,
                        seen,
                        vertex.normal
                    );
                }
            }
        }
        assert!(coincident > 30, "the sweep actually met shared seams ({coincident})");
    }

    /// The apron-crater seam (Atlas audit P3): a shell crater at the red line digs the
    /// playfield below the analytic continuation — an apron tucked under the UN-cratered
    /// height floated as a plate inside the bowl (falsified: reverting the fix trips this
    /// probe at 9.6 over a 9.325 bowl floor). With the tuck seated under the LIVE surface,
    /// every triangle-referenced in-playfield vertex sits at-or-under the cratered ground.
    #[test]
    fn the_apron_tucks_under_a_crater_at_the_red_line() {
        let mut heightmap = terrain::HeightMap::flat(41, 41, 5.0, 10.0).expect("flat map");
        let [extent_x, extent_z] = heightmap.extent_m();
        // Centred so an APRON-grid vertex (the 12 m lattice from -240) lands inside the
        // bowl proper - the probe must meet the ring, not only playfield nodes.
        let record = terrain::CraterRecord::from_world(
            extent_x - 8.0,
            106.0,
            4.0,
            1.2,
            terrain::CRATER_KIND_HIGH_EXPLOSIVE,
        );
        heightmap.set_craters(std::slice::from_ref(&record));
        let beyond = |_: f32, _: f32| 10.0_f32;
        let (vertices, indices) =
            terrain_scene_mesh_full(&heightmap, WaterView::DRY, &[], Some(&beyond));
        // Only vertices a triangle actually references: cut cells leave their un-cratered
        // base corners in the buffer unreferenced by construction, and a buffer entry no
        // triangle draws cannot float over anything.
        let mut referenced = vec![false; vertices.len()];
        for &index in &indices {
            referenced[index as usize] = true;
        }
        let mut checked = 0;
        for (vertex, used) in vertices.iter().zip(&referenced) {
            if !used {
                continue;
            }
            let [x, y, z] = vertex.position;
            if !(0.0..=extent_x).contains(&x) || !(0.0..=extent_z).contains(&z) {
                continue;
            }
            let dx = x - record.x_m();
            let dz = z - record.z_m();
            if (dx * dx + dz * dz).sqrt() >= record.influence_radius_m() {
                continue;
            }
            checked += 1;
            let ground = heightmap.sample_height(x, z).expect("inside the map");
            assert!(
                y <= ground + 1.0e-3,
                "a vertex floats over the bowl at ({x}, {z}): {y} over {ground}"
            );
        }
        assert!(checked > 8, "the sweep actually met the bowl ({checked} vertices)");
    }

    #[test]
    fn the_border_apron_continues_the_ground_to_the_haze_on_every_authored_map() {
        for map in [
            map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2),
            map_forge::battlefield(terrain::MapId::BystraValley),
        ] {
            let [extent_x, extent_z] = map.heightmap.extent_m();
            let (vertices, indices) = battlefield_ground_mesh(&map);
            let (mut min_x, mut max_x, mut min_z, mut max_z) =
                (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            for vertex in &vertices {
                min_x = min_x.min(vertex.position[0]);
                max_x = max_x.max(vertex.position[0]);
                min_z = min_z.min(vertex.position[2]);
                max_z = max_z.max(vertex.position[2]);
            }
            assert!(
                min_x < -1400.0
                    && max_x > extent_x + 1400.0
                    && min_z < -1400.0
                    && max_z > extent_z + 1400.0,
                "{}: the apron must reach the haze on all four sides",
                map.id
            );
            // Budget: the apron is two chunk-culled rings, not a second map's worth of mesh.
            let playfield_vertices = map.heightmap.width() * map.heightmap.height();
            assert!(
                vertices.len() > playfield_vertices && vertices.len() < playfield_vertices + 40_000,
                "{}: apron adds a bounded ring, got {} vertices over {playfield_vertices}",
                map.id,
                vertices.len()
            );
            assert!(indices.iter().all(|&index| (index as usize) < vertices.len()));
            // Determinism: craters aside, the same map builds the same ground.
            assert_eq!(vertices.len(), battlefield_ground_mesh(&map).0.len());
        }
    }

    #[test]
    fn the_beyond_surface_is_exact_at_the_border_nodes() {
        // The apron stands on each map's analytic continuation; at the border the continuation
        // IS the surface the heightmap sampled, so the seam matches by construction. This
        // locks that contract — a map whose beyond-function drifts from its own heightmap
        // would open a visible step along the red line.
        for map in [
            map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2),
            map_forge::battlefield(terrain::MapId::BystraValley),
        ] {
            let beyond = beyond_border_height(&map);
            let [extent_x, extent_z] = map.heightmap.extent_m();
            let cell = map.heightmap.cell_size_m();
            for i in 0..map.heightmap.width() {
                let t = i as f32 * cell;
                for (x, z) in [(0.0, t), (extent_x, t), (t, 0.0), (t, extent_z)] {
                    let sampled =
                        map.heightmap.sample_height(x, z).expect("border node is in the map");
                    assert!(
                        (beyond(x, z) - sampled).abs() < 0.01,
                        "{}: beyond surface must meet the heightmap at ({x}, {z}): {} vs {sampled}",
                        map.id,
                        beyond(x, z)
                    );
                }
            }
        }
    }

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
        let map = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
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
    fn a_wounded_wall_wears_its_scars_and_spills_rubble_at_its_foot() {
        let map = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let building = map
            .static_cover
            .iter()
            .position(|cover| cover.kind == StaticCoverKind::FarmBuilding)
            .expect("map has a farm building");
        let he_bite = terrain::CoverScar {
            cover: building as u16,
            face: 2,
            u_q: 128,
            v_q: 100,
            radius_q: 20, // a metre-wide bite
            kind: terrain::COVER_SCAR_KIND_HIGH_EXPLOSIVE,
        };

        let clean = battlefield_statics_mesh(&map, &[]);
        let wounded = battlefield_statics_mesh_with_scars(&map, &[], &[he_bite]);
        assert!(
            wounded.0.len() > clean.0.len(),
            "the wound adds geometry: {} vs {}",
            wounded.0.len(),
            clean.0.len()
        );

        // The spalled masonry lies at the wall's FOOT: exposed-brick vertices near ground
        // level that the clean bake does not have (scars append inside the cover loop, so
        // the tail of the buffer is backdrop — hunt by the chunk's tone instead).
        let object = &map.static_cover[building];
        let ground_y = object.center[1] - object.half_extents_m[1];
        let rubble_tone = HE_EXPOSED_TONE;
        let count_rubble = |mesh: &[SceneVertex]| {
            mesh.iter().filter(|v| v.color == rubble_tone && v.position[1] < ground_y + 0.8).count()
        };
        assert!(
            count_rubble(&wounded.0) > count_rubble(&clean.0),
            "rubble heaps at the foot of the wall"
        );

        // Inny Poziom Z5: the wound is a RECESS dug into the box, never a stamp proud of it.
        // Every wound vertex stays inside the collision box (a 1 cm lift for the shed-render
        // fan against z-fighting), the bite's floor sits a real depth behind the face, and
        // the rim is irregular — no square, no circle.
        let center = glam::Vec3::from_array(object.center);
        let half = glam::Vec3::from_array(object.half_extents_m);
        let face_z = center.z + half.z;
        // Scars append inside the cover loop, so hunt the recess by its own tones.
        let wound: Vec<&SceneVertex> = wounded
            .0
            .iter()
            .filter(|v| {
                [WOUND_FLOOR_TONE, WOUND_SIDE_TONE, WOUND_SHED_TONE].contains(&v.color)
                    || (v.color == HE_EXPOSED_TONE && v.position[1] > ground_y + 0.8)
            })
            .collect();
        assert!(wound.len() >= 16 * 6, "the recess and its ring are there: {}", wound.len());
        for v in &wound {
            let p = glam::Vec3::from_array(v.position);
            let d = (p - center).abs() - half;
            assert!(d.max_element() <= 0.011, "a wound vertex left the box by {d}");
        }
        let deepest = wound
            .iter()
            .filter(|v| v.color == WOUND_FLOOR_TONE)
            .map(|v| face_z - v.position[2])
            .fold(0.0_f32, f32::max);
        assert!(deepest >= 0.09, "the bite is dug into the wall, deepest {deepest} m");
        let rim_radii: Vec<f32> = wound
            .iter()
            .filter(|v| (v.position[2] - face_z).abs() < 0.012 && v.color == HE_EXPOSED_TONE)
            .map(|v| {
                let p = glam::Vec3::from_array(v.position);
                ((p.x - (center.x + (128.0 / 255.0 * 2.0 - 1.0) * half.x)).powi(2)
                    + (p.y - (center.y + (100.0 / 255.0 * 2.0 - 1.0) * half.y)).powi(2))
                .sqrt()
            })
            .collect();
        let (rmin, rmax) =
            rim_radii.iter().fold((f32::MAX, 0.0_f32), |(a, b), r| (a.min(*r), b.max(*r)));
        assert!(
            rim_radii.len() >= 12 && rmax / rmin > 1.15,
            "an irregular rim, {rmin}..{rmax} over {} vertices",
            rim_radii.len()
        );

        // A collapsed wall drops its scars with it: rubble phase ignores the ledger.
        let mut states = vec![0u8; map.static_cover.len()];
        states[building] = 1;
        let collapsed_clean = battlefield_statics_mesh(&map, &states);
        let collapsed_scarred = battlefield_statics_mesh_with_scars(&map, &states, &[he_bite]);
        assert_eq!(collapsed_clean.0.len(), collapsed_scarred.0.len());
    }

    /// Levelling a tree line empties the volume it occupied and leaves wreckage on the ground.
    ///
    /// The oaks that also dressed it are no longer part of this bake — they draw from the
    /// instanced LOD path, which drops them on the same rule (`tree_lod::tree_frame_objects`,
    /// locked by its own test). What this locks is the bake's half: nothing of the standing
    /// line survives up in its box, and something does survive down on the ground.
    #[test]
    fn a_cleared_tree_line_empties_its_volume_and_leaves_wreckage() {
        let map = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let tree_line = map
            .static_cover
            .iter()
            .position(|cover| cover.kind == StaticCoverKind::TreeLine)
            .expect("prokhorovka has a tree line");

        let intact = battlefield_scene_mesh(&map);
        let mut states = vec![0u8; map.static_cover.len()];
        states[tree_line] = 2; // gone
        let cleared = battlefield_scene_mesh_with_cover_states(&map, &states);

        let box_ = &map.static_cover[tree_line];
        let inside_above = |mesh: &(Vec<SceneVertex>, Vec<u32>), frac: f32| {
            mesh.0
                .iter()
                .filter(|v| {
                    (v.position[0] - box_.center[0]).abs() <= box_.half_extents_m[0]
                        && (v.position[2] - box_.center[2]).abs() <= box_.half_extents_m[2]
                        && v.position[1] >= box_.center[1] + box_.half_extents_m[1] * frac
                })
                .count()
        };
        assert!(
            inside_above(&intact, 0.5) > 0,
            "the standing line fills the upper half of its box"
        );
        assert_eq!(
            inside_above(&cleared, 0.5),
            0,
            "levelling it empties that half — the box is gone, not merely shortened"
        );
        assert!(
            inside_above(&cleared, -1.0) > 0,
            "and leaves stumps and a fallen trunk on the ground it stood on"
        );
    }

    /// Fizyczny Świat P11: a crushed tree line is wreckage, not a vacuum — stumps stand where
    /// its trees stood and at least one trunk lies along the run, all of it low to the ground.
    #[test]
    fn a_crushed_tree_line_leaves_stumps_and_fallen_trunks() {
        let map = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let tree_line = map
            .static_cover
            .iter()
            .position(|cover| cover.kind == StaticCoverKind::TreeLine)
            .expect("prokhorovka has a tree line");
        let object = &map.static_cover[tree_line];
        let mut states = vec![0u8; map.static_cover.len()];
        states[tree_line] = 2; // gone (shelled clear or crushed under a hull — same phase)

        let cleared = battlefield_scene_mesh_with_cover_states(&map, &states);
        let bark = [0.26, 0.20, 0.13];
        let ground_top = object.center[1] + object.half_extents_m[1];
        let wreckage: Vec<_> = cleared
            .0
            .iter()
            .filter(|v| {
                v.color == bark
                    && (v.position[0] - object.center[0]).abs() <= object.half_extents_m[0] + 1.0
                    && (v.position[2] - object.center[2]).abs() <= object.half_extents_m[2] + 1.0
            })
            .collect();
        assert!(!wreckage.is_empty(), "the crush leaves stumps and trunks behind");
        for vertex in &wreckage {
            assert!(
                vertex.position[1] < ground_top,
                "wreckage lies LOW — nothing pokes above the old canopy box: y {}",
                vertex.position[1]
            );
        }
    }

    /// Inny Poziom F3: the standing line is trees over the hedge body, not boxes on sticks.
    /// The cover-box bake draws the body alone (one surfaced box, 24 vertices) — a shrub mass
    /// over the tallest hull in the fleet and up to the crown hulls' bottom, never the old
    /// crown slab; every other vertex in the intact line's footprint is a planted tree — bark
    /// or foliage by surface role — and there are many of them.
    #[test]
    fn a_standing_tree_line_is_planted_trees_over_the_hedge_body_and_no_slab() {
        let map = map_forge::battlefield(terrain::MapId::BystraValley);
        for cover in map.static_cover.iter().filter(|cover| cover.kind == StaticCoverKind::TreeLine)
        {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            append_cover_box(&mut vertices, &mut indices, cover);
            assert_eq!(vertices.len(), 24, "{}: the box bake is the hedge body alone", cover.id);
            let floor = cover.center[1] - cover.half_extents_m[1];
            let box_height = cover.half_extents_m[1] * 2.0;
            let body_top = vertices.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max) - floor;
            assert!(
                body_top >= 3.1 && body_top <= box_height * TREE_LINE_BODY_HEIGHT + 1.0e-3,
                "{}: the body hides a Tiger II (3.09 m) and stops under the crowns: {body_top:.2}",
                cover.id
            );
            assert!(
                body_top >= box_height * crate::tree_line::HULL_BOTTOM,
                "{}: the body reaches the crown hulls, so the wall has no window between them",
                cover.id
            );

            let mut planted = Vec::new();
            let mut planted_indices = Vec::new();
            crate::tree_line::push_tree_line_trees(&mut planted, &mut planted_indices, &map, cover);
            assert!(planted.len() > 24 * 8, "{}: the line is planted, not sketched", cover.id);
            for vertex in &planted {
                assert!(
                    vertex.surface == renderer_api::surface_role::BARK
                        || vertex.surface == renderer_api::surface_role::FOLIAGE,
                    "{}: a planted line is bark and foliage — a vertex with role {} is a slab",
                    cover.id,
                    vertex.surface
                );
            }
        }
    }

    /// Inny Poziom F3: a felled line leaves a stump at every station it was planted from
    /// (the heartwood cap counts them), on top of the stumps of any scenery it hosted.
    #[test]
    fn a_felled_tree_line_leaves_a_stump_where_each_planted_tree_stood() {
        let map = map_forge::battlefield(terrain::MapId::BystraValley);
        let heartwood = [0.45, 0.36, 0.24];
        for cover in map.static_cover.iter().filter(|cover| cover.kind == StaticCoverKind::TreeLine)
        {
            let stations = crate::tree_line::tree_line_stations(&map, cover).len();
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            append_felled_tree_line(&mut vertices, &mut indices, &map, cover);
            let caps = vertices.iter().filter(|v| v.color == heartwood).count() / 24;
            assert!(
                caps >= stations && stations >= 5,
                "{}: {stations} trees planted, {caps} stumps left",
                cover.id
            );
        }
    }

    /// Świat 2.0 PR1: a felled oak leaves a bole-scale stump, not a 26 cm diorama peg. The
    /// stump's half-width tracks `TreeSpecies::Oak.trunk_radius` (~0.52 m).
    #[test]
    fn a_felled_oak_leaves_a_bole_scale_stump() {
        let map = map_forge::battlefield(terrain::MapId::BystraValley);
        let trunk_index = map
            .static_cover
            .iter()
            .position(|cover| cover.kind == StaticCoverKind::TreeTrunk)
            .expect("bystra oaks compile trunk cover");
        let cover = &map.static_cover[trunk_index];
        let mut states = vec![0u8; map.static_cover.len()];
        states[trunk_index] = 2;
        let cleared = battlefield_scene_mesh_with_cover_states(&map, &states);
        let bark = [0.26, 0.20, 0.13];
        let ground_y = cover.center[1] - cover.half_extents_m[1];
        // Stump verts sit near the trunk's XZ and within ~1.2 m of the ground (half-height
        // ≤ 0.55 + cap). The fallen log also shares the bark colour but stretches far — keep
        // only the near-footprint cluster.
        let stump: Vec<_> = cleared
            .0
            .iter()
            .filter(|v| {
                v.color == bark
                    && (v.position[0] - cover.center[0]).abs() < 1.2
                    && (v.position[2] - cover.center[2]).abs() < 1.2
                    && v.position[1] < ground_y + 1.2
            })
            .collect();
        assert!(!stump.is_empty(), "a felled oak leaves a stump");
        let max_half = stump
            .iter()
            .map(|v| {
                (v.position[0] - cover.center[0]).abs().max((v.position[2] - cover.center[2]).abs())
            })
            .fold(0.0_f32, f32::max);
        let oak_radius = world_forge::tree::TreeSpecies::Oak.trunk_radius();
        assert!(
            max_half >= oak_radius * 0.9,
            "stump half-width {max_half} undershoots the oak butt {oak_radius}"
        );
        assert!(
            max_half <= oak_radius * 1.15,
            "stump half-width {max_half} overshoots the oak butt {oak_radius}"
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
        for map in [
            map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2),
            map_forge::battlefield(terrain::MapId::BystraValley),
            map_forge::battlefield(terrain::MapId::Ostrogorsk),
            map_forge::battlefield(terrain::MapId::OrlinyPereval),
        ] {
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

    /// Świat 2.0 PR 5's load-bearing proof, the other direction of the treeline promise:
    /// every TreeLine collision box on every shipped map TOWERS over the trees it hosts —
    /// the LOS wall reaches at least the tallest crown standing inside it (15–25 m, the
    /// register's mature band). Before PR 5 the boxes were 8.4–10 m slabs under 17–23 m
    /// trees: a crew could be spotted OVER a wall its eyes could not see through.
    #[test]
    fn tree_line_boxes_contain_the_trees_they_host() {
        for map_id in [
            terrain::MapId::BystraValley,
            terrain::MapId::Ostrogorsk,
            terrain::MapId::ProkhorovkaHill252_2,
            terrain::MapId::OrlinyPereval,
        ] {
            let map = map_forge::battlefield(map_id);
            for cover in
                map.static_cover.iter().filter(|cover| cover.kind == StaticCoverKind::TreeLine)
            {
                let box_top = cover.center[1] + cover.half_extents_m[1];
                for instance in &map.scenery {
                    let in_footprint = (instance.position[0] - cover.center[0]).abs()
                        <= cover.half_extents_m[0]
                        && (instance.position[2] - cover.center[2]).abs()
                            <= cover.half_extents_m[2];
                    if !in_footprint {
                        continue;
                    }
                    let Some(species) = tree_species_for_scenery(instance.kind) else {
                        continue;
                    };
                    let rendered_top = if instance.kind == terrain::SceneryKind::Oak {
                        crate::tree_lod::battle_tree_rendered_top_m(instance.scale)
                    } else {
                        // The statics-bake path: per-position seed, Mid rung, no sink.
                        let seed = instance.position[0].to_bits() as u64
                            ^ ((instance.position[2].to_bits() as u64) << 32);
                        world_forge::tree::bake_tree_lod(
                            species,
                            seed,
                            world_forge::tree::TreeLod::Mid,
                        )
                        .canopy
                        .bounds()
                        .map(|bounds| bounds.max.y * instance.scale)
                        .unwrap_or(0.0)
                    };
                    let tree_top = instance.position[1] + rendered_top;
                    assert!(
                        tree_top <= box_top + 0.05,
                        "{map_id:?}/{}: a {:?} crown reaches {:.1} m but the LOS wall tops at {:.1} m",
                        cover.id,
                        instance.kind,
                        tree_top,
                        box_top,
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
        for map in [
            map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2),
            map_forge::battlefield(terrain::MapId::BystraValley),
        ] {
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
        let battlefield = map_forge::battlefield(terrain::MapId::BystraValley);
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
        let map = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let barn = map
            .static_cover
            .iter()
            .find(|c| c.kind == StaticCoverKind::FarmBuilding)
            .expect("prokhorovka has barns");
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        append_cover_box(&mut vertices, &mut indices, barn);
        let windows =
            vertices.iter().filter(|v| v.color == crate::world_material::WINDOW.0).count();
        let doors = vertices.iter().filter(|v| v.color == crate::world_material::DOOR.0).count();
        assert!(windows >= 8, "a barn wall carries windows, got {windows} verts");
        assert!(doors >= 4, "a door stands proud of the plaster, got {doors} verts");
        // Glass answers the sky harder than the plaster around it.
        assert!(crate::world_material::WINDOW.1 > 0.10, "window glaze outshines the wall");
    }

    /// Materia Świata 3: a building names its materials down the surface lane — rendered
    /// walls, coursed roofs, a plank door — while glass keeps the untreated look. The lane
    /// is what the scene shader dispatches its procedural detail on.
    #[test]
    fn buildings_name_their_surfaces_down_the_lane() {
        use renderer_api::surface_role;
        let map = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
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
            if vertex.color == crate::world_material::WINDOW.0 {
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
        let map = map_forge::battlefield(terrain::MapId::BystraValley);
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
        let map = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
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

    /// Cobble (urban-map program PR-05) reads as grey granite setts: even channels with a
    /// cool bias, a faint sheen above dirt and ballast but still far from a mirror, and a
    /// tone distinct from both by color alone.
    #[test]
    fn cobble_reads_grey_setts_distinct_from_dirt_and_ballast() {
        let (dirt, dirt_gloss) = road_surface_tone(RoadSurface::Dirt);
        let (ballast, ballast_gloss) = road_surface_tone(RoadSurface::Ballast);
        let (cobble, cobble_gloss) = road_surface_tone(RoadSurface::Cobble);
        assert!((cobble.x - cobble.y).abs() < 0.03, "setts are grey, not tinted earth");
        assert!(cobble.z >= cobble.x, "the grey leans cool, never warm like dirt");
        assert!(cobble.distance(dirt) > 0.08, "cobble must not read as dirt");
        assert!(cobble.distance(ballast) > 0.04, "cobble must not read as ballast");
        assert!(
            cobble_gloss > ballast_gloss && cobble_gloss > dirt_gloss,
            "setts carry the faint worn sheen"
        );
        assert!(cobble_gloss <= 0.15, "a street is stone, not a mirror");
    }

    /// The style-derivation table (urban-map PR-08): explicit names beat proportions, the
    /// tenement is reachable both ways, and every legacy rule still lands where it always
    /// did — a new style must never silently re-dress an old map.
    #[test]
    fn the_style_table_names_the_tenement_and_keeps_the_legacy_rules() {
        use world_forge::building::BuildingStyle;
        let by = |id: &str, half: [f32; 3]| derived_building_style(id, Vec3::from_array(half));
        assert_eq!(by("ostrogorsk_church", [5.0, 7.0, 6.5]), BuildingStyle::Church);
        assert_eq!(by("old_windmill", [4.0, 6.0, 4.0]), BuildingStyle::Windmill);
        assert_eq!(by("tenement_row_a", [9.0, 4.0, 5.0]), BuildingStyle::Tenement);
        assert_eq!(by("elevator_south", [6.0, 9.5, 6.0]), BuildingStyle::Tenement);
        assert_eq!(by("mill_factory_south", [14.0, 6.0, 9.0]), BuildingStyle::FactoryHall);
        assert_eq!(
            by("long_low_hall", [14.0, 2.7, 9.0]),
            BuildingStyle::Barn,
            "no proportion ever invents a factory - halls stand by name"
        );
        assert_eq!(by("barn_2", [7.0, 2.7, 4.2]), BuildingStyle::Barn);
        assert_eq!(by("town_house_c1", [4.5, 3.4, 4.5]), BuildingStyle::Townhouse);
        assert_eq!(by("cottage_9", [4.0, 2.6, 3.2]), BuildingStyle::Cottage);
    }

    /// The coursed wall (urban-map PR-10): every box inside the collision footprint, a
    /// lighter coping crowning the run, and piers adding real geometry beyond one plain box.
    #[test]
    fn the_stone_wall_wears_courses_inside_its_box() {
        let wall = StaticCoverObject {
            id: "yard_wall_probe".into(),
            name: "yard wall".into(),
            kind: StaticCoverKind::StoneWall,
            center: [0.0, 1.1, 0.0],
            half_extents_m: [0.4, 1.1, 7.0],
        };
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        append_cover_box(&mut vertices, &mut indices, &wall);
        assert!(vertices.len() > 24, "coping + piers add geometry beyond one box");
        for vertex in &vertices {
            assert!(
                vertex.position[0].abs() <= 0.4 + 1.0e-3
                    && vertex.position[2].abs() <= 7.0 + 1.0e-3
                    && vertex.position[1] <= 2.2 + 1.0e-3
                    && vertex.position[1] >= -1.0e-3,
                "wall geometry must stay inside the footprint, got {:?}",
                vertex.position
            );
        }
        let coping_lit =
            vertices.iter().any(|v| v.position[1] > 2.0 && v.color[0] > 0.48 && v.color[1] > 0.46);
        assert!(coping_lit, "the crown of the run wears the lighter coping stone");
    }

    /// Świat 2.0 PR 7: RailCover is a revetment (or a coursed tower for square silos), never
    /// a single slab — more than one box of geometry, every vertex inside the collision AABB,
    /// and a lighter coping/cap on the crown. Locked on a berm run and an Ostrogorsk silo.
    #[test]
    fn the_rail_cover_wears_a_revetment_inside_its_box() {
        let cases = [
            StaticCoverObject {
                id: "berm_probe".into(),
                name: "berm parapet".into(),
                kind: StaticCoverKind::RailCover,
                center: [0.0, 0.9, 0.0],
                half_extents_m: [1.4, 0.9, 12.0],
            },
            StaticCoverObject {
                id: "silo_probe".into(),
                name: "elevator silo".into(),
                kind: StaticCoverKind::RailCover,
                center: [0.0, 5.5, 0.0],
                half_extents_m: [2.2, 5.5, 2.2],
            },
        ];
        for cover in &cases {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            append_cover_box(&mut vertices, &mut indices, cover);
            assert!(
                vertices.len() > 24,
                "{}: revetment/tower adds geometry beyond one box, got {}",
                cover.id,
                vertices.len()
            );
            let center = Vec3::from_array(cover.center);
            let half = Vec3::from_array(cover.half_extents_m);
            for vertex in &vertices {
                let delta = (Vec3::from_array(vertex.position) - center).abs();
                assert!(
                    delta.x <= half.x + 1.0e-3
                        && delta.y <= half.y + 1.0e-3
                        && delta.z <= half.z + 1.0e-3,
                    "{} draws outside its collision box at {:?}",
                    cover.id,
                    vertex.position
                );
            }
            let crown_y = center.y + half.y - 0.35;
            let lit_crown = vertices
                .iter()
                .any(|v| v.position[1] >= crown_y && v.color[0] >= 0.48 && v.color[1] >= 0.46);
            assert!(lit_crown, "{}: the crown wears the lighter coping/cap stone", cover.id);
        }
    }

    /// Teren W3b: the Svan watchtower reads as the landmark it is — pale limewashed shaft,
    /// dark machicolation voids, slate cap — with every vertex inside the collision box and
    /// the silhouette actually FILLING the box's height (a landmark that stops short of its
    /// own collider would be a lie both ways).
    #[test]
    fn the_watchtower_wears_lime_voids_and_slate_inside_its_box() {
        let tower = StaticCoverObject {
            id: "col_watchtower_probe".into(),
            name: "watchtower".into(),
            kind: StaticCoverKind::StoneTower,
            center: [0.0, 10.0, 0.0],
            half_extents_m: [2.6, 10.0, 2.6],
        };
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        append_cover_box(&mut vertices, &mut indices, &tower);
        assert!(vertices.len() > 24 * 6, "plinth, courses, slits, crown and cap add up");
        let mut top = f32::MIN;
        for vertex in &vertices {
            assert!(
                vertex.position[0].abs() <= 2.6 + 1.0e-3
                    && vertex.position[2].abs() <= 2.6 + 1.0e-3
                    && vertex.position[1] <= 20.0 + 1.0e-3
                    && vertex.position[1] >= -1.0e-3,
                "tower geometry must stay inside the collision box, got {:?}",
                vertex.position
            );
            top = top.max(vertex.position[1]);
        }
        assert!(top >= 20.0 - 1.0e-3, "the cap reaches the box top, got {top}");
        let limewash = vertices.iter().any(|v| v.color[0] > 0.52 && v.color[2] > 0.45);
        assert!(limewash, "the shaft wears the pale limewash that makes the landmark");
        let voids = vertices.iter().filter(|v| v.color[0] < 0.1).count();
        assert!(voids >= 24 * 8, "loopholes and machicolation mouths read as dark voids");
        let slate_cap = vertices
            .iter()
            .any(|v| v.position[1] > 18.5 && v.color[2] > v.color[0] && v.color[0] < 0.3);
        assert!(slate_cap, "the crown carries the cool slate cap");
    }

    /// The breach (urban-map PR-10): a Gone wall leaves a knee-high toppled course — inside
    /// the old footprint, far below cover height, deterministic per id, and absent while the
    /// wall stands.
    #[test]
    fn a_breached_wall_leaves_a_knee_high_toppled_course() {
        let wall = StaticCoverObject {
            id: "yard_wall_probe".into(),
            name: "yard wall".into(),
            kind: StaticCoverKind::StoneWall,
            center: [10.0, 1.1, 5.0],
            half_extents_m: [0.4, 1.1, 7.0],
        };
        let mut first = (Vec::new(), Vec::new());
        append_toppled_wall(&mut first.0, &mut first.1, &wall);
        assert!(!first.0.is_empty(), "a breach is bricks at your feet, not a vacuum");
        for vertex in &first.0 {
            assert!(
                (vertex.position[0] - 10.0).abs() <= 0.4 + 1.0e-3
                    && (vertex.position[2] - 5.0).abs() <= 7.0 + 1.0e-3,
                "tumbled slabs stay inside the old footprint, got {:?}",
                vertex.position
            );
            assert!(
                vertex.position[1] <= 0.5,
                "the toppled course stays knee-high (honest-blockers rule), got {:?}",
                vertex.position
            );
        }
        let mut second = (Vec::new(), Vec::new());
        append_toppled_wall(&mut second.0, &mut second.1, &wall);
        assert_eq!(
            first.0.len(),
            second.0.len(),
            "the same wall always falls the same way (deterministic per id)"
        );
    }

    /// The grass is a patchwork, not a lawn: across the open steppe the green varies by
    /// visible drifts, deterministically — the same map builds the same field every time.
    #[test]
    fn grass_patchwork_varies_and_is_deterministic() {
        let map = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
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
        let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        assert!(!battlefield.static_cover.is_empty(), "map should carry static cover");

        let (terrain_vertices, _) = terrain_scene_mesh(&battlefield.heightmap);
        let (vertices, indices) = battlefield_scene_mesh(&battlefield);

        assert!(vertices.len() > terrain_vertices.len(), "cover must add geometry");
        assert!(indices.iter().all(|&index| (index as usize) < vertices.len()));
        for cover in &battlefield.static_cover {
            // An oak's bole is the one box the BAKE does not draw — its visual is the
            // instanced tree, so the promise is kept by proving a tree actually stands in it.
            // The doctrine is unchanged: every box is something the player can see.
            if cover.kind == StaticCoverKind::TreeTrunk {
                assert!(
                    dressed_by_an_oak(&battlefield, cover),
                    "trunk box {} must have the tree it claims to be",
                    cover.id
                );
                continue;
            }
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

    /// A `TreeTrunk` box is honest only if a procedural oak really stands in it: same
    /// footprint, same ground. This is the substitute proof for the geometry check the bake
    /// cannot make.
    fn dressed_by_an_oak(
        battlefield: &terrain::BattlefieldMap,
        cover: &terrain::StaticCoverObject,
    ) -> bool {
        battlefield.scenery.iter().any(|instance| {
            instance.kind == terrain::SceneryKind::Oak
                && (instance.position[0] - cover.center[0]).abs() < 0.05
                && (instance.position[2] - cover.center[2]).abs() < 0.05
        })
    }

    /// THE partial-rebake lock (urban-map program PR-04): collapse one building, re-bake ONLY
    /// the buckets its footprint touches, reassemble — and the result equals a full fresh bake
    /// bit for bit. This is what licenses the client to skip 16/17 of the bake on a phase
    /// change.
    #[test]
    fn replacing_only_the_dirty_buckets_equals_a_full_fresh_bake() {
        let battlefield = map_forge::battlefield(terrain::MapId::BystraValley);
        let intact = vec![0u8; battlefield.static_cover.len()];
        let mut buckets = battlefield_statics_buckets(&battlefield, &intact, &[]);

        let collapsed_index = battlefield
            .static_cover
            .iter()
            .position(|cover| cover.kind == StaticCoverKind::FarmBuilding)
            .expect("Bystra carries buildings");
        let mut states = intact.clone();
        states[collapsed_index] = 1;

        let dirty: Vec<usize> = statics_buckets_touched_by_cover(
            &battlefield,
            &battlefield.static_cover[collapsed_index],
        )
        .collect();
        assert!(!dirty.is_empty(), "a cover box must touch at least its own bucket");
        assert!(
            dirty.iter().all(|&bucket| bucket != STATICS_BACKDROP_BUCKET),
            "gameplay must never dirty the backdrop bucket"
        );
        for &bucket in &dirty {
            buckets[bucket] = battlefield_statics_bucket_mesh(&battlefield, &states, &[], bucket);
        }

        let partial = assemble_statics_mesh(&buckets);
        let full = battlefield_statics_mesh_with_scars(&battlefield, &states, &[]);
        assert_eq!(partial.0.len(), full.0.len(), "vertex counts must agree");
        assert_eq!(partial.1, full.1, "index streams must agree");
        assert!(
            partial
                .0
                .iter()
                .zip(&full.0)
                .all(|(a, b)| a.position == b.position && a.color == b.color),
            "vertex streams must agree"
        );
    }

    /// The bucket partition never loses an object: every cover box still contributes geometry
    /// to exactly the bucket its center owns, and the assembled mesh carries them all.
    #[test]
    fn every_bucket_object_survives_partitioning() {
        let battlefield = map_forge::battlefield(terrain::MapId::BystraValley);
        let states = vec![0u8; battlefield.static_cover.len()];
        let buckets = battlefield_statics_buckets(&battlefield, &states, &[]);
        assert_eq!(buckets.len(), STATICS_BUCKET_COUNT);
        let (vertices, _) = assemble_statics_mesh(&buckets);
        for cover in &battlefield.static_cover {
            // Trunk boxes bake nothing (their tree is instanced), so there is no bucket
            // geometry to survive — what must survive is the tree standing in them.
            if cover.kind == StaticCoverKind::TreeTrunk {
                assert!(dressed_by_an_oak(&battlefield, cover), "trunk {} kept", cover.id);
                continue;
            }
            let center = Vec3::from_array(cover.center);
            let half = Vec3::from_array(cover.half_extents_m);
            let rendered = vertices.iter().any(|vertex| {
                let delta = (Vec3::from_array(vertex.position) - center).abs();
                delta.x <= half.x + 1.0e-3
                    && delta.y <= half.y + 1.0e-3
                    && delta.z <= half.z + 1.0e-3
            });
            assert!(rendered, "cover {} must survive the bucket partition", cover.id);
        }
    }
}
