//! A recipe split into pieces (Forge 2.0 K3): the Tiger I describes as the five builders its
//! recipe is made of, each a `Recipe` part with a name the part library can replace one at a
//! time, and the description bakes the recipe's exact bytes (`vehicle_forge/tests/seam_lock.rs`
//! pins the hash against the recipe golden; this file pins the shape of the split).

use game_core::VehicleKind;
use vehicle_build::{GeneratorKind, PostMerge};
use vehicle_geometry::SubmeshKind;
use vehicle_recipes::describe;

#[test]
fn the_tiger_describes_as_the_five_pieces_of_its_recipe() {
    let description = describe(VehicleKind::TigerI).expect("describes");
    let names: Vec<(&str, SubmeshKind)> =
        description.parts.iter().map(|p| (p.key.name, p.submesh)).collect();
    assert_eq!(
        names,
        vec![
            ("recipe_hull_slab", SubmeshKind::Hull),
            ("recipe_hull_deck", SubmeshKind::Hull),
            ("recipe_hull_details", SubmeshKind::Hull),
            ("recipe_turret", SubmeshKind::Turret),
            ("recipe_gun", SubmeshKind::Gun),
        ]
    );
    assert!(description.parts.iter().all(|p| p.generator == GeneratorKind::Recipe));
    assert_eq!(description.post_merge, PostMerge::WeldAndSmooth, "a recipe welds after the merge");
    assert!(
        description.surface_bake.cavities.iter().all(|c| c.scope.is_some()),
        "every recipe band is scoped to the submesh `assemble` applied it to"
    );
    assert!(!description.surface_bake.is_empty(), "the blueprint's cavity bands ride along");
}

#[test]
fn an_unsplit_recipe_still_wraps_its_three_submeshes() {
    let description = describe(VehicleKind::TigerII).expect("describes");
    let names: Vec<&str> = description.parts.iter().map(|p| p.key.name).collect();
    assert_eq!(names, vec!["recipe_hull", "recipe_turret", "recipe_gun"]);
    assert_eq!(description.post_merge, PostMerge::None, "the wrapped submeshes are already welded");
}
