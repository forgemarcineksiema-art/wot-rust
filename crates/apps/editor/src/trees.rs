//! The planted trees in the viewport (F9, the owner: "can I use all these trees in the
//! editor?"). Since F7 every planted tree draws through the instanced LOD ladder, not the
//! statics bake — an editor that only uploaded the statics showed a placed oak as NOTHING and
//! the owner would have taken the palette for broken. The editor now submits the same ladder
//! instances the battle does, from the same eye rule, so a tree in the viewport is the tree
//! the game will draw at that distance.

use renderer_api::RenderObject;
use scene_build::tree_lod::{TreeEye, TreeLodState, tree_frame_objects_with_backdrop};

use crate::CompiledMap;

/// The ladder instances for a compiled document as seen from `eye`. `born_phases` are the
/// cover phase bytes the born-ruins preview already uses (a tree inside a levelled tree-line
/// box is gone, exactly as in the battle).
pub fn planted_tree_objects(
    compiled: &CompiledMap,
    born_phases: &[u8],
    eye: TreeEye,
    state: &mut TreeLodState,
) -> Vec<RenderObject> {
    tree_frame_objects_with_backdrop(&compiled.battlefield, born_phases, eye, state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objects::{PaletteEntry, place_entry};
    use glam::Vec3;

    /// Every ladder tree the palette can plant is a drawn instance in the editor's frame —
    /// the oak the owner places is the oak they see.
    #[test]
    fn every_planted_ladder_tree_draws_in_the_viewport() {
        let mut document = crate::EditorDocument::new_scratch();
        let planted =
            [PaletteEntry::Oak, PaletteEntry::Poplar, PaletteEntry::FruitTree, PaletteEntry::Bush];
        document.apply_edit(|blueprint| {
            for (index, entry) in planted.iter().enumerate() {
                place_entry(blueprint, *entry, [120.0 + 30.0 * index as f32, 90.0]);
            }
        });
        let compiled = document.recompile();
        let born = terrain::initial_cover_phase_bytes(&compiled.battlefield.static_cover);
        let mut state = TreeLodState::default();
        let objects = planted_tree_objects(
            &compiled,
            &born,
            TreeEye::at(Vec3::new(130.0, 2.0, 95.0)),
            &mut state,
        );
        // Each placement is a tree AND its symmetry twin. A tree may be two or three objects
        // (a cross-fade band, the impostor's two quads) whose windows partition [0, 1): the
        // one starting at 0 counts the tree.
        let trees = objects.iter().filter(|o| o.dither[0] == 0.0).count();
        assert_eq!(trees, planted.len() * 2, "one tree per planted tree");
        for object in &objects {
            assert!(
                scene_build::tree_lod::tree_lod_meshes()
                    .iter()
                    .any(|(handle, _)| *handle == object.mesh),
                "every instance is a registered ladder rung"
            );
        }
    }
}
