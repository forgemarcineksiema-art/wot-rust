//! The low tier is low for the WHOLE fleet (the one program's X4).
//!
//! `StaticCoverKind::LowWall` promises a step every running gear climbs. The step is the
//! vehicle's (`HullPlan::step_m`, the belt's top run less a clearance), so the promise is a
//! number per map: no shipped low wall stands taller than the shortest step in the roster —
//! or the IS-3 would meet a wall where the T-54 meets a kerb.

use game_core::{HullPlan, VehicleKind};
use terrain::{MapId, StaticCoverKind};

#[test]
fn every_low_wall_on_every_shipped_map_is_a_step_for_every_hull() {
    let fleet_step = VehicleKind::PLAYABLE
        .iter()
        .map(|&kind| HullPlan::for_vehicle(kind).step_m)
        .fold(f32::INFINITY, f32::min);
    assert!(
        (0.6..0.9).contains(&fleet_step),
        "the fleet's shortest step must be a tank's vertical obstacle, got {fleet_step} m"
    );

    let mut low_walls = 0;
    for id in MapId::ALL {
        let map = map_forge::battlefield(id);
        for object in &map.static_cover {
            if object.kind != StaticCoverKind::LowWall {
                continue;
            }
            low_walls += 1;
            let height_m = 2.0 * object.half_extents_m[1];
            assert!(
                height_m <= fleet_step + 1.0e-4,
                "{:?}: `{}` is {height_m} m of LowWall, over the fleet's {fleet_step} m step — \
                 a wall for the IS-3, a step for the rest",
                id,
                object.id
            );
            assert!(height_m >= 0.3, "{:?}: `{}` is not a wall at {height_m} m", id, object.id);
        }
    }
    assert!(low_walls >= 2, "the Kamienna bridge parapets are the first of the tier");
}
