//! Locking tests for the hull's vertical dynamics: driving off a cliff is a multi-tick ballistic
//! arc (never a teleport), drivable downhills stay glued, and an airborne hull is deaf to the
//! driver until the terrain catches it.

use game_core::TankSpec;
use glam::Vec3;
use physics::{
    GroundStep, TankControlInput, TankControllerSettings, TankKinematicState,
    step_tank_on_heightmap,
};
use terrain::HeightMap;

const DT: f32 = 1.0 / 60.0;
const FULL_THROTTLE: TankControlInput = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };

/// 41x41 @ 1 m map from a height function (z is the driving axis for yaw 0).
fn map_from(height_at: impl Fn(f32, f32) -> f32) -> HeightMap {
    let mut samples = Vec::with_capacity(41 * 41);
    for z in 0..41 {
        for x in 0..41 {
            samples.push(height_at(x as f32, z as f32));
        }
    }
    HeightMap::new(41, 41, 1.0, samples).expect("test heightmap dimensions are fixed")
}

/// A 6 m plateau ending in a sheer face at z = 20, then flat ground.
fn cliff_map() -> HeightMap {
    map_from(|_, z| if z < 20.0 { 6.0 } else { 0.0 })
}

fn tank_on(map: &HeightMap, z: f32) -> TankKinematicState {
    let y = map.sample_height(20.0, z).expect("start position is on the map");
    TankKinematicState { position: Vec3::new(20.0, y, z), ..Default::default() }
}

/// Drive until the hull first leaves the ground; panics if it never does.
fn drive_to_takeoff(
    state: &mut TankKinematicState,
    settings: &TankControllerSettings,
    map: &HeightMap,
) -> GroundStep {
    for _ in 0..1200 {
        let step = step_tank_on_heightmap(state, FULL_THROTTLE, settings, map, DT);
        if !step.grounded {
            return step;
        }
    }
    panic!("the hull must leave the ground at the cliff edge");
}

#[test]
fn driving_off_a_cliff_descends_ballistically_not_teleporting() {
    let settings = TankControllerSettings::from_spec(&TankSpec::medium_test_tank());
    let map = cliff_map();
    let mut state = tank_on(&map, 10.0);

    drive_to_takeoff(&mut state, &settings, &map);
    let mut airborne_ticks = 1;
    let mut last_fall = 0.0;
    let landing = loop {
        let before = state.position.y;
        let step = step_tank_on_heightmap(&mut state, FULL_THROTTLE, &settings, &map, DT);
        if step.grounded {
            break step;
        }
        airborne_ticks += 1;
        let fall = before - state.position.y;
        assert!(fall < 0.5, "no teleports: a single tick fell {fall} m");
        assert!(fall > last_fall, "gravity must accelerate the fall");
        last_fall = fall;
        assert!(airborne_ticks < 1200, "the terrain must catch the hull");
    };

    // A 6 m drop takes ~1 s of flight at g = 12 and lands at sqrt(2 g h) ~ 12 m/s.
    assert!(airborne_ticks > 30, "6 m of flight lasts many ticks, got {airborne_ticks}");
    assert!(landing.landing_impact_mps > 8.0, "impact {}", landing.landing_impact_mps);
    let ground = map.sample_height(state.position.x, state.position.z).expect("landed on map");
    assert!((state.position.y - ground).abs() < 1.0e-4, "landing snaps onto the terrain");
}

#[test]
fn a_drivable_downhill_keeps_the_hull_grounded_every_tick() {
    let settings = TankControllerSettings::from_spec(&TankSpec::medium_test_tank());
    // Grade 0.3 downhill along +z: well inside gradeability, must read as ground follow.
    let map = map_from(|_, z| 14.0 - 0.3 * z);
    let mut state = tank_on(&map, 2.0);

    for tick in 0..600 {
        let step = step_tank_on_heightmap(&mut state, FULL_THROTTLE, &settings, &map, DT);
        assert!(step.grounded, "tick {tick}: a drivable slope never unsticks the tracks");
        if state.position.z > 36.0 {
            return;
        }
    }
}

#[test]
fn an_airborne_hull_is_deaf_to_throttle_steer_and_brake() {
    let settings = TankControllerSettings::from_spec(&TankSpec::medium_test_tank());
    let map = cliff_map();
    let mut state = tank_on(&map, 10.0);

    drive_to_takeoff(&mut state, &settings, &map);
    let velocity_xz = (state.velocity.x, state.velocity.z);
    let yaw_rate = state.yaw_rate_rad_s;
    // Mid-air the driver mashes steer and brake: nothing may change but the fall.
    let wild = TankControlInput { throttle: -1.0, steer: 1.0, brake: 1.0 };
    for _ in 0..10 {
        let step = step_tank_on_heightmap(&mut state, wild, &settings, &map, DT);
        if step.grounded {
            break;
        }
        assert_eq!((state.velocity.x, state.velocity.z), velocity_xz);
        assert_eq!(state.yaw_rate_rad_s, yaw_rate);
    }
}
