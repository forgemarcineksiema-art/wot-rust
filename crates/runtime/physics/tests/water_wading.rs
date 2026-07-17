use game_core::TankSpec;
use glam::Vec3;
use physics::water::{FORD_MAX_DEPTH_M, WADE_DRAG_START_M};
use physics::{
    TankControlInput, TankControllerSettings, TankFootprint, TankKinematicState,
    TankWorldObstacles, step_tank_on_world_with_tanks,
};
use terrain::{HeightMap, WaterBody};

const DT: f32 = 1.0 / 60.0;

/// Drive full throttle straight ahead for `seconds` over flat ground flooded to `water`.
fn run(water: Option<WaterBody>, seconds: f32) -> TankKinematicState {
    let heightmap = HeightMap::flat(60, 60, 5.0, 0.0).expect("flat map");
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let input = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    let mut state =
        TankKinematicState { position: Vec3::new(30.0, 0.0, 20.0), ..Default::default() };
    let footprint = TankFootprint::from_hitbox(TankSpec::t54_1951().hitbox);
    let obstacles = TankWorldObstacles::new(&[], footprint, &[]).with_water(water);
    for _ in 0..(seconds / DT) as u32 {
        step_tank_on_world_with_tanks(
            &mut state,
            input,
            &settings,
            Some(&heightmap),
            obstacles,
            None,
            DT,
        );
    }
    state
}

#[test]
fn fording_water_is_measurably_slower_than_dry_ground() {
    let dry = run(None, 6.0);
    let ford = run(Some(WaterBody { surface_level_m: FORD_MAX_DEPTH_M }), 6.0);

    let dry_distance = dry.position.z - 20.0;
    let ford_distance = ford.position.z - 20.0;
    assert!(dry_distance > 10.0, "the dry run must actually drive, got {dry_distance} m");
    assert!(
        ford_distance < dry_distance * 0.8,
        "a {FORD_MAX_DEPTH_M} m ford must cost at least 20% of the dry distance: \
         {ford_distance} vs {dry_distance} m"
    );
    assert!(ford_distance > 2.0, "a ford is slow, not impassable, got {ford_distance} m");
}

#[test]
fn splash_depth_water_costs_nothing() {
    // At or below the wade-start depth the water is presentation only.
    let dry = run(None, 4.0);
    let splash = run(Some(WaterBody { surface_level_m: WADE_DRAG_START_M }), 4.0);
    assert_eq!(dry, splash, "splash-depth water must not change the drive at all");
}

#[test]
fn a_water_table_below_the_ground_is_bit_identical_to_no_water() {
    // The determinism proof behind the replay lock: depth 0 collapses every wading formula to
    // the exact dry value, so a dry map with a (pointless) water body drives bit-identically.
    let none = run(None, 5.0);
    let underground = run(Some(WaterBody { surface_level_m: -2.0 }), 5.0);
    assert_eq!(none, underground);
}
