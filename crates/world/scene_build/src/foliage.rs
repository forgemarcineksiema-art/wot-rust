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
        SceneryKind::Pine => Some(world_forge::tree::TreeSpecies::Pine),
        SceneryKind::Rock | SceneryKind::Lamppost | SceneryKind::DebrisHeap => None,
        SceneryKind::FloraTree | SceneryKind::FloraPine | SceneryKind::FloraBush => None,
    };
    if let Some(name) = imported_flora_name(instance.kind) {
        push_imported_flora(vertices, indices, instance, name);
        return;
    }
    if let Some(species) = species {
        push_baked_tree(vertices, indices, instance, species);
        return;
    }
    push_scenery_instance_far(vertices, indices, instance);
}

/// Which shipped `assets/flora` asset a scenery kind names, if any.
fn imported_flora_name(kind: SceneryKind) -> Option<&'static str> {
    match kind {
        SceneryKind::FloraTree => Some("stylized-tree"),
        SceneryKind::FloraPine => Some("stylized-pine"),
        SceneryKind::FloraBush => Some("stylized-bush"),
        _ => None,
    }
}

/// The trees-to-scale multiplier for an IMPORTED flora MESH (trees-to-scale, 2026-08-03). The
/// CC0 assets were authored small — `stylized-tree` bakes at ~7.3 m, `stylized-pine` ~7.4 m,
/// ~30% of a real mature tree — so on top of the per-instance scatter scale they get a species
/// factor that brings a broadleaf to ~20 m and a pine to ~21 m. A tank beside one now reads at
/// a tank's true fraction of a tree, not half a tree. The bush is already the right knee-height.
fn imported_flora_scale(kind: SceneryKind) -> f32 {
    match kind {
        SceneryKind::FloraTree => 2.75,
        SceneryKind::FloraPine => 2.85,
        _ => 1.0,
    }
}

/// The trees-to-scale multiplier for a BACKDROP-ring frustum stack. The far stack is coarser and
/// intrinsically shorter than the mesh it stands in for, so the factor is per-kind (larger than
/// the mesh factor) to bring the horizon silhouette up to the same mature height the near mesh
/// now has — a distant treeline that reads as a treeline, not a hedge. Furniture (lamppost,
/// rock, debris, fruit, bush) is already correct and stays at 1.0.
fn far_frustum_scale(kind: SceneryKind) -> f32 {
    match kind {
        SceneryKind::Oak => 3.4,
        SceneryKind::Poplar => 3.0,
        SceneryKind::Willow => 3.6,
        SceneryKind::Pine => 2.7,
        SceneryKind::FloraTree => 3.3,
        SceneryKind::FloraPine => 2.8,
        _ => 1.0,
    }
}

