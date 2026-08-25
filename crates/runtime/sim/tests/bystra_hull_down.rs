//! The Bystra hull-down promises, certified by the SIM'S OWN EYE.
//!
//! These two contracts lived in `map_forge/tests/bystra_map.rs` with a local `sight_clear`
//! copy whose slack carried the OPPOSITE sign to the sim's (`ground > line - 0.3` vs
//! spotting's `ground > point.y + 0.3`) — the instrument certified crests the live game
//! saw straight over. The tests now call `sim::line_of_sight` — the exact rule the
//! spotting recompute and the bots resolve with — with the benchmark T-54's real geometry
//! (`observer_eye` = hitbox top, target points = hull centre / hitbox top), so a promise
//! proven here is a promise a crew can stand on.

use game_core::TankSpec;
use glam::Vec3;
use sim::line_of_sight;
use terrain::{BattlefieldMap, MapId, bystra_river_center_x};

const HALF_M: f32 = 500.0;

fn map() -> BattlefieldMap {
    map_forge::battlefield(MapId::BystraValley)
}

/// The commander's eye over a ground point: the hitbox top, exactly as
/// `sim::spotting::observer_eye` stacks it over a hull standing there.
fn eye(map: &BattlefieldMap, x: f32, z: f32) -> Vec3 {
    let spec = TankSpec::t54_1951();
    let ground = map.heightmap.sample_height(x, z).expect("probe on the map");
    Vec3::new(x, ground + spec.hitbox.center_y_m + spec.hitbox.half_height_m, z)
}

/// A target's two sample points over a ground point: hull centre and hitbox top, exactly
/// as `sim::spotting::target_points` presents a hull to an observer.
fn target_points(map: &BattlefieldMap, x: f32, z: f32) -> [Vec3; 2] {
    let spec = TankSpec::t54_1951();
    let ground = map.heightmap.sample_height(x, z).expect("probe on the map");
    [
        Vec3::new(x, ground + spec.hitbox.center_y_m, z),
        Vec3::new(x, ground + spec.hitbox.center_y_m + spec.hitbox.half_height_m, z),
    ]
}

/// The western high ground works: from the Windmill Hill crest, a commander's eye sees a
/// hull on the stone bridge's deck across the floodplain.
///
/// The probe stands BESIDE the windmill, not at (250, 500) — the old instrument ignored
/// cover boxes and happily certified an overwatch from inside the mill tower. The honest
/// eye stands on the crest shoulder a hull can actually park on.
#[test]
fn windmill_hill_overwatches_the_stone_bridge() {
    let map = map();
    let bridge_x = bystra_river_center_x(HALF_M);
    let hill_eye = eye(&map, 250.0, HALF_M - 14.0);
    let seen = target_points(&map, bridge_x, HALF_M)
        .into_iter()
        .any(|point| line_of_sight(Some(&map.heightmap), &map.static_cover, hill_eye, point));
    assert!(seen, "the Windmill Hill crest must see a hull on the bridge deck");
}

/// The hull-down shelf works exactly like a hull-down shelf, under the live spotting rule:
/// from the bridge, the shelf's hull-centre point is masked by the crest while the turret
/// top still works — so a crew parked on the certified shelf trades turret-only.
#[test]
fn windmill_shelf_masks_a_hull_from_the_bridge_but_fires_over_the_crest() {
    let map = map();
    let bridge_x = bystra_river_center_x(HALF_M);
    let bridge_eye = eye(&map, bridge_x, HALF_M);
    let (shelf_x, shelf_z) = (340.0, HALF_M - 90.0);
    let [hull, turret] = target_points(&map, shelf_x, shelf_z);
    assert!(
        !line_of_sight(Some(&map.heightmap), &map.static_cover, bridge_eye, hull),
        "the crest must mask the hull centre on the shelf from the bridge"
    );
    assert!(
        line_of_sight(Some(&map.heightmap), &map.static_cover, bridge_eye, turret),
        "the turret above the crest must still work the bridge"
    );
}
