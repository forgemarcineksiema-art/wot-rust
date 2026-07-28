//! Collapsed masonry is GROUND, and it behaves like the ground it is.
//!
//! The support envelope reads the debris surface, so a mound raises the hull, tilts it, and slows
//! it — no rule is written for rubble anywhere in the drive model; it is simply terrain that was
//! not in the heightmap.
//!
//! Worth recording, because the design that motivated this expected otherwise: the repose grade
//! (0.78) sits above the momentum-climb ceiling (0.68), so the FLANK is steeper than anything a
//! tank climbs — and yet every mound the shipping `rubble_height_frac` values produce is still
//! crossable. The reason is the whole point of the support envelope: a barn mound is 2.4 m tall
//! over a 3.1 m talus, and a T-54's running gear is 4.4 m between end stations. The rigid beam
//! BRIDGES a flank shorter than its own wheelbase, exactly as it bridges a trench narrower than
//! its wheel pitch. A pile smaller than the tank is something a tank drives over. So there is no
//! angle-of-attack gate on rubble; there is a real cost in tilt and speed, which is what these
//! tests lock.

use game_core::{ContactFootprint, TankSpec, VehicleKind};
use glam::Vec3;
use physics::{
    TankControlInput, TankControllerSettings, TankFootprint, TankKinematicState,
    TankWorldObstacles, step_tank_on_world_with_tanks, support_height,
};
use terrain::{HeightMap, RubbleMound, StaticCoverKind, StaticCoverObject};

const DT: f32 = 1.0 / 60.0;
const DRIVE: TankControlInput = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };

fn flat_ground() -> HeightMap {
    HeightMap::flat(121, 121, 1.0, 0.0).expect("flat test terrain")
}

/// The mound a knocked-down farm building leaves: 16 x 12 m in plan, 6 m tall before it came
/// down, so 2.4 m of debris (`rubble_height_frac` 0.4) with flanks ~3 m deep.
fn barn_rubble() -> RubbleMound {
    RubbleMound::from_cover(&StaticCoverObject {
        id: "barn".into(),
        name: "barn".into(),
        kind: StaticCoverKind::FarmBuilding,
        center: [60.0, 3.0, 60.0],
        half_extents_m: [8.0, 3.0, 6.0],
    })
}

/// Drive a hull from `start` on the given heading and report the HIGHEST it ever got, plus the
/// steepest nose-up attitude it reached. The peak is the honest measure of "did it climb": a hull
/// that scales the pile also drives off the far side, so the final pose says nothing.
fn climb(start: Vec3, yaw_rad: f32, rubble: &[RubbleMound], ticks: usize) -> (f32, f32) {
    let spec = TankSpec::t54_1951();
    let settings = TankControllerSettings::from_spec(&spec);
    let footprint = ContactFootprint::for_vehicle(VehicleKind::T54_1951);
    let hull = TankFootprint::from_hitbox(spec.hitbox);
    let map = flat_ground();
    let mut state = TankKinematicState { position: start, yaw_rad, ..Default::default() };
    let (mut peak_y, mut steepest_pitch) = (f32::MIN, 0.0_f32);
    for _ in 0..ticks {
        step_tank_on_world_with_tanks(
            &mut state,
            DRIVE,
            &settings,
            Some(&map),
            // No cover: this is the world as it stands once rubble stops blocking movement. The
            // debris is the ONLY thing between the hull and flat ground.
            TankWorldObstacles::new(&[], hull, &[]).with_rubble(rubble),
            Some(&footprint),
            DT,
        );
        peak_y = peak_y.max(state.position.y);
        steepest_pitch = steepest_pitch.max(state.pitch_rad);
    }
    (peak_y, steepest_pitch)
}

/// Ground covered along the heading over `ticks`, for comparing a run across debris with the
/// same run over bare dirt.
fn distance_covered(start: Vec3, yaw_rad: f32, rubble: &[RubbleMound], ticks: usize) -> f32 {
    let spec = TankSpec::t54_1951();
    let settings = TankControllerSettings::from_spec(&spec);
    let footprint = ContactFootprint::for_vehicle(VehicleKind::T54_1951);
    let hull = TankFootprint::from_hitbox(spec.hitbox);
    let map = flat_ground();
    let mut state = TankKinematicState { position: start, yaw_rad, ..Default::default() };
    for _ in 0..ticks {
        step_tank_on_world_with_tanks(
            &mut state,
            DRIVE,
            &settings,
            Some(&map),
            TankWorldObstacles::new(&[], hull, &[]).with_rubble(rubble),
            Some(&footprint),
            DT,
        );
    }
    (state.position.x - start.x).hypot(state.position.z - start.z)
}

