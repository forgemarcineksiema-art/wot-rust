//! Shared map-authoring builders: every battlefield generator grounds its spawns, strategic
//! points, cover and features on the heightmap the same way, so the grounding lives once here
//! and each map module keeps only its actual design (positions, sizes, names).

use crate::{
    HeightMap, MapFeature, MapFeatureKind, SpawnZone, StaticCoverKind, StaticCoverObject,
    StrategicPoint, StrategicRole,
};

/// Build a square heightmap by sampling an analytic height function on the cell grid.
pub fn heightmap_from_fn(
    samples_per_side: usize,
    cell_size_m: f32,
    height_at: impl Fn(f32, f32) -> f32,
) -> HeightMap {
    let mut samples = Vec::with_capacity(samples_per_side * samples_per_side);
    for z in 0..samples_per_side {
        for x in 0..samples_per_side {
            samples.push(height_at(x as f32 * cell_size_m, z as f32 * cell_size_m));
        }
    }
    HeightMap::new(samples_per_side, samples_per_side, cell_size_m, samples)
        .expect("generated heightmap dimensions are fixed")
}

/// A world position snapped to the terrain surface.
pub fn ground_position(heightmap: &HeightMap, x: f32, z: f32) -> [f32; 3] {
    [x, heightmap.sample_height(x, z).unwrap_or(0.0), z]
}

pub fn grounded_spawn_zone(
    heightmap: &HeightMap,
    team: u16,
    x: f32,
    z: f32,
    facing_yaw_rad: f32,
) -> SpawnZone {
    SpawnZone { team, center: ground_position(heightmap, x, z), radius_m: 55.0, facing_yaw_rad }
}

pub fn grounded_point(
    heightmap: &HeightMap,
    id: &str,
    name: &str,
    role: StrategicRole,
    x: f32,
    z: f32,
    radius_m: f32,
) -> StrategicPoint {
    StrategicPoint {
        id: id.to_string(),
        name: name.to_string(),
        role,
        position: ground_position(heightmap, x, z),
        radius_m,
    }
}

/// A cover box seated on the terrain: `center.y = ground + half_height`.
pub fn grounded_cover(
    heightmap: &HeightMap,
    id: &str,
    name: &str,
    kind: StaticCoverKind,
    xz: [f32; 2],
    half_extents_m: [f32; 3],
) -> StaticCoverObject {
    let ground_y = heightmap.sample_height(xz[0], xz[1]).unwrap_or(0.0);
    StaticCoverObject {
        id: id.to_string(),
        name: name.to_string(),
        kind,
        center: [xz[0], ground_y + half_extents_m[1], xz[1]],
        half_extents_m,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn grounded_feature(
    heightmap: &HeightMap,
    kind: MapFeatureKind,
    name: &str,
    x: f32,
    z: f32,
    radius_m: f32,
    note: &str,
) -> MapFeature {
    MapFeature {
        kind,
        name: name.to_string(),
        center: ground_position(heightmap, x, z),
        radius_m,
        note: note.to_string(),
    }
}
