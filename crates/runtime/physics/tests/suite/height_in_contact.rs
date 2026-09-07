//! Height exists in contact (the one program's X3).
//!
//! The SAT never read `center.y`: a hull carried up a rubble mound collided with the hull below as
//! if the two were level, and a knee-high parapet blocked like a tenement. Contact is now an XZ
//! overlap AND an overlap of height bands — the hull's from its support height up the shell
//! volume's top, a standing solid's from the ground it is planted in up to its top.

use game_core::{ContactFootprint, HullPlan, TankSpec, VehicleKind};
use glam::Vec3;
use physics::{
    ContactBody, ContactCache, GroundLayers, HeightBand, TankFootprint, TankObstacle,
    footprint_blocked_by_cover, footprint_penetration_m, resolve_contacts, resolve_cover_collision,
    support_height,
};
use terrain::{HeightMap, RubbleMound, StaticCoverKind, StaticCoverObject};

fn t54() -> TankFootprint {
    TankFootprint::from_plan(HullPlan::for_vehicle(VehicleKind::T54_1951))
}

fn solid(center: [f32; 3], half: [f32; 3]) -> StaticCoverObject {
    StaticCoverObject {
        id: "solid".into(),
        name: "solid".into(),
        kind: StaticCoverKind::RailCover,
        center,
        half_extents_m: half,
        yaw_rad: 0.0,
    }
}

/// The row's lock: a hull three metres above a one-metre wall passes; the same hull on the
/// ground does not. The boundary is the wall's TOP less the running gear's step (X4): a support
/// from which the top is within the step is over it — the hull climbs, it does not collide.
#[test]
fn a_hull_three_metres_above_a_one_metre_wall_passes_and_one_on_the_ground_does_not() {
    let wall = solid([50.0, 0.5, 50.0], [6.0, 0.5, 0.5]);
    let walls = std::slice::from_ref(&wall);
    let hull = t54();
    let at = |y: f32| Vec3::new(50.0, y, 50.0);
    let climbs_from = 1.0 - hull.step_m;
    assert!(climbs_from > 0.1, "the fixture needs a wall taller than the step");

    assert!(footprint_blocked_by_cover(at(0.0), 0.0, hull, walls), "on the ground the wall blocks");
    assert!(
        footprint_blocked_by_cover(at(climbs_from - 0.01), 0.0, hull, walls),
        "a hand under the step still blocks"
    );
    assert!(
        !footprint_blocked_by_cover(at(climbs_from), 0.0, hull, walls),
        "a support the top is within a step of is over it"
    );
    assert!(!footprint_blocked_by_cover(at(3.0), 0.0, hull, walls), "three metres up passes");

    // ...and the resolver lets the move through untouched, y included.
    let attempted = Vec3::new(50.0, 3.0, 50.0);
    let resolved = resolve_cover_collision(Vec3::new(50.0, 3.0, 40.0), attempted, 0.0, hull, walls);
    assert_eq!(resolved, attempted, "nothing to resolve three metres over a parapet");
    let held = resolve_cover_collision(
        Vec3::new(50.0, 0.0, 40.0),
        Vec3::new(50.0, 0.0, 50.0),
        0.0,
        hull,
        walls,
    );
    assert_eq!(held, Vec3::new(50.0, 0.0, 40.0), "on the ground the same move holds");
}

/// A standing solid is planted in the ground, not floating at its box: a box whose bottom face is
/// four metres up (a building grounded by its centre on a hill, seen from its downhill corner)
/// still blocks a hull on the ground under it. Only the top decides.
#[test]
fn a_standing_solid_blocks_from_the_ground_up_whatever_its_box_bottom_says() {
    let hillside_house = solid([50.0, 5.0, 50.0], [4.0, 1.0, 4.0]);
    let houses = std::slice::from_ref(&hillside_house);
    let hull = t54();
    assert!(
        footprint_blocked_by_cover(Vec3::new(50.0, 0.0, 50.0), 0.0, hull, houses),
        "the hull's top (2.53 m) is under the box's bottom (4 m) and the house blocks all the same"
    );
    assert!(
        !footprint_blocked_by_cover(Vec3::new(50.0, 6.0, 50.0), 0.0, hull, houses),
        "...and a support at the roof (6 m) is over it"
    );
}

