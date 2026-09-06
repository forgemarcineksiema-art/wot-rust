//! A recipe split into pieces (Forge 2.0 K3): the Tiger I describes as the five builders its
//! recipe is made of, each a `Recipe` part with a name the part library can replace one at a
//! time, and the description bakes the recipe's exact bytes (`vehicle_forge/tests/seam_lock.rs`
//! pins the hash against the recipe golden; this file pins the shape of the split).

use game_core::VehicleKind;
use vehicle_build::{GeneratorKind, PostMerge};
use vehicle_geometry::SubmeshKind;
use vehicle_recipes::describe;

#[test]
fn the_tiger_describes_as_the_pieces_its_recipe_still_owns() {
    let description = describe(VehicleKind::TigerI).expect("describes");
    // The recipe pieces come first; the library's fittings (K3-2b) follow them.
    let names: Vec<(&str, SubmeshKind)> = description
        .parts
        .iter()
        .filter(|p| p.generator == GeneratorKind::Recipe)
        .map(|p| (p.key.name, p.submesh))
        .collect();
    assert!(
        names.is_empty(),
        "step 4d: the library owns every part of the shipped Tiger: {names:?}"
    );
    assert!(
        description.parts.iter().any(|p| p.generator != GeneratorKind::Recipe),
        "K3-2b: the library rides along"
    );
    assert_eq!(description.post_merge, PostMerge::WeldAndSmooth, "a recipe welds after the merge");
    assert!(
        description.surface_bake.cavities.iter().all(|c| c.scope.is_some()),
        "every recipe band is scoped to the submesh `assemble` applied it to"
    );
    assert!(!description.surface_bake.is_empty(), "the blueprint's cavity bands ride along");
}

#[test]
fn the_tiger_ii_describes_as_library_parts_alone() {
    let description = describe(VehicleKind::TigerII).expect("describes");
    let recipe: Vec<&str> = description
        .parts
        .iter()
        .filter(|p| p.generator == GeneratorKind::Recipe)
        .map(|p| p.key.name)
        .collect();
    assert!(recipe.is_empty(), "K3 Tiger II: the library owns every part: {recipe:?}");
    let names: Vec<&str> = description.parts.iter().map(|p| p.key.name).collect();
    for key in ["slab_upper_box", "turret_shell", "gun_barrel", "skirt_plate", "periscope_hood"] {
        assert!(names.iter().any(|n| n.starts_with(key)), "{key} is built: {names:?}");
    }
    assert_eq!(description.post_merge, PostMerge::WeldAndSmooth, "welded after the merge");
}

#[test]
fn the_panther_ii_describes_as_library_parts_alone() {
    let description = describe(VehicleKind::PantherII).expect("describes");
    let recipe: Vec<&str> = description
        .parts
        .iter()
        .filter(|p| p.generator == GeneratorKind::Recipe)
        .map(|p| p.key.name)
        .collect();
    assert!(recipe.is_empty(), "K3 Panther II: the library owns every part: {recipe:?}");
    let names: Vec<&str> = description.parts.iter().map(|p| p.key.name).collect();
    for key in ["slab_upper_box", "turret_shell", "gun_barrel", "fender_sweep", "periscope_hood"] {
        assert!(names.iter().any(|n| n.starts_with(key)), "{key} is built: {names:?}");
    }
    assert!(!names.contains(&"fender_flap"), "the Panther sweeps, it does not flap");
}

#[test]
fn the_jagdtiger_describes_as_library_parts_alone() {
    let description = describe(VehicleKind::Jagdtiger).expect("describes");
    let recipe: Vec<&str> = description
        .parts
        .iter()
        .filter(|p| p.generator == GeneratorKind::Recipe)
        .map(|p| p.key.name)
        .collect();
    assert!(recipe.is_empty(), "K3 Jagdtiger: the library owns every part: {recipe:?}");
    let names: Vec<&str> = description.parts.iter().map(|p| p.key.name).collect();
    for key in ["turret_shell", "casemate_hatch", "spare_track_rail", "fender_guard", "tow_cable"] {
        assert!(names.iter().any(|n| n.starts_with(key)), "{key} is built: {names:?}");
    }
    assert!(!names.contains(&"cupola_drum") && !names.contains(&"cupola_hatch"), "no cupola");
}

#[test]
fn the_t34_85_describes_as_library_parts_alone() {
    let description = describe(VehicleKind::T34_85).expect("describes");
    let recipe: Vec<&str> = description
        .parts
        .iter()
        .filter(|p| p.generator == GeneratorKind::Recipe)
        .map(|p| p.key.name)
        .collect();
    assert!(recipe.is_empty(), "K3 T-34-85: the library owns every part: {recipe:?}");
    let names: Vec<&str> = description.parts.iter().map(|p| p.key.name).collect();
    for key in
        ["slab_upper_box", "turret_shell", "cupola_drum", "glacis_hatch", "engine_deck_louvre"]
    {
        assert!(names.iter().any(|n| n.starts_with(key)), "{key} is built: {names:?}");
    }
}

#[test]
fn an_unsplit_recipe_still_wraps_its_three_submeshes() {
    let description = describe(VehicleKind::IS3).expect("describes");
    let names: Vec<&str> = description.parts.iter().map(|p| p.key.name).collect();
    assert_eq!(names, vec!["recipe_hull", "recipe_turret", "recipe_gun"]);
    assert_eq!(description.post_merge, PostMerge::None, "the wrapped submeshes are already welded");
}
