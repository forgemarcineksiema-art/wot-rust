//! Procedural foliage meshes — trees 2.0 (B2): battlefield trees come BAKED from
//! `world_forge::tree` (species as a parameter set, painterly crown normals), colored here and
//! folded into the static scene mesh, so a dressed valley still costs the frame nothing. The
//! FAR representation of a tree is its impostor — two crossed quads over the species' sprite
//! pair in the foliage atlas — and it is ONE expansion here for both routes: the instanced
//! ladder's far rung (`tree_lod`) and the backdrop ring baked into the statics
//! (`backdrop`). The flat-shaded frustum kit that used to stand in for the ring is gone
//! (Inny Poziom F1): at 40 m past the red line a hexagonal cone was never "the same picture".
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

/// The seed a statics-baked tree grows from: the instance's position bits, so a scatter never
/// repeats a tree yet every scene bake is identical. One function, because the planted tree
/// line (`tree_line`) measures a tree for its fit BEFORE the bake draws it, and the two must
/// grow the same tree.
pub(crate) fn statics_tree_seed(position: [f32; 3]) -> u64 {
    position[0].to_bits() as u64 ^ ((position[2].to_bits() as u64) << 32)
}

/// The whole baked tree, transformed and colored into the static scene mesh. The seed comes
/// from the instance's position bits (`statics_tree_seed`). A non-tree kind draws nothing.
pub(crate) fn push_baked_tree(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    instance: &SceneryInstance,
) {
    let Some(species) = tree_species(instance.kind) else {
        return;
    };
    let base = Vec3::from_array(instance.position);
    let seed = statics_tree_seed(instance.position);
    // The bush bakes at CLOSE: its whole body is card mass, and the Mid thinning that a tall
    // tree hides inside its silhouette makes a knee-high tuft nearly vanish at 200 m — the
    // steppe's dark value plane (rule 1) rode on exactly those tufts. A bush is a tenth of a
    // tree's triangles; full price is the honest price.
    let lod = if species == world_forge::tree::TreeSpecies::Bush {
        world_forge::tree::TreeLod::Close
    } else {
        world_forge::tree::TreeLod::Mid
    };
    let tree = world_forge::tree::bake_tree_lod(species, seed, lod);
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
        card_color_for_species(species),
        |local| base + rotation * (local * scale),
        |direction| rotation * direction,
        |_, _| 0.0,
    );
}

