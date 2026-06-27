use glam::{Mat3, Vec3};
use renderer_api::SceneVertex;
use terrain::{BattlefieldMap, HeightMap, StaticCoverKind, StaticCoverObject};

use crate::tank_mesh::push_oriented_box;

/// Build the static battlefield mesh: the terrain plus every static cover object. Cover is
/// gameplay state (it blocks movement, shells, and the camera), so whatever the simulation
/// collides must be visible — rendering the exact sim boxes keeps the world honest.
pub fn battlefield_scene_mesh(battlefield: &BattlefieldMap) -> (Vec<SceneVertex>, Vec<u32>) {
    let (mut vertices, mut indices) = terrain_scene_mesh(&battlefield.heightmap);
    for cover in &battlefield.static_cover {
        append_cover_box(&mut vertices, &mut indices, cover);
    }
    (vertices, indices)
}

fn append_cover_box(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    cover: &StaticCoverObject,
) {
    push_oriented_box(
        vertices,
        indices,
        Vec3::from_array(cover.center),
        Vec3::from_array(cover.half_extents_m),
        Mat3::IDENTITY,
        cover_color(cover.kind),
    );
}

fn cover_color(kind: StaticCoverKind) -> [f32; 3] {
    match kind {
        StaticCoverKind::FarmBuilding => [0.45, 0.33, 0.24],
        StaticCoverKind::RailCover => [0.30, 0.24, 0.18],
        StaticCoverKind::TreeLine => [0.16, 0.30, 0.14],
        StaticCoverKind::Wreck => [0.25, 0.20, 0.17],
    }
}

/// Build a lit triangle mesh for the whole heightmap, colored by height and slope so
/// the terrain reads clearly: grass in the lowlands, rock on the heights and steeps.
pub fn terrain_scene_mesh(heightmap: &HeightMap) -> (Vec<SceneVertex>, Vec<u32>) {
    let w = heightmap.width();
    let h = heightmap.height();
    let cell = heightmap.cell_size_m();
    let stats = heightmap.stats();

    let mut vertices = Vec::with_capacity(w * h);
    for z in 0..h {
        for x in 0..w {
            let y = heightmap.sample_at_index(x, z);
            let normal = vertex_normal(heightmap, x, z, cell);
            let color = terrain_color(y, stats.min_m, stats.max_m, normal.y);
            vertices.push(SceneVertex::new(
                [x as f32 * cell, y, z as f32 * cell],
                normal.to_array(),
                color,
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
