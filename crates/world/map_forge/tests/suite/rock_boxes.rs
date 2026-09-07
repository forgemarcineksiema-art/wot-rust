//! Rocks are neither walls to the sky nor ghosts (the one program's X5).
//!
//! Rule 8 of `docs/map-forge-policy.md`: a SOLID scenery object stays under the fleet's belly
//! line, and one that stands over it is cover. The scatters plant erratics of 0.35–1.45 m, so
//! most field stones are cover — and they were ghosts. Now every one over the line earns a
//! `Boulder` box that is the bounds of the very mesh the picture draws, from the same seed.
//! This is the lock the row asked for: every shipped Rock's top − ground ≤ the belly line, or
//! it is boxed — and boxed HONESTLY, every vertex of the stone inside its box.

use map_forge::{battlefield, boulder_box_of};
use terrain::{CoverBox, MapId, SceneryKind, StaticCoverKind};
use world_forge::rock::{RockForm, bake_rock, rock_seed};

/// How far a stone's vertex may stand outside its box: the box is the mesh's own bounds, so
/// this is float noise, not tolerance.
const SKIN_M: f32 = 0.01;

#[test]
fn every_shipped_rock_is_under_the_belly_line_or_boxed_by_its_own_bounds() {
    let belly_line_m = game_core::fleet_belly_line_m();
    assert!((0.3..0.5).contains(&belly_line_m), "the belly line is a tank's, got {belly_line_m}");
    let mut boxed_anywhere = 0;
    let mut checked = 0;
    for id in MapId::SHIPPED {
        let map = battlefield(*id);
        let boulders: Vec<_> =
            map.static_cover.iter().filter(|c| c.kind == StaticCoverKind::Boulder).collect();
        let rocks: Vec<_> =
            map.scenery.iter().filter(|instance| instance.kind == SceneryKind::Rock).collect();
        let mut tallest = 0.0_f32;
        let mut under = 0;
        for instance in &rocks {
            let rock = bake_rock(RockForm::Erratic, rock_seed(instance.position, instance.seed));
            let top_m = rock.height_m() * instance.scale;
            tallest = tallest.max(top_m);
            checked += 1;
            let Some((center, half)) = boulder_box_of(instance, belly_line_m) else {
                assert!(
                    top_m <= belly_line_m + 1.0e-6,
                    "{id:?}: a {top_m:.2} m stone at {:?} stands over the belly line unboxed",
                    instance.position
                );
                under += 1;
                // A stone under the line is dressing: nothing more to hold it to.
                continue;
            };
            let cover = boulders
                .iter()
                .find(|c| {
                    (0..3).all(|axis| {
                        (c.center[axis] - center[axis]).abs() < 1.0e-4
                            && (c.half_extents_m[axis] - half[axis]).abs() < 1.0e-4
                    })
                })
                .unwrap_or_else(|| {
                    panic!("{id:?}: the {top_m:.2} m stone at {:?} has no box", instance.position)
                });
            // Honest: every vertex of the drawn stone above the ground line lies inside its box,
            // and the box stands on the stone's ground and reaches its crest.
            let plan = CoverBox::of(cover);
            let (sin, cos) = instance.yaw_rad.sin_cos();
            for vertex in rock.body.vertices() {
                let local = vertex.position * instance.scale;
                if local.y <= 0.0 {
                    continue;
                }
                let world = [
                    instance.position[0] + local.x * cos + local.z * sin,
                    instance.position[1] + local.y,
                    instance.position[2] - local.x * sin + local.z * cos,
                ];
                assert!(
                    plan.contains(world, SKIN_M),
                    "{id:?}: a vertex of the stone at {:?} stands outside its box `{}`",
                    instance.position,
                    cover.id
                );
            }
            assert!(
                (cover.center[1] - cover.half_extents_m[1] - instance.position[1]).abs() < 1.0e-4
            );
            assert!(
                (cover.center[1] + cover.half_extents_m[1] - (instance.position[1] + top_m)).abs()
                    < 1.0e-3
            );
        }
        // And no box without a stone.
        assert_eq!(
            boulders.len(),
            rocks.len() - under,
            "{id:?}: every boulder box is one stone over the line"
        );
        boxed_anywhere += boulders.len();
        println!(
            "{id:?}: {} rocks, {} under the {belly_line_m:.2} m belly line, {} boxed, tallest {tallest:.2} m",
            rocks.len(),
            under,
            boulders.len()
        );
    }
    assert!(boxed_anywhere > 0, "the scatters plant stones over the line; none was boxed");
    // The floor: the shipped scatters plant over two hundred stones (216 on 2026-09-07); a walk
    // that checked fewer checked a map that lost its rocks.
    assert!(checked >= 200, "only {checked} stones were checked across the shipped maps");
}