/// An imported CC0 asset, transformed into the static scene mesh with its UVs remapped into
/// the packed atlas region (Flora 2.0, FL-4). Vertex color carries the baked material tint
/// (base_color_factor from the source) and the FL-2 fragment path multiplies the sampled
/// texel in — the product reconstructs the authored look; gloss keeps the leaf-wax sheen.
/// No procedural surface treatment: the texture carries the detail.
fn push_imported_flora(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    instance: &SceneryInstance,
    name: &str,
) {
    let Some((asset, region)) = crate::flora_pack::flora_catalog().get(name) else {
        // A kind naming a missing asset is a build error caught by the catalog tests; at
        // runtime we draw nothing rather than lie with a placeholder.
        return;
    };
    let base = Vec3::from_array(instance.position);
    let rotation = Mat3::from_rotation_y(instance.yaw_rad);
    let scale = instance.scale * imported_flora_scale(instance.kind);
    let start = vertices.len() as u32;
    for (index, position) in asset.positions.iter().enumerate() {
        let world = base + rotation * (Vec3::from_array(*position) * scale);
        let normal = (rotation * Vec3::from_array(asset.normals[index])).normalize_or_zero();
        let uv = asset.uvs[index];
        vertices.push(
            SceneVertex::surfaced(world.to_array(), normal.to_array(), asset.colors[index], 0.07)
                .with_uv([
                    region.u_offset + uv[0] * region.u_scale,
                    region.v_offset + uv[1] * region.v_scale,
                ]),
        );
    }
    indices.extend(asset.indices.iter().map(|index| index + start));
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
            // The trunk wears bark (Materia Świata 3); the canopy keeps its painterly fill.
            let role = if lit_by_sky {
                renderer_api::surface_role::LEGACY
            } else {
                renderer_api::surface_role::BARK
            };
            vertices.push(
                SceneVertex::surfaced(
                    position.to_array(),
                    normal.to_array(),
                    [color[0] * shade, color[1] * shade, color[2] * shade],
                    gloss,
                )
                .with_surface(role),
            );
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
        world_forge::tree::TreeSpecies::Pine => CANOPY_PINE,
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
    // Trees-to-scale: the backdrop silhouette rises to the same mature height the near mesh now
    // has (furniture stays at 1.0). See `far_frustum_scale`.
    let s = instance.scale * far_frustum_scale(instance.kind);
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
        SceneryKind::Pine => {
            // The conifer cone: a bare trunk under two stacked needle frusta tapering to a
            // near-point tip — unmistakably not a broadleaf, even at backdrop range.
            push_frustum(vertices, indices, base, 0.24 * s, 0.16 * s, 2.3 * s, TRUNK);
            let skirt = base + Vec3::Y * 2.0 * s;
            push_frustum(vertices, indices, skirt, 1.9 * s, 1.0 * s, 2.6 * s, CANOPY_PINE);
            let tip = skirt + Vec3::Y * 2.6 * s;
            push_frustum(vertices, indices, tip, 1.1 * s, 0.04 * s, 2.9 * s, CANOPY_PINE);
        }
        SceneryKind::Lamppost => {
            // Cast iron (urban-map PR-11): the tapering column, the short arm reaching over
            // the roadway along the instance yaw, and the warm glass head under it.
            push_frustum(vertices, indices, base, 0.09 * s, 0.055 * s, 4.0 * s, IRON);
            let reach = Vec3::new(yaw.sin(), 0.0, yaw.cos());
            let arm_root = base + Vec3::Y * 4.0 * s;
            let start = vertices.len();
            push_oriented_box(
                vertices,
                indices,
                arm_root + reach * 0.35 * s + Vec3::Y * 0.06 * s,
                Vec3::new(0.045 * s, 0.045 * s, 0.42 * s),
                Mat3::from_rotation_y(yaw),
                IRON.0,
            );
            push_oriented_box(
                vertices,
                indices,
                arm_root + reach * 0.72 * s - Vec3::Y * 0.12 * s,
                Vec3::new(0.11 * s, 0.16 * s, 0.11 * s),
                Mat3::from_rotation_y(yaw),
                LAMP_GLASS.0,
            );
            for vertex in &mut vertices[start..] {
                vertex.gloss = if vertex.color == LAMP_GLASS.0 { LAMP_GLASS.1 } else { IRON.1 };
            }
        }
        SceneryKind::DebrisHeap => {
            // A knee-high masonry spill (urban-map PR-11): a low mound with two tumbled
            // slabs. Deliberately under bush height — dressing, never implied cover.
            push_frustum(vertices, indices, base, 0.95 * s, 0.3 * s, 0.42 * s, RUBBLE);
            let start = vertices.len();
            push_oriented_box(
                vertices,
                indices,
                base + Vec3::new(0.45 * s, 0.14 * s, -0.2 * s),
                Vec3::new(0.3 * s, 0.12 * s, 0.2 * s),
                Mat3::from_rotation_y(yaw + 0.5),
                RUBBLE.0,
            );
            push_oriented_box(
                vertices,
                indices,
                base + Vec3::new(-0.4 * s, 0.1 * s, 0.3 * s),
                Vec3::new(0.24 * s, 0.09 * s, 0.17 * s),
                Mat3::from_rotation_y(yaw - 0.8),
                RUBBLE.0,
            );
            for vertex in &mut vertices[start..] {
                vertex.gloss = RUBBLE.1;
            }
        }
        // Imported kinds at BACKDROP range: the painted frusta ARE the far representation
        // (FL-4 LOD contract) — at kilometres a textured canopy and a cone read identically,
        // and the ring carries thousands of instances.
        SceneryKind::FloraTree => {
            push_frustum(vertices, indices, base, 0.26 * s, 0.18 * s, 2.4 * s, TRUNK);
            let crown = base + Vec3::Y * 2.1 * s;
            push_frustum(vertices, indices, crown, 2.2 * s, 1.2 * s, 2.4 * s, CANOPY_DARK);
            let top = crown + Vec3::Y * 2.4 * s;
            push_frustum(vertices, indices, top, 1.2 * s, 0.3 * s, 1.6 * s, CANOPY);
        }
        SceneryKind::FloraPine => {
            push_frustum(vertices, indices, base, 0.24 * s, 0.16 * s, 2.3 * s, TRUNK);
            let skirt = base + Vec3::Y * 2.0 * s;
            push_frustum(vertices, indices, skirt, 1.8 * s, 0.9 * s, 2.7 * s, CANOPY_PINE);
            let tip = skirt + Vec3::Y * 2.7 * s;
            push_frustum(vertices, indices, tip, 1.0 * s, 0.05 * s, 2.6 * s, CANOPY_PINE);
        }
        SceneryKind::FloraBush => {
            push_frustum(vertices, indices, base, 1.1 * s, 0.8 * s, 0.6 * s, CANOPY_DARK);
            let top = base + Vec3::Y * 0.55 * s;
            push_frustum(vertices, indices, top, 0.75 * s, 0.2 * s, 0.55 * s, CANOPY);
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
const CANOPY_PINE: ([f32; 3], f32) = ([0.10, 0.22, 0.13], 0.05);
/// Cast iron and its warm glass head (urban-map PR-11); masonry spill for the debris heap.
const IRON: ([f32; 3], f32) = ([0.16, 0.17, 0.18], 0.25);
const LAMP_GLASS: ([f32; 3], f32) = ([0.75, 0.68, 0.45], 0.5);
const RUBBLE: ([f32; 3], f32) = ([0.44, 0.40, 0.35], 0.07);

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
        // Imported kinds (Flora 2.0) carry their own DELIBERATE ceiling: the whole program
        // exists to spend more triangles on close-range foliage, and the import gate already
        // enforces it per asset — the raise is per-kind, never a fleet-wide envelope bump.
        for (kind, ceiling) in [
            (SceneryKind::Oak, MAX_TRIS_PER_INSTANCE),
            (SceneryKind::Poplar, MAX_TRIS_PER_INSTANCE),
            (SceneryKind::Willow, MAX_TRIS_PER_INSTANCE),
            (SceneryKind::FruitTree, MAX_TRIS_PER_INSTANCE),
            (SceneryKind::Rock, MAX_TRIS_PER_INSTANCE),
            (SceneryKind::Bush, MAX_TRIS_PER_INSTANCE),
            (SceneryKind::Pine, MAX_TRIS_PER_INSTANCE),
            (SceneryKind::Lamppost, MAX_TRIS_PER_INSTANCE),
            (SceneryKind::DebrisHeap, MAX_TRIS_PER_INSTANCE),
            (SceneryKind::FloraTree, world_forge::flora::FLORA_MAX_TRIS),
            (SceneryKind::FloraPine, world_forge::flora::FLORA_MAX_TRIS),
            (SceneryKind::FloraBush, world_forge::flora::FLORA_MAX_TRIS),
        ] {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            push_scenery_instance(
                &mut vertices,
                &mut indices,
                &SceneryInstance { kind, position: [10.0, 3.0, 10.0], yaw_rad: 0.7, scale: 1.3 },
            );
            assert!(!indices.is_empty(), "{kind:?} must draw something");
            assert!(indices.len().is_multiple_of(3));
            if kind == SceneryKind::DebrisHeap {
                let top = vertices.iter().map(|v| v.position[1] - 3.0).fold(f32::MIN, f32::max);
                assert!(
                    top <= 0.7 * 1.3,
                    "a debris heap stays knee-high (honest-blockers rule), got {top}"
                );
            }
            assert!(
                indices.len() / 3 <= ceiling,
                "{kind:?} broke the per-instance triangle budget: {}",
                indices.len() / 3
            );
            assert!(indices.iter().all(|&index| (index as usize) < vertices.len()));
            // Nothing floats far below its ground point (rocks legitimately embed a little).
            assert!(vertices.iter().all(|vertex| vertex.position[1] >= 3.0 - 1.0));
        }
    }

    /// The imported kinds carry REMAPPED atlas UVs (never the whole page, never outside it)
    /// on white vertices — the texture is the palette, exactly the FL-2 contract.
    #[test]
    fn imported_kinds_carry_remapped_atlas_uvs() {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        push_scenery_instance(
            &mut vertices,
            &mut indices,
            &SceneryInstance {
                kind: SceneryKind::FloraPine,
                position: [0.0, 0.0, 0.0],
                yaw_rad: 0.0,
                scale: 1.0,
            },
        );
        assert!(!vertices.is_empty());
        let (region_area, page_area) = {
            let (_, region) =
                crate::flora_pack::flora_catalog().get("stylized-pine").expect("shipped");
            (region.u_scale * region.v_scale, 1.0)
        };
        assert!(region_area < page_area, "the pine owns a region, not the page");
        assert!(
            vertices.iter().all(|v| {
                (0.0..=1.0).contains(&v.uv[0])
                    && (0.0..=1.0).contains(&v.uv[1])
                    && v.color.iter().all(|channel| (0.0..=1.0).contains(channel))
            }),
            "in-page UVs with bounded material tints - factor x texel is the palette"
        );
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
        // Materia Świata 3: the trunk wears bark down the surface lane, the canopy does not.
        let barked = vertices_a
            .iter()
            .filter(|v| (v.surface - renderer_api::surface_role::BARK).abs() < 0.01)
            .count();
        assert!(barked > 0, "the trunk names its bark");
        assert!(barked < vertices_a.len(), "the canopy keeps its painterly fill");
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
