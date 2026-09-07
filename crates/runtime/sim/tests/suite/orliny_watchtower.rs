//! The Orlinoye watchtower promises, certified by the SIM'S OWN EYE (teren W3b tail).
//!
//! Two contracts make the landmark worth its stone: the crown breaks the pass skyline
//! from BOTH spawn roads (the objective draws the eye from the first minute), and the
//! FELLED tower leaves a stump in the hull-down band — the turret line over it opens
//! while the hull line stays covered, so destruction turns the landmark into a fighting
//! position instead of a wall. Both proven with `sim::line_of_sight` — the exact rule the
//! spotting recompute resolves with — and the benchmark T-54's real geometry.

use game_core::TankSpec;
use glam::Vec3;
use sim::line_of_sight;
use terrain::{BattlefieldMap, MapId};

#[allow(unused_imports)]
use super::common;
use common::commander_eye as eye;

fn map() -> BattlefieldMap {
    map_forge::battlefield(MapId::OrlinyPereval)
}

/// The crown of the tower breaks the skyline from both spawn roads: a commander rolling
/// out of either spawn sees the crown's near arris (a probe 0.4 m in FRONT of the box at
/// exactly box-top height, so the tower's own collider cannot shadow its own tip). The
/// landmark is not decoration — it is the map explaining its objective.
#[test]
fn the_watchtower_crests_the_skyline_from_both_spawn_roads() {
    let map = map();
    let tower = map
        .static_cover
        .iter()
        .find(|object| object.id == "col_watchtower")
        .expect("the col watchtower ships");
    let top_y = tower.center[1] + tower.half_extents_m[1];
    for (spawn_z, facing) in [(140.0, 1.0f32), (860.0, -1.0)] {
        let spawn_eye = eye(&map, 500.0, spawn_z);
        let crown_arris =
            Vec3::new(500.0, top_y, tower.center[2] - facing * (tower.half_extents_m[2] + 0.4));
        assert!(
            line_of_sight(Some(&map.heightmap), &map.static_cover, &[], spawn_eye, crown_arris),
            "the crown must break the skyline from the spawn road at z {spawn_z}"
        );
    }
}

/// The felled tower leaves a HULL-DOWN stump on the cap point: across the col bench the
/// intact shaft blocks everything, and after the collapse the turret line clears the
/// mound while the hull line of a tank sitting behind it stays covered. This is the 0.11
/// rubble fraction earning its number on the shipped map — the landmark falls into a
/// fighting position.
///
/// The mound is ONE shape (X11): the 38° pyramid the hull climbs is what the eye meets. A
/// 5.2 m footprint holds a 2.03 m pile at the repose angle (the doctrine's 2.2 m was the
/// old sight box's height; the hull always climbed the 2.03 m pyramid), pointed — so the
/// hull-down band is measured where a hull-down tank sits, 6 m behind the apex (3.4 m behind
/// the talus foot), not 22 m across the bench where the crossing line grazes the apex by 3 mm.
#[test]
fn the_felled_watchtower_leaves_a_hull_down_stump_on_the_col() {
    use sim::{CoverPhase, damage_cover, initial_cover_states, live_cover_for_sight_and_shells};
    let map = map();
    let mut states = initial_cover_states(&map.static_cover);
    let (index, _) = map
        .static_cover
        .iter()
        .enumerate()
        .find(|(_, object)| object.id == "col_watchtower")
        .expect("the col watchtower ships");

    let spec = TankSpec::t54_1951();
    let west_eye = eye(&map, 478.0, 500.0);
    let east_ground = map.heightmap.sample_height(506.0, 500.0).expect("east of the tower");
    let east_hull = Vec3::new(506.0, east_ground + spec.hitbox.center_y_m, 500.0);
    let east_turret =
        Vec3::new(506.0, east_ground + spec.hitbox.center_y_m + spec.hitbox.half_height_m, 500.0);

    let live = live_cover_for_sight_and_shells(&map.static_cover, &states);
    assert!(
        !line_of_sight(Some(&map.heightmap), &live, &[], west_eye, east_turret),
        "the intact shaft must block the cross-col turret line"
    );

    damage_cover(&mut states, &map.static_cover, index, u32::MAX, 0.0);
    assert_eq!(states[index].phase, CoverPhase::Rubble, "masonry falls into rubble, not thin air");
    let live_after = live_cover_for_sight_and_shells(&map.static_cover, &states);
    // The mound as the pyramid the eye meets (X11): the sight slice carries no box for it.
    let mounds_after = sim::rubble_mounds(&map.static_cover, &states);
    assert!(
        line_of_sight(Some(&map.heightmap), &live_after, &mounds_after, west_eye, east_turret),
        "the stump must stay under turret eyes - the col reopens when the tower falls"
    );
    assert!(
        !line_of_sight(Some(&map.heightmap), &live_after, &mounds_after, west_eye, east_hull),
        "and the hull line stays covered - the stump is a fighting mound, not a vacuum"
    );
    // The number, so the lock cannot drift onto a knife edge: the pile is capped by the repose
    // angle over half the footprint (2.6 m × 0.78), not by the 2.2 m fraction.
    let stump = mounds_after[0];
    let height = stump.crest_y_m - stump.base_y_m;
    assert!((height - 2.028).abs() < 0.01, "a 2.03 m pointed pile: {height} m");
    assert_eq!(stump.crown_half_m, [0.0, 0.0], "...pointed: the talus meets in the middle");
}
