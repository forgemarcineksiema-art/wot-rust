//! Procedural foliage meshes — trees 2.0 (B2): battlefield trees now come BAKED from
//! `world_forge::tree` (species as a parameter set, painterly crown normals), colored here and
//! folded into the static scene mesh, so a dressed valley still costs the frame nothing. The
//! old flat-shaded frusta remain as the FAR representation (the backdrop ring uses them
//! explicitly — at kilometers they read identically and cost almost nothing). Wind sway (D4)
//! arrives with the weather package as a shader effect.

use glam::{Mat3, Vec3};
use renderer_api::SceneVertex;
use terrain::{SceneryInstance, SceneryKind};

use crate::tank_mesh::push_oriented_box;

pub fn push_scenery_instance(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    instance: &SceneryInstance,
) {
    // Battlefield trees are the baked species; rocks keep their mineral box below.
    let species = match instance.kind {
        SceneryKind::Oak => Some(world_forge::tree::TreeSpecies::Oak),
        SceneryKind::Poplar => Some(world_forge::tree::TreeSpecies::Poplar),
        SceneryKind::Willow => Some(world_forge::tree::TreeSpecies::Willow),
        SceneryKind::FruitTree => Some(world_forge::tree::TreeSpecies::FruitTree),
        SceneryKind::Bush => Some(world_forge::tree::TreeSpecies::Bush),
        SceneryKind::Rock => None,
    };
    if let Some(species) = species {
        push_baked_tree(vertices, indices, instance, species);
        return;
    }
    push_scenery_instance_far(vertices, indices, instance);
}

/// The whole baked tree, transformed and colored into the static scene mesh. The seed comes
/// from the instance's position bits, so a shelterbelt never repeats a tree yet every scene
/// bake is identical.
fn push_baked_tree(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    instance: &SceneryInstance,
    species: world_forge::tree::TreeSpecies,
) {
    let base = Vec3::from_array(instance.position);
    let seed =
        instance.position[0].to_bits() as u64 ^ ((instance.position[2].to_bits() as u64) << 32);
    let tree = world_forge::tree::bake_tree_lod(species, seed, world_forge::tree::TreeLod::Mid);
    let rotation = Mat3::from_rotation_y(instance.yaw_rad);
    let scale = instance.scale;
    let canopy_color = canopy_color_for(species);
    for (mesh, (color, gloss), lit_by_sky) in
        [(&tree.trunk, TRUNK, false), (&tree.canopy, canopy_color, true)]
    {
        let start = vertices.len() as u32;
        for vertex in mesh.vertices() {
            let position = base + rotation * (vertex.position * scale);
            let normal = (rotation * vertex.normal).normalize_or_zero();
            // The painterly gradient: crown tops toward the light, undersides into shade.
            let shade = if lit_by_sky { 0.82 + 0.18 * normal.y.max(0.0) } else { 1.0 };
            vertices.push(SceneVertex::surfaced(
                position.to_array(),
                normal.to_array(),
                [color[0] * shade, color[1] * shade, color[2] * shade],
                gloss,
            ));
        }
        indices.extend(mesh.indices().iter().map(|index| index + start));
    }
}

fn canopy_color_for(species: world_forge::tree::TreeSpecies) -> ([f32; 3], f32) {
    match species {
        world_forge::tree::TreeSpecies::Oak => CANOPY_DARK,
        world_forge::tree::TreeSpecies::Poplar => CANOPY,
        world_forge::tree::TreeSpecies::Willow => CANOPY_PALE,
        world_forge::tree::TreeSpecies::FruitTree => CANOPY_PALE,
        world_forge::tree::TreeSpecies::Bush => CANOPY_DARK,
    }
}

/// The far representation: the original flat-shaded frusta. The backdrop ring calls this
/// directly — at its distances the baked crown and the painted cone are the same picture, and
/// the ring has thousands of instances.
pub fn push_scenery_instance_far(
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

    /// Per-kind triangle budget guard: a baked mid-LOD tree tops out at the world_forge LOD1
    /// budget; rocks stay a box.
    const MAX_TRIS_PER_INSTANCE: usize = world_forge::tree::TREE_LOD1_MAX_TRIS;

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

#[cfg(test)]
mod baked_tree_tests {
    use super::*;

    /// B2's on-screen contract: a battlefield tree is the BAKED species (hundreds of triangles,
    /// canopy normals bent from the crown centroid), deterministic per position — the scene
    /// bake is identical every time, yet no two shelterbelt oaks repeat.
    #[test]
    fn battlefield_trees_are_baked_species_deterministic_per_position() {
        let instance = |x: f32| SceneryInstance {
            kind: SceneryKind::Oak,
            position: [x, 0.0, 4.0],
            yaw_rad: 0.3,
            scale: 1.0,
        };
        let build = |instance: &SceneryInstance| {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            push_scenery_instance(&mut vertices, &mut indices, instance);
            (vertices, indices)
        };
        let (vertices_a, indices_a) = build(&instance(10.0));
        let (vertices_b, _) = build(&instance(10.0));
        assert_eq!(vertices_a.len(), vertices_b.len(), "the scene bake is deterministic");
        assert!(
            indices_a.len() / 3 > 60,
            "a baked oak is real geometry, not a frustum stack: {} tris",
            indices_a.len() / 3
        );
        let (vertices_c, _) = build(&instance(50.0));
        assert_ne!(
            vertices_a.iter().map(|v| u64::from(v.position[1].to_bits())).sum::<u64>(),
            vertices_c.iter().map(|v| u64::from(v.position[1].to_bits())).sum::<u64>(),
            "two oaks at different spots are different individuals"
        );
    }

    /// The backdrop ring keeps the cheap painted frusta — thousands of instances at kilometers.
    #[test]
    fn the_far_path_stays_a_cheap_frustum_stack() {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        push_scenery_instance_far(
            &mut vertices,
            &mut indices,
            &SceneryInstance {
                kind: SceneryKind::Oak,
                position: [0.0, 0.0, 0.0],
                yaw_rad: 0.0,
                scale: 1.0,
            },
        );
        assert!(indices.len() / 3 <= 60, "the far oak stays ~50 tris: {}", indices.len() / 3);
    }
}
