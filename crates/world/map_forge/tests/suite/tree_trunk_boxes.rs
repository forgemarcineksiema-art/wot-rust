//! Trees are not ghosts (the one program's X10).
//!
//! Only the oak earned a trunk box, and its box was two literals from a generator that no longer
//! ships; the poplars of Bystra, Mazurski and Orliny and the orchards' fruit trees were boles a
//! hull drove through, a shell flew through and an eye saw through. Now every tree the ladder
//! draws earns a `TreeTrunk` box sized off the very wood mesh it draws: the bole's chest radius
//! and its first limb, at the instance's variant and scale, set into the ground by the ladder's
//! own sink. This is the row's lock: every bole vertex under the box's top — the chest-height
//! ones the row names among them — inside the box ± 5 cm, the first limb above 0.9 of the box.

use map_forge::{TRUNK_SINK_M, battlefield, tree_trunk_box_of, trunk_species_for};
use terrain::{CoverBox, MapId, StaticCoverKind};
use world_forge::tree::TreeLod;
use world_forge::tree::authored::{
    LIMB_LEAVES_M, instance_tree_seed, tree_variant, variant_of_seed,
};

/// The row's number: how far a chest-height bole vertex may stand outside its box.
const CHEST_SKIN_M: f32 = 0.05;

#[test]
fn every_shipped_tree_stands_in_a_trunk_box_sized_off_its_own_bole() {
    let mut checked = 0;
    let mut by_species = std::collections::BTreeMap::new();
    for id in MapId::SHIPPED {
        let map = battlefield(*id);
        let trunks: Vec<_> =
            map.static_cover.iter().filter(|c| c.kind == StaticCoverKind::TreeTrunk).collect();
        let mut trees = 0;
        for instance in &map.scenery {
            let Some(species) = trunk_species_for(instance.kind) else { continue };
            trees += 1;
            checked += 1;
            *by_species.entry(format!("{species:?}")).or_insert(0usize) += 1;
            let (center, half) = tree_trunk_box_of(instance).expect("an authored tree earns a box");
            // The box stands on the tree; a mirrored pair shares ONE box (the wider bole's
            // radius, the lower first limb), so it may be a hand wider or shorter than the
            // tree's own — never narrower, never taller.
            let cover = trunks
                .iter()
                .find(|c| {
                    (c.center[0] - center[0]).abs() < 1.0e-4
                        && (c.center[2] - center[2]).abs() < 1.0e-4
                })
                .unwrap_or_else(|| {
                    panic!("{id:?}: the {species:?} at {:?} has no trunk box", instance.position)
                });
            assert!(cover.half_extents_m[0] >= half[0] - 1.0e-4, "the pair's box holds the bole");
            assert!(
                cover.half_extents_m[1] <= half[1] + 1.0e-4,
                "the pair's box ends under the limb"
            );
            let plan = CoverBox::of(cover);
            // The tree as the ladder draws it: its variant from the same seed, at the instance
            // scale, sunk by the ladder's rule (the mirror flips x, which a radius cannot see).
            let (variant, _) = variant_of_seed(instance_tree_seed(instance.position));
            let tree = tree_variant(species, variant, TreeLod::Close);
            let ground = instance.position[1];
            let box_top = cover.center[1] + cover.half_extents_m[1];
            let mut bole_vertices = 0;
            let mut first_limb = f32::INFINITY;
            let bole_radius = cover.half_extents_m[0];
            for vertex in tree.trunk.vertices() {
                let local = vertex.position * instance.scale;
                let world_y = ground - TRUNK_SINK_M + local.y;
                let radial = (local.x * local.x + local.z * local.z).sqrt();
                // A limb: wood past the butt's cylinder by a hand (the compiler's own rule).
                if radial > bole_radius + LIMB_LEAVES_M * instance.scale {
                    first_limb = first_limb.min(world_y);
                    continue;
                }
                // The bole: every vertex of it under the box's top stands inside the box — the
                // chest-height ones the row names, and the rest of the bole with them.
                if world_y >= ground && world_y <= box_top {
                    bole_vertices += 1;
                    assert!(
                        plan.contains_xz(
                            instance.position[0] + local.x,
                            instance.position[2] + local.z,
                            CHEST_SKIN_M
                        ),
                        "{id:?}: a bole vertex of the {species:?} at {:?} stands {radial:.3} m off \
                         the axis at {:.2} m up, outside its {bole_radius:.3} m box (+ 5 cm)",
                        instance.position,
                        world_y - ground
                    );
                }
            }
            assert!(
                bole_vertices > 0,
                "{id:?}: the {species:?} at {:?} has no bole inside its box",
                instance.position
            );
            assert!(
                first_limb >= 0.9 * (box_top - ground) + ground,
                "{id:?}: the {species:?} at {:?} leaves its first limb at {:.2} m over the ground, \
                 inside a box that reaches {:.2} m",
                instance.position,
                first_limb - ground,
                box_top - ground
            );
            assert!(
                (cover.center[1] - cover.half_extents_m[1] - ground).abs() < 1.0e-4,
                "the box starts at the ground the eye sees the bole leave"
            );
        }
        assert_eq!(trunks.len(), trees, "{id:?}: one trunk box per tree, no box without a tree");
        println!("{id:?}: {trees} trees, {} trunk boxes", trunks.len());
    }
    println!("boxed by species: {by_species:?}");
    // The floor: the shipped maps plant hundreds of trees; a walk that checked fewer checked
    // maps that lost their trees.
    assert!(checked >= 300, "only {checked} trees were checked across the shipped maps");
    assert!(by_species.len() >= 3, "the oak, the poplar and the fruit tree all earn boxes");
}
