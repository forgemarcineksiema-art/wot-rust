//! Procedural foliage meshes — low-poly trees and rocks in the vehicle-forge spirit: no
//! external assets, flat-shaded frusta with honest normals, colors doing the talking. Baked
//! into the static scene mesh (uploaded once per scene swap), so a dressed valley costs the
//! frame nothing. Wind sway arrives with the weather package as a shader effect.

use glam::{Mat3, Vec3};
use renderer_api::SceneVertex;
use terrain::{SceneryInstance, SceneryKind};

use crate::tank_mesh::push_oriented_box;

pub fn push_scenery_instance(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    instance: &SceneryInstance,
) {
    let base = Vec3::from_array(instance.position);
    let s = instance.scale;
    let yaw = instance.yaw_rad;
    match instance.kind {
        SceneryKind::Oak => {
            push_frustum(vertices, indices, base, 0.26 * s, 0.18 * s, 2.2 * s, TRUNK);
            let crown = base + Vec3::Y * 1.9 * s;
            push_frustum(vertices, indices, crown, 2.3 * s, 1.5 * s, 1.7 * s, CANOPY_DARK);
            let top = crown + Vec3::Y * 1.7 * s;
            push_frustum(vertices, indices, top, 1.5 * s, 0.35 * s, 1.5 * s, CANOPY);
        }
        SceneryKind::Poplar => {
            push_frustum(vertices, indices, base, 0.18 * s, 0.13 * s, 1.4 * s, TRUNK);
            let crown = base + Vec3::Y * 1.2 * s;
            push_frustum(vertices, indices, crown, 0.95 * s, 0.12 * s, 6.2 * s, CANOPY_DARK);
        }
        SceneryKind::Willow => {
            push_frustum(vertices, indices, base, 0.30 * s, 0.20 * s, 1.7 * s, TRUNK);
            // The drooping skirt: wider at its LOWER rim than the crown above it.
            let skirt = base + Vec3::Y * 1.1 * s;
            push_frustum(vertices, indices, skirt, 2.9 * s, 2.3 * s, 1.1 * s, CANOPY_PALE);
            let crown = skirt + Vec3::Y * 1.1 * s;
            push_frustum(vertices, indices, crown, 2.3 * s, 0.8 * s, 1.3 * s, CANOPY);
        }
        SceneryKind::FruitTree => {
            push_frustum(vertices, indices, base, 0.15 * s, 0.11 * s, 1.1 * s, TRUNK);
            let crown = base + Vec3::Y * 0.9 * s;
            push_frustum(vertices, indices, crown, 1.25 * s, 0.4 * s, 1.5 * s, CANOPY_PALE);
        }
        SceneryKind::Bush => {
            // A squat leafy mound, no trunk: knee-high, so it dresses the steppe without
            // ever *looking* like the concealment it honestly is not.
            push_frustum(vertices, indices, base, 1.15 * s, 0.85 * s, 0.55 * s, CANOPY_DARK);
            let top = base + Vec3::Y * 0.5 * s;
            push_frustum(vertices, indices, top, 0.8 * s, 0.22 * s, 0.5 * s, CANOPY);
        }
        SceneryKind::Rock => {
            // Bare mineral faces catch the sky harder than anything vegetal around them.
            let start = vertices.len();
            push_oriented_box(
                vertices,
                indices,
                base + Vec3::Y * 0.45 * s,
                Vec3::new(0.9, 0.5, 0.7) * s,
                Mat3::from_rotation_y(yaw),
                [0.42, 0.40, 0.37],
            );
            for vertex in &mut vertices[start..] {
                vertex.gloss = 0.18;
            }
        }
    }
}

// Each material is (color, gloss): bark is matte, leaf canopies carry the faint waxy sheen
// that answers a wet sky without ever reading as plastic.
const TRUNK: ([f32; 3], f32) = ([0.30, 0.22, 0.14], 0.04);
const CANOPY: ([f32; 3], f32) = ([0.18, 0.34, 0.15], 0.07);
const CANOPY_DARK: ([f32; 3], f32) = ([0.13, 0.27, 0.12], 0.06);
const CANOPY_PALE: ([f32; 3], f32) = ([0.24, 0.38, 0.19], 0.08);

/// A flat-shaded n-gon frustum standing on `base`: `r0` at the bottom, `r1` at the top,
/// closed with a top fan. Six segments keep a tree ~50 tris.
fn push_frustum(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    base: Vec3,
    r0: f32,
    r1: f32,
    height: f32,
    (color, gloss): ([f32; 3], f32),
) {
    const SEGMENTS: usize = 6;
    let top_center = base + Vec3::Y * height;
    for segment in 0..SEGMENTS {
        let a0 = segment as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let a1 = (segment + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let (d0, d1) = (Vec3::new(a0.cos(), 0.0, a0.sin()), Vec3::new(a1.cos(), 0.0, a1.sin()));
        let b0 = base + d0 * r0;
        let b1 = base + d1 * r0;
        let t0 = top_center + d0 * r1;
        let t1 = top_center + d1 * r1;
        // Flat side normal: the outward mid direction tilted by the slope (r0 -> r1 over h).
        let mid = (d0 + d1).normalize_or_zero();
        let normal = (mid * height + Vec3::Y * (r0 - r1)).normalize_or_zero().to_array();
        let start = vertices.len() as u32;
        for point in [b0, b1, t1, t0] {
            vertices.push(SceneVertex::surfaced(point.to_array(), normal, color, gloss));
        }
        indices.extend_from_slice(&[start, start + 2, start + 1, start, start + 3, start + 2]);
        // Top cap wedge (skipped for near-point tips).
        if r1 > 0.05 {
            let up = [0.0, 1.0, 0.0];
            let cap = vertices.len() as u32;
            vertices.push(SceneVertex::surfaced(top_center.to_array(), up, color, gloss));
            vertices.push(SceneVertex::surfaced(t0.to_array(), up, color, gloss));
            vertices.push(SceneVertex::surfaced(t1.to_array(), up, color, gloss));
            indices.extend_from_slice(&[cap, cap + 2, cap + 1]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Per-kind triangle budget guard.
    const MAX_TRIS_PER_INSTANCE: usize = 160;

    #[test]
    fn every_kind_stays_inside_the_triangle_budget_with_sane_indices() {
        for kind in [
            SceneryKind::Oak,
            SceneryKind::Poplar,
            SceneryKind::Willow,
            SceneryKind::FruitTree,
            SceneryKind::Rock,
            SceneryKind::Bush,
        ] {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            push_scenery_instance(
                &mut vertices,
                &mut indices,
                &SceneryInstance { kind, position: [10.0, 3.0, 10.0], yaw_rad: 0.7, scale: 1.3 },
            );
            assert!(!indices.is_empty());
            assert!(indices.len().is_multiple_of(3));
            assert!(
                indices.len() / 3 <= MAX_TRIS_PER_INSTANCE,
                "{kind:?} broke the per-instance triangle budget: {}",
                indices.len() / 3
            );
            assert!(indices.iter().all(|&index| (index as usize) < vertices.len()));
            // Nothing floats far below its ground point (rocks legitimately embed a little).
            assert!(vertices.iter().all(|vertex| vertex.position[1] >= 3.0 - 1.0));
        }
    }
}