/// The row's other lock: a hull on a mound does not shove the hull below. The hull on the pile
/// stands where the support envelope puts it on real rubble (a knocked-down two-storey house,
/// 8 m tall, so 3.2 m of debris); the hull below stands on the ground. Their plans overlap; their bands do not; the
/// solver exchanges nothing. Put the same two hulls level and it does — the height is the only
/// thing that changed.
#[test]
fn a_hull_on_a_mound_does_not_shove_the_hull_below() {
    let map = HeightMap::flat(121, 121, 1.0, 0.0).expect("flat test terrain");
    let house = RubbleMound::from_cover(&StaticCoverObject {
        id: "house".into(),
        name: "house".into(),
        kind: StaticCoverKind::FarmBuilding,
        center: [60.0, 4.0, 60.0],
        half_extents_m: [8.0, 4.0, 6.0],
        yaw_rad: 0.0,
    });
    let spec = TankSpec::t54_1951();
    let running_gear = ContactFootprint::for_vehicle(VehicleKind::T54_1951);
    let on_the_pile = Vec3::new(60.0, 0.0, 60.0);
    let carried_to = support_height(
        &map,
        on_the_pile,
        0.0,
        &running_gear,
        GroundLayers::rubble(std::slice::from_ref(&house)),
    )
    .expect("the mound carries the hull");
    let hull = TankFootprint::from_plan(spec.hull_plan());
    assert!(
        carried_to > hull.height_m,
        "the fixture needs the pile taller than the hull, got {carried_to} m over {} m",
        hull.height_m
    );

    let body = |id: u64, position: Vec3, velocity: Vec3| ContactBody {
        id,
        position,
        velocity,
        yaw_rad: 0.0,
        yaw_rate_rad_s: 0.0,
        footprint: hull,
        mass_kg: spec.mass_kg,
        movable: true,
    };
    // The plans overlap by half a metre across the flank; the upper hull is driving into it.
    let above = body(1, Vec3::new(60.0, carried_to, 60.0), Vec3::new(2.0, 0.0, 0.0));
    let below = body(2, Vec3::new(60.0 + 2.0 * hull.half_width_m - 0.5, 0.0, 60.0), Vec3::ZERO);

    assert_eq!(
        footprint_penetration_m(
            &TankObstacle::new(above.position, 0.0, hull),
            &TankObstacle::new(below.position, 0.0, hull)
        ),
        0.0,
        "the solver's own penetration reads nothing between a hull on the pile and one below"
    );
    let report = resolve_contacts(&[above, below], &mut ContactCache::default(), 1.0 / 60.0);
    assert!(report.pairs.is_empty(), "no pair: the bands do not meet, {:?}", report.pairs);
    assert!(
        report.bodies.iter().all(|impulse| impulse.delta_velocity == Vec3::ZERO),
        "nothing exchanged, got {:?}",
        report.bodies
    );

    // Level, the same pair is a contact — the pile is the only difference.
    let level = ContactBody { position: Vec3::new(60.0, 0.0, 60.0), ..above };
    let report = resolve_contacts(&[level, below], &mut ContactCache::default(), 1.0 / 60.0);
    assert_eq!(report.pairs.len(), 1, "level, the pair touches");
    assert!(report.bodies[1].delta_velocity.x > 0.0, "...and the lower hull is shoved");
}

/// The band's edge rule, stated once: touching edge to edge is not an overlap, so a hull resting
/// exactly on a solid's top is carried by it (X4), never blocked by it.
#[test]
fn bands_touching_edge_to_edge_do_not_overlap() {
    let wall = HeightBand { bottom_m: f32::NEG_INFINITY, top_m: 1.0 };
    assert!(HeightBand { bottom_m: 0.0, top_m: 2.3 }.overlaps(wall));
    assert!(!HeightBand { bottom_m: 1.0, top_m: 3.3 }.overlaps(wall));
    assert!(!HeightBand { bottom_m: 3.0, top_m: 5.3 }.overlaps(wall));
    let hull = HeightBand { bottom_m: 0.0, top_m: 2.33 };
    assert!(!hull.overlaps(HeightBand { bottom_m: 2.33, top_m: 4.66 }));
    assert!(hull.overlaps(HeightBand { bottom_m: 2.32, top_m: 4.65 }));
}
