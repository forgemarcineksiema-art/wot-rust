//! Ostrogorsk's BATTLE invariants (urban-map program PR-14): the promises the city makes to
//! the fight, proven on the compiled shipped map through the same sim primitives the server
//! runs — not on synthetic fixtures. A tenement row blocks the eye across the block and the
//! street carries it; collapsing the row opens the pair over the mound (destruction changes
//! the map — as a test, not a slogan); and the born-ruins are already rubble in the battle's
//! initial states, wire bytes included.

use glam::Vec3;
use sim::{
    CoverPhase, damage_cover, initial_cover_states, line_of_sight, live_cover_for_sight_and_shells,
};
use terrain::MapId;

fn city() -> terrain::BattlefieldMap {
    map_forge::battlefield(MapId::Ostrogorsk)
}

/// Commander-eye point over the ground at (x, z).
fn eye(map: &terrain::BattlefieldMap, x: f32, z: f32) -> Vec3 {
    let ground = map.heightmap.sample_height(x, z).expect("inside map");
    Vec3::new(x, ground + 2.3, z)
}

/// Hull-centre target point over the ground at (x, z).
fn hull(map: &terrain::BattlefieldMap, x: f32, z: f32) -> Vec3 {
    let ground = map.heightmap.sample_height(x, z).expect("inside map");
    Vec3::new(x, ground + 1.0, z)
}

/// (a) + (b): the tenement row at (250, 468) blocks the cross-block sightline while the
/// market-lane canyon carries the eye — and a scripted collapse of that row opens the pair
/// over the low mound. This is the whole urban promise in one test: buildings write the
/// sightline map, and destruction rewrites it.
#[test]
fn a_tenement_row_blocks_until_it_collapses_and_the_street_always_carries() {
    let map = city();
    let mut states = initial_cover_states(&map.static_cover);
    let live = live_cover_for_sight_and_shells(&map.static_cover, &states);

    // Across the block, through the row at z 468: blocked while the tenement stands.
    let from = eye(&map, 250.0, 450.0);
    let to = hull(&map, 250.0, 486.0);
    assert!(
        !line_of_sight(Some(&map.heightmap), &live, from, to),
        "the standing tenement row must block the cross-block sightline"
    );
    // Along the market-lane canyon: the street is the sightline.
    let a = eye(&map, 200.0, 446.0);
    let b = hull(&map, 400.0, 446.0);
    assert!(
        line_of_sight(Some(&map.heightmap), &live, a, b),
        "the street canyon must carry the eye down its own axis"
    );

    // Collapse every standing block on the z=468 row line between the two positions.
    let mut collapsed = 0;
    for (index, cover) in map.static_cover.iter().enumerate() {
        if (cover.center[2] - 468.0).abs() < 6.0
            && (200.0..=300.0).contains(&cover.center[0])
            && cover.kind == terrain::StaticCoverKind::CityBuilding
        {
            damage_cover(&mut states, &map.static_cover, index, u32::MAX);
            assert_eq!(states[index].phase, CoverPhase::Rubble, "{} collapses", cover.id);
            collapsed += 1;
        }
    }
    assert!(collapsed > 0, "the row line must carry at least one standing block to fell");

    let live_after = live_cover_for_sight_and_shells(&map.static_cover, &states);
    // The spotting recompute samples the hull centre AND the turret top: over the mound the
    // TURRET line opens (the pair lights up), while the HULL line stays covered — the mound
    // is still cover, just no longer a wall. Both halves of that promise, asserted.
    let turret_to = eye(&map, 250.0, 486.0);
    assert!(
        line_of_sight(Some(&map.heightmap), &live_after, from, turret_to),
        "after the collapse the turret line must open OVER the mound - the per-kind \
         rubble fraction keeps a felled 11 m block under turret eyes"
    );
    assert!(
        !line_of_sight(Some(&map.heightmap), &live_after, from, to),
        "the hull line stays covered - the mound is still cover, not a vacuum"
    );
    // And the mound still stops a hull: it is present in the live slice, lower but real.
    let mound = live_after
        .iter()
        .find(|c| (c.center[2] - 468.0).abs() < 6.0 && (200.0..=300.0).contains(&c.center[0]))
        .expect("the mound remains a blocking box");
    assert!(mound.half_extents_m[1] > 0.5, "the mound still stops a hull");
    assert!(mound.half_extents_m[1] < 1.5, "but it stays under the sightline");
}

/// (c): the born-ruins are already rubble in the battle's initial states — on the compiled
/// shipped map, with the same wire bytes every client and late joiner receives.
#[test]
fn the_born_ruins_open_the_city_from_tick_zero() {
    let map = city();
    let states = initial_cover_states(&map.static_cover);
    let mut ruined = 0;
    for (index, cover) in map.static_cover.iter().enumerate() {
        if cover.id.contains("ruin") {
            assert_eq!(
                states[index].phase,
                CoverPhase::Rubble,
                "{} must be born collapsed",
                cover.id
            );
            assert_eq!(states[index].phase.to_wire(), 1, "and ride the wire as rubble");
            ruined += 1;
        } else {
            assert_eq!(states[index].phase, CoverPhase::Intact, "{} is born whole", cover.id);
        }
    }
    assert_eq!(ruined, 6, "three mirrored born-ruin pairs open the sightlines");
}