#[test]
fn the_support_envelope_rests_on_the_debris_instead_of_the_buried_ground() {
    let map = flat_ground();
    let mound = barn_rubble();
    let footprint = ContactFootprint::for_vehicle(VehicleKind::T54_1951);

    let on_crest = support_height(&map, Vec3::new(60.0, 0.0, 60.0), 0.0, &footprint, &[mound])
        .expect("stations on the map");
    assert!(
        (on_crest - mound.crest_y_m).abs() < 0.05,
        "a hull over the crown rides the debris at {}, not the buried ground",
        on_crest
    );

    // Clear of the footprint the debris is simply not there.
    let beside = support_height(&map, Vec3::new(20.0, 0.0, 60.0), 0.0, &footprint, &[mound])
        .expect("stations on the map");
    assert!(beside.abs() < 1.0e-4, "off the pile the ground is the ground, got {beside}");

    // And an EMPTY rubble slice must read exactly like the terrain — this is what keeps every
    // battlefield that has not been knocked down yet bit-identical.
    let untouched = support_height(&map, Vec3::new(60.0, 0.0, 60.0), 0.0, &footprint, &[]);
    assert_eq!(untouched, Some(0.0));
}

/// THE feature: a hull drives onto a collapsed building and stands on top of it. Before this the
/// mound was an infinitely tall prism for movement, so a flattened block blocked a hull exactly as
/// the standing block had — "destruction opens the map" held for fire and sight but never for
/// manoeuvre.
#[test]
fn a_hull_drives_up_onto_the_debris_and_tops_it() {
    let mound = barn_rubble();
    let (peak_y, _) = climb(Vec3::new(60.0, 0.0, 40.0), 0.0, &[mound], 300);
    assert!(
        peak_y > mound.crest_y_m * 0.9,
        "the hull must top the pile: peaked at y {peak_y} against a crest of {}",
        mound.crest_y_m
    );
}

/// ...and the crossing is not free. The debris is a slope like any other: it pitches the hull up
/// on the way in, which both costs speed through the force model and puts the belly in the air.
#[test]
fn crossing_the_debris_tilts_the_hull_and_costs_it_speed() {
    let mound = barn_rubble();
    let start = Vec3::new(60.0, 0.0, 40.0);
    let (_, steepest) = climb(start, 0.0, &[mound], 300);
    assert!(steepest > 0.15, "the debris must pitch the hull up, peaked at {steepest} rad");

    // Same run, same ticks, over bare ground: the pile has to cost real distance.
    let over_rubble = distance_covered(start, 0.0, &[mound], 300);
    let over_ground = distance_covered(start, 0.0, &[], 300);
    assert!(
        over_rubble < over_ground - 2.0,
        "crossing the pile must cost ground: {over_rubble:.1} m over debris vs          {over_ground:.1} m over dirt"
    );
}

/// The repose grade is a real wall — for a pile big enough to present it. A flank LONGER than the
/// running gear cannot be bridged, so the hull meets 0.78 head on, above the momentum-climb
/// ceiling, and digs its nose in. No shipping `rubble_height_frac` builds a pile this big; this
/// locks that the mechanism is there if a map ever authors one.
#[test]
fn a_flank_longer_than_the_running_gear_is_a_wall() {
    // 24 m tall before it fell: 9.6 m of debris over a 12.3 m talus, against a 4.4 m wheelbase.
    let tower = RubbleMound::from_cover(&StaticCoverObject {
        id: "tower".into(),
        name: "tower".into(),
        kind: StaticCoverKind::FarmBuilding,
        center: [60.0, 12.0, 60.0],
        half_extents_m: [20.0, 12.0, 20.0],
    });
    let (peak_y, _) = climb(Vec3::new(60.0, 0.0, 20.0), 0.0, &[tower], 600);
    assert!(
        peak_y < tower.crest_y_m * 0.5,
        "a flank the gear cannot bridge must wall a straight charge: peaked at y {peak_y}          against a crest of {}",
        tower.crest_y_m
    );
}
