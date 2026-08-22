//! Procedural foliage meshes — trees 2.0 (B2): battlefield trees come BAKED from
//! `world_forge::tree` (species as a parameter set, painterly crown normals), colored here and
//! folded into the static scene mesh, so a dressed valley still costs the frame nothing. The
//! old flat-shaded frusta remain as the FAR representation (the backdrop ring uses them
//! explicitly — at kilometers they read identically and cost almost nothing).
//!
//! This module is the VEGETAL half of the scenery vocabulary. The scatter's dispatch and its
//! non-plant kinds (stone, street furniture, debris) live in `crate::clutter` — they moved out
//! with Skały 1.0, when a rock stopped being a cuboid and stopped being a leaf's business.

use glam::{Mat3, Vec3};
use renderer_api::SceneVertex;
use terrain::{SceneryInstance, SceneryKind};

/// Which species a scenery kind grows, if it grows one at all.
pub(crate) fn tree_species(kind: SceneryKind) -> Option<world_forge::tree::TreeSpecies> {
    match kind {
        SceneryKind::Oak => Some(world_forge::tree::TreeSpecies::Oak),
        SceneryKind::Poplar => Some(world_forge::tree::TreeSpecies::Poplar),
        SceneryKind::Willow => Some(world_forge::tree::TreeSpecies::Willow),
        SceneryKind::FruitTree => Some(world_forge::tree::TreeSpecies::FruitTree),
        SceneryKind::Bush => Some(world_forge::tree::TreeSpecies::Bush),
        SceneryKind::Pine => Some(world_forge::tree::TreeSpecies::Pine),
        SceneryKind::Rock
        | SceneryKind::Lamppost
        | SceneryKind::DebrisHeap
        | SceneryKind::FloraTree
        | SceneryKind::FloraPine
        | SceneryKind::FloraBush => None,
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
        _ => 1.0,
    }
}

