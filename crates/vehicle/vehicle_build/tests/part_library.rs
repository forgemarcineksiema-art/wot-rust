//! The fleet part library (Forge 2.0 K2): the T-54's hull, plates and fittings moved out of the
//! `solid` kernel into this crate, and lost their `t54_` prefix on the way — they read blueprint
//! visuals, so any vehicle carrying those visuals builds with them.
//!
//! The move is byte-exact. The hash below was taken on master before the files moved
//! (`t54_description().build().deterministic_hash()`, 27 565 triangles); a construction PR that
//! moves the bake re-records it deliberately, with the number in its message.

use game_core::{VehicleBlueprint, VehicleKind};
use vehicle_build::t54_description;
use vehicle_geometry::{MaterialRole, SmoothingGroup};

const T54_LOD0_HASH_BEFORE_THE_MOVE: u64 = 9_296_666_834_409_964_133;
const T54_LOD0_TRIS_BEFORE_THE_MOVE: usize = 27_565;

#[test]
fn moving_the_parts_out_of_the_kernel_left_the_bake_untouched() {
    let baked = t54_description().build();
    let tris: usize = baked.submeshes().iter().map(|s| s.mesh.triangle_count()).sum();
    assert_eq!(tris, T54_LOD0_TRIS_BEFORE_THE_MOVE);
    assert_eq!(
        baked.deterministic_hash(),
        T54_LOD0_HASH_BEFORE_THE_MOVE,
        "the T-54 bake changed under the part-library move — a move is not a construction PR"
    );
}

#[test]
fn the_library_builds_a_hull_from_blueprint_visuals_alone() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let v = bp.complete_visual().expect("hybrid visual");
    let hull = vehicle_build::hull_solid(
        v.hull,
        bp.armor.hull_front.0,
        bp.armor.hull_side.0,
        bp.armor.hull_rear.0,
        bp.armor.hull_rear_knuckle,
    )
    .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
    .expect("hull solid is valid");
    assert!(hull.triangle_count() > 0);
    let seams = vehicle_build::hull_plate_seams(v.hull, bp.armor.hull_front.0);
    let covers = vehicle_build::transmission_covers(v.deck);
    let grille = vehicle_build::deck_grille(v.detail, v.deck.center.y + v.deck.half.y);
    assert!(!seams.is_empty() && !covers.is_empty() && !grille.is_empty());
}