/// Expand a baked tree's card deck into `SceneVertex` quads — ONE expansion for both paths
/// (the instanced ladder and the statics bake), so a card can never render differently by
/// route. Each card is 8 vertices / 4 triangles: dual winding with a normal ring per face,
/// or a card seen from behind lights only by the transmission lobe and reads as a black
/// cutout. `place` maps a tree-local position into the output space, `rotate` maps a
/// direction, `sway` answers per-corner wind allowance given `(corner, card_center)` — the
/// card center is what a per-CARD wind decision (the L2 branch jitter) keys off, so all
/// eight vertices of one card agree and the quad never shears (the statics bake passes zero).
pub(crate) fn push_leaf_cards(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    tree: &world_forge::tree::BakedTree,
    (color, gloss): ([f32; 3], f32),
    place: impl Fn(Vec3) -> Vec3,
    rotate: impl Fn(Vec3) -> Vec3,
    sway: impl Fn(Vec3, Vec3) -> f32,
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
                    .with_sway(sway(local, card.center)),
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

/// What a CARD is tinted: white for a species with authored cluster sprites (route 2 — the
/// sprite carries its own albedo, and a tint on top would double-colour it), else the
/// species canopy tone the procedural masks are multiplied by. Shared by the statics bake,
/// the ladder and the impostor splat, so every route colours a card the same way.
pub(crate) fn card_color_for_species(species: world_forge::tree::TreeSpecies) -> ([f32; 3], f32) {
    let (tone, gloss) = canopy_color_for_species(species);
    if world_forge::tree::authored::clusters(species).is_some() {
        ([1.0, 1.0, 1.0], gloss)
    } else {
        (tone, gloss)
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
        world_forge::tree::TreeSpecies::Bush => CANOPY_SCRUB,
        world_forge::tree::TreeSpecies::Pine => CANOPY_PINE,
    }
}

/// The impostor of a species: two crossed vertical quads sampling the pre-splatted sprite
/// pair in the foliage atlas (Drzewa 3.0 PR10) — ONE expansion for both routes, the
/// instanced ladder (`tree_lod`, tree-local space) and the statics bake (the backdrop ring,
/// placed by the instance transform), so a far tree can never render differently by route.
/// The sprite stores albedo·shade and the quads ride the FOLIAGE role, so the tree is lit
/// live by the same model as its cards; the quad extents come from the SAME window the
/// splat used, so silhouette agreement is shared math, not tuning. Vertex colour stays
/// white — the sprite carries the tree's own tones. Each quad is double-faced on its own
/// normal ring, like a leaf card: seen from behind it lights, not blacks out.
pub(crate) fn push_impostor_quads(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    species: world_forge::tree::TreeSpecies,
    place: impl Fn(Vec3) -> Vec3,
    rotate: impl Fn(Vec3) -> Vec3,
) {
    let window = crate::foliage_atlas_paint::impostor_window(species);
    let (_, gloss) = canopy_color_for_species(species);
    for which in 0..2u32 {
        // Azimuth 0 spans X and faces ±Z; azimuth 1 spans Z and faces ∓X — the two views the
        // paint side splatted.
        let (right, facing) = if which == 0 { (Vec3::X, Vec3::Z) } else { (Vec3::Z, -Vec3::X) };
        let rect = world_forge::tree::leaf_atlas::impostor_rect(species, which);
        let corners = [
            (right * -window.half_width_m + Vec3::Y * window.bottom_m, [rect[0], rect[3]]),
            (right * window.half_width_m + Vec3::Y * window.bottom_m, [rect[2], rect[3]]),
            (right * window.half_width_m + Vec3::Y * window.top_m, [rect[2], rect[1]]),
            (right * -window.half_width_m + Vec3::Y * window.top_m, [rect[0], rect[1]]),
        ];
        let start = vertices.len() as u32;
        for face_normal in [facing, -facing] {
            let normal = rotate(face_normal).normalize_or_zero();
            for (local, uv) in corners {
                vertices.push(
                    SceneVertex::surfaced(
                        place(local).to_array(),
                        normal.to_array(),
                        [1.0, 1.0, 1.0],
                        gloss,
                    )
                    .with_surface(renderer_api::surface_role::FOLIAGE)
                    .with_uv(uv)
                    .with_sway(0.0),
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
            start + 4,
            start + 6,
            start + 5,
            start + 4,
            start + 7,
            start + 6,
        ]);
    }
}

/// A far tree baked into the statics at its instance transform: the backdrop ring's route.
/// The trunk sinks by the ladder's own constant so a ring tree roots in the enclosing hills
/// the way a battlefield oak roots in the field. A non-tree kind draws nothing.
pub(crate) fn push_impostor_tree(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    instance: &SceneryInstance,
) {
    let Some(species) = tree_species(instance.kind) else {
        return;
    };
    let base = Vec3::from_array(instance.position) - Vec3::Y * crate::tree_lod::TRUNK_SINK_M;
    let rotation = Mat3::from_rotation_y(instance.yaw_rad);
    let scale = instance.scale;
    push_impostor_quads(
        vertices,
        indices,
        species,
        |local| base + rotation * (local * scale),
        |direction| rotation * direction,
    );
}

// Each material is (color, gloss): bark is matte, leaf canopies carry the faint waxy sheen
// that answers a wet sky without ever reading as plastic.
pub(crate) const TRUNK_TONE: ([f32; 3], f32) = ([0.30, 0.22, 0.14], 0.04);
const TRUNK: ([f32; 3], f32) = TRUNK_TONE;
const CANOPY: ([f32; 3], f32) = ([0.18, 0.34, 0.15], 0.07);
const CANOPY_DARK: ([f32; 3], f32) = ([0.13, 0.27, 0.12], 0.06);
/// Scrub foliage (hawthorn, elder) genuinely runs darker than a broadleaf crown — and the
/// steppe's overcast dark plane (rule 1) rides on these tufts now that they are cards with
/// lit rims instead of solid occluded blobs.
const CANOPY_SCRUB: ([f32; 3], f32) = ([0.09, 0.20, 0.09], 0.05);
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

    /// B2's on-screen contract for the trees the statics bake still grows — the planted tree
    /// line's stations (`tree_line`), which call this route directly; every free-standing tree
    /// species rides the instanced ladder since F7: a baked tree is the BAKED species (real
    /// geometry, canopy normals bent from the crown centroid), deterministic per position — the
    /// scene bake is identical every time, yet no two shelterbelt trees repeat.
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
            push_baked_tree(&mut vertices, &mut indices, instance);
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
        // Every tree species rides the instanced ladder (F7), so the near bake owes it
        // NOTHING — a species baked here too would draw twice. The tree line's stations still
        // bake through `push_baked_tree` directly and keep their own budget in `tree_line`.
        for kind in SceneryKind::ALL {
            let on_ladder = crate::tree_lod::ladder_species(kind).is_some();
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            push_scenery_instance(
                &mut vertices,
                &mut indices,
                &SceneryInstance { kind, position: [10.0, 3.0, 10.0], yaw_rad: 0.7, scale: 1.3 },
                StoneTone::NEUTRAL,
            );
            if on_ladder {
                assert!(indices.is_empty(), "{kind:?} rides the ladder and must not bake near");
            }
        }
        // The bush bakes at CLOSE (its Mid deck vanished at 200 m and took the steppe's dark
        // plane with it), so its ceiling is its own: full card deck + 4-sided stick bark.
        const BUSH_STATICS_MAX_TRIS: usize = 1_000;
        for (kind, ceiling) in [
            (SceneryKind::Bush, BUSH_STATICS_MAX_TRIS),
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