/// The whole baked tree, transformed and colored into the static scene mesh. The seed comes
/// from the instance's position bits, so a shelterbelt never repeats a tree yet every scene
/// bake is identical. A non-tree kind draws nothing.
pub(crate) fn push_baked_tree(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    instance: &SceneryInstance,
) {
    let Some(species) = tree_species(instance.kind) else {
        return;
    };
    let base = Vec3::from_array(instance.position);
    let seed =
        instance.position[0].to_bits() as u64 ^ ((instance.position[2].to_bits() as u64) << 32);
    let tree = world_forge::tree::bake_tree_lod(species, seed, world_forge::tree::TreeLod::Mid);
    let rotation = Mat3::from_rotation_y(instance.yaw_rad);
    let scale = instance.scale;
    let canopy_color = canopy_color_for_species(species);
    for (mesh, (color, gloss), lit_by_sky) in
        [(&tree.trunk, TRUNK, false), (&tree.canopy, canopy_color, true)]
    {
        let start = vertices.len() as u32;
        for vertex in mesh.vertices() {
            let position = base + rotation * (vertex.position * scale);
            let normal = (rotation * vertex.normal).normalize_or_zero();
            // The painterly gradient: crown tops toward the light, undersides into shade.
            let shade = if lit_by_sky { 0.82 + 0.18 * normal.y.max(0.0) } else { 1.0 };
            // The trunk wears bark (Materia Świata 3); the canopy is FOLIAGE — the same role
            // the instanced oak carries in `tree_lod`, so a shelterbelt poplar and the
            // battlefield oak answer the sun with the same wrapped-diffuse + transmission
            // model. It rode LEGACY from before the role existed, which kept the whole foliage
            // lighting path dead on every statics-baked tree (Drzewa 3.0 PR1).
            let role = if lit_by_sky {
                renderer_api::surface_role::FOLIAGE
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
    // The card canopy of a migrated species (Drzewa 3.0 PR7): same expansion the instanced
    // ladder uses, placed by this instance's transform. Statics carry no sway — the chunked
    // buffer is the far representation, and wind is a near-rung luxury.
    push_leaf_cards(
        vertices,
        indices,
        &tree,
        canopy_color,
        |local| base + rotation * (local * scale),
        |direction| rotation * direction,
        |_| 0.0,
    );
}

/// Expand a baked tree's card deck into `SceneVertex` quads — ONE expansion for both paths
/// (the instanced ladder and the statics bake), so a card can never render differently by
/// route. Each card is 8 vertices / 4 triangles: dual winding with a normal ring per face,
/// or a card seen from behind lights only by the transmission lobe and reads as a black
/// cutout. `place` maps a tree-local position into the output space, `rotate` maps a
/// direction, `sway` answers per-corner wind allowance (the statics bake passes zero).
pub(crate) fn push_leaf_cards(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    tree: &world_forge::tree::BakedTree,
    (color, gloss): ([f32; 3], f32),
    place: impl Fn(Vec3) -> Vec3,
    rotate: impl Fn(Vec3) -> Vec3,
    sway: impl Fn(Vec3) -> f32,
) {
    for card in &tree.leaves {
        let rect = world_forge::tree::leaf_atlas::atlas_rect(card.slot);
        let start = vertices.len() as u32;
        // The cluster stem sits at -half_up (v1, the bottom of the slot).
        let corners = [
            (card.center - card.half_right - card.half_up, [rect[0], rect[3]]),
            (card.center + card.half_right - card.half_up, [rect[2], rect[3]]),
            (card.center + card.half_right + card.half_up, [rect[2], rect[1]]),
            (card.center - card.half_right + card.half_up, [rect[0], rect[1]]),
        ];
        for face_normal in [card.normal, -card.normal] {
            let normal = rotate(face_normal).normalize_or_zero();
            for (local, uv) in corners {
                vertices.push(
                    SceneVertex::surfaced(
                        place(local).to_array(),
                        normal.to_array(),
                        [color[0] * card.shade, color[1] * card.shade, color[2] * card.shade],
                        gloss,
                    )
                    .with_surface(renderer_api::surface_role::FOLIAGE)
                    .with_uv(uv)
                    .with_sway(sway(local)),
                );
            }
        }
        indices.extend_from_slice(&[
            start,
            start + 1,
            start + 2,
            start,
            start + 2,
            start + 3,
            // The far side, wound the other way, on its own normal ring.
            start + 4,
            start + 6,
            start + 5,
            start + 4,
            start + 7,
            start + 6,
        ]);
    }
}

/// The species canopy tone, shared by the statics bake and the instanced LOD ladder
/// (`tree_lod`) so the two paths agree on the tree's colour.
pub(crate) fn canopy_color_for_species(species: world_forge::tree::TreeSpecies) -> ([f32; 3], f32) {
    match species {
        world_forge::tree::TreeSpecies::Oak => CANOPY_DARK,
        world_forge::tree::TreeSpecies::Poplar => CANOPY,
        world_forge::tree::TreeSpecies::Willow => CANOPY_PALE,
        world_forge::tree::TreeSpecies::FruitTree => CANOPY_PALE,
        world_forge::tree::TreeSpecies::Bush => CANOPY_DARK,
        world_forge::tree::TreeSpecies::Pine => CANOPY_PINE,
    }
}

/// The far representation of a PLANT: the original flat-shaded frusta. The backdrop ring reaches
/// this through `crate::clutter::push_scenery_instance_far` — at its distances the baked crown
/// and the painted cone are the same picture, and the ring has thousands of instances.
pub(crate) fn push_scenery_tree_far(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    instance: &SceneryInstance,
) {
    let base = Vec3::from_array(instance.position);
    // Trees-to-scale: the backdrop silhouette rises to the same mature height the near mesh now
    // has (furniture stays at 1.0). See `far_frustum_scale`.
    let s = instance.scale * far_frustum_scale(instance.kind);
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
        // Not a plant: `crate::clutter` owns these, and the retired imported kinds draw nothing
        // anywhere. This arm only exists so the match is total.
        SceneryKind::Rock
        | SceneryKind::Lamppost
        | SceneryKind::DebrisHeap
        | SceneryKind::FloraTree
        | SceneryKind::FloraPine
        | SceneryKind::FloraBush => {}
    }
}

// Each material is (color, gloss): bark is matte, leaf canopies carry the faint waxy sheen
// that answers a wet sky without ever reading as plastic.
pub(crate) const TRUNK_TONE: ([f32; 3], f32) = ([0.30, 0.22, 0.14], 0.04);
#[allow(clippy::upper_case_acronyms)]
const TRUNK: ([f32; 3], f32) = TRUNK_TONE;
const CANOPY: ([f32; 3], f32) = ([0.18, 0.34, 0.15], 0.07);
const CANOPY_DARK: ([f32; 3], f32) = ([0.13, 0.27, 0.12], 0.06);
const CANOPY_PALE: ([f32; 3], f32) = ([0.24, 0.38, 0.19], 0.08);
const CANOPY_PINE: ([f32; 3], f32) = ([0.10, 0.22, 0.13], 0.05);

/// A flat-shaded n-gon frustum standing on `base`: `r0` at the bottom, `r1` at the top,
/// closed with a top fan. Six segments keep a tree ~50 tris.
pub(crate) fn push_frustum(
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
mod baked_tree_tests {
    use crate::clutter::{StoneTone, push_scenery_instance, push_scenery_instance_far};

    use super::*;

    /// B2's on-screen contract for the species that still bake into the statics (the oak is
    /// instanced): a battlefield tree is the BAKED species (real geometry, canopy normals bent
    /// from the crown centroid), deterministic per position — the scene bake is identical every
    /// time, yet no two shelterbelt trees repeat.
    #[test]
    fn battlefield_trees_are_baked_species_deterministic_per_position() {
        let instance = |x: f32| SceneryInstance {
            kind: SceneryKind::Poplar,
            position: [x, 0.0, 4.0],
            yaw_rad: 0.3,
            scale: 1.0,
        };
        let build = |instance: &SceneryInstance| {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            push_scenery_instance(&mut vertices, &mut indices, instance, StoneTone::NEUTRAL);
            (vertices, indices)
        };
        let (vertices_a, indices_a) = build(&instance(10.0));
        let (vertices_b, _) = build(&instance(10.0));
        assert_eq!(vertices_a.len(), vertices_b.len(), "the scene bake is deterministic");
        assert!(
            indices_a.len() / 3 > 60,
            "a baked poplar is real geometry, not a frustum stack: {} tris",
            indices_a.len() / 3
        );
        let (vertices_c, _) = build(&instance(50.0));
        assert_ne!(
            vertices_a.iter().map(|v| u64::from(v.position[1].to_bits())).sum::<u64>(),
            vertices_c.iter().map(|v| u64::from(v.position[1].to_bits())).sum::<u64>(),
            "two poplars at different spots are different individuals"
        );
        // Materia Świata 3 + Drzewa 3.0 PR1: the trunk wears bark down the surface lane, the
        // canopy wears FOLIAGE — the statics bake and the instanced ladder must answer the sun
        // with the same lighting model, and nothing may slide back to LEGACY.
        let barked = vertices_a
            .iter()
            .filter(|v| (v.surface - renderer_api::surface_role::BARK).abs() < 0.01)
            .count();
        let leafed = vertices_a
            .iter()
            .filter(|v| (v.surface - renderer_api::surface_role::FOLIAGE).abs() < 0.01)
            .count();
        assert!(barked > 0, "the trunk names its bark");
        assert!(leafed > 0, "the canopy names its foliage");
        assert_eq!(
            barked + leafed,
            vertices_a.len(),
            "a baked tree is bark and foliage, nothing rides LEGACY"
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
            StoneTone::NEUTRAL,
        );
        assert!(indices.len() / 3 <= 60, "the far oak stays ~50 tris: {}", indices.len() / 3);
    }

    /// Świat 2.0 PR1: the backdrop silhouette must not undercut the mature near height — a
    /// distant treeline that reads as a hedge undoes the trees-to-scale pass.
    #[test]
    fn far_frustum_oaks_and_pines_reach_mature_height() {
        let tip = |kind: SceneryKind| {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            push_scenery_instance_far(
                &mut vertices,
                &mut indices,
                &SceneryInstance { kind, position: [0.0, 0.0, 0.0], yaw_rad: 0.0, scale: 1.0 },
                StoneTone::NEUTRAL,
            );
            vertices.iter().map(|v| v.position[1]).fold(f32::NEG_INFINITY, f32::max)
        };
        assert!(tip(SceneryKind::Oak) > 15.0, "far oak: {}", tip(SceneryKind::Oak));
        assert!(tip(SceneryKind::Pine) > 18.0, "far pine: {}", tip(SceneryKind::Pine));
        assert!(tip(SceneryKind::Poplar) > 19.0, "far poplar: {}", tip(SceneryKind::Poplar));
    }

    /// Per-kind triangle budgets, and the two promises that bound the whole scatter: retired
    /// kinds draw nothing, live kinds draw something, and the debris heap stays knee-high.
    ///
    /// Budgets are PER KIND now. They used to be one borrowed constant — the tree's LOD1 ceiling
    /// standing in for every kind in the vocabulary, which is a ratchet wearing a budget's name
    /// (a rock has nothing to do with a mid-LOD tree). Each number below is what that kind's own
    /// construction costs, plus a little headroom.
    #[test]
    fn every_kind_stays_inside_its_own_triangle_budget_with_sane_indices() {
        for retired in [SceneryKind::FloraPine, SceneryKind::FloraBush, SceneryKind::FloraTree] {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            push_scenery_instance(
                &mut vertices,
                &mut indices,
                &SceneryInstance {
                    kind: retired,
                    position: [10.0, 3.0, 10.0],
                    yaw_rad: 0.7,
                    scale: 1.3,
                },
                StoneTone::NEUTRAL,
            );
            assert!(indices.is_empty(), "{retired:?} contributes nothing to the statics bake");
        }
        let tree_ceiling = world_forge::tree::TREE_LOD1_MAX_TRIS;
        // A migrated species' statics instance is Mid bark PLUS its thinned card deck at
        // 4 tris a card — raised DELIBERATELY with the wave (Drzewa 3.0 PR7, measured:
        // poplar 548). The fill verdict stays the flora_frame_probe's; this catches silent
        // geometric growth. Legacy species hold the old lobed ceiling until their wave.
        const MIGRATED_STATICS_MAX_TRIS: usize = 700;
        for (kind, ceiling) in [
            (SceneryKind::Poplar, MIGRATED_STATICS_MAX_TRIS),
            (SceneryKind::Willow, tree_ceiling),
            (SceneryKind::FruitTree, MIGRATED_STATICS_MAX_TRIS),
            (SceneryKind::Bush, tree_ceiling),
            (SceneryKind::Pine, tree_ceiling),
            // The forged field stone: 80 triangles of displaced body plus its two frost chips.
            (SceneryKind::Rock, 108),
            // The masonry spill, raised from the old 60 with its construction: a 7-sided mass,
            // three tumbled wall pieces, two roof shards and a snapped joist. It buys the thing
            // the grey frustum kit could not — the pile reads as a piece of a building.
            (SceneryKind::DebrisHeap, world_forge::spill::SPILL_MAX_TRIS),
            // Street furniture, unchanged frustum kit.
            (SceneryKind::Lamppost, 60),
        ] {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            push_scenery_instance(
                &mut vertices,
                &mut indices,
                &SceneryInstance { kind, position: [10.0, 3.0, 10.0], yaw_rad: 0.7, scale: 1.3 },
                StoneTone::NEUTRAL,
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
                "{kind:?} broke its per-instance triangle budget: {}",
                indices.len() / 3
            );
            assert!(indices.iter().all(|&index| (index as usize) < vertices.len()));
            // Nothing floats far below its ground point (rocks legitimately embed a little).
            assert!(vertices.iter().all(|vertex| vertex.position[1] >= 3.0 - 1.0));
        }
    }
}
