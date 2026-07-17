use game_core::{DamageCause, ModuleSlot, TankSpec, TeamId};
use glam::Vec3;
use sim::{DROWN_DEPTH_M, FixedTimestep, SimulationState, TankCommand};
use terrain::{HeightMap, WaterBody};

const TICK_HZ: u32 = 60;

/// A flat basin flooded 2 m deep: anywhere on it is past [`DROWN_DEPTH_M`].
fn flooded_sim() -> (SimulationState, HeightMap, game_core::TankId) {
    let heightmap = HeightMap::flat(60, 60, 5.0, 0.0).expect("flat map");
    let mut sim = SimulationState::new();
    sim.set_water(Some(WaterBody { surface_level_m: DROWN_DEPTH_M + 0.5 }));
    let id = sim.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(30.0, 0.0, 30.0));
    (sim, heightmap, id)
}

fn tick(sim: &mut SimulationState, heightmap: &HeightMap, id: game_core::TankId, count: u32) {
    let timestep = FixedTimestep::from_hz(TICK_HZ);
    for _ in 0..count {
        sim.apply_commands_on_terrain(&[(id, TankCommand::idle())], timestep, heightmap);
    }
}

#[test]
fn deep_water_floods_the_engine_then_drains_the_hull_to_zero() {
    let (mut sim, heightmap, id) = flooded_sim();

    // Before the flood deadline (2 s) the engine survives.
    tick(&mut sim, &heightmap, id, 110);
    assert!(
        sim.tank(id).unwrap().modules.is_functional(ModuleSlot::Engine),
        "the engine must survive the grace period"
    );

    // Past the deadline it floods (and the event says so).
    tick(&mut sim, &heightmap, id, 20);
    let tank = sim.tank(id).unwrap();
    assert!(
        !tank.modules.is_functional(ModuleSlot::Engine),
        "2 s under the surface must flood the engine"
    );
    assert!(tank.hit_points > 0, "flooding kills the engine first, not the tank");

    // The hull then drains in pulses; ~6 s after entering the water the vehicle is lost, and
    // every pulse is an honest self-inflicted drowning event.
    let mut saw_drowning_pulse = false;
    for _ in 0..280 {
        tick(&mut sim, &heightmap, id, 1);
        saw_drowning_pulse |= sim
            .damage_events()
            .iter()
            .any(|event| event.cause == DamageCause::Drowning && event.damage_hp > 0);
        if sim.tank(id).unwrap().hit_points == 0 {
            break;
        }
    }
    assert_eq!(sim.tank(id).unwrap().hit_points, 0, "the drowned hull must reach zero");
    assert!(saw_drowning_pulse, "hull loss must be reported as DamageCause::Drowning");
}

#[test]
fn leaving_deep_water_before_the_deadline_resets_the_clock() {
    let (mut sim, heightmap, id) = flooded_sim();

    // 1.8 s under — then the hull climbs out (teleported onto a dry shelf for the test).
    tick(&mut sim, &heightmap, id, 108);
    let dry = Vec3::new(500.0, 0.0, 500.0); // outside the 300 m flooded grid = no water sample
    sim.tank_mut(id).unwrap().position = dry;
    tick(&mut sim, &heightmap, id, 10);
    // Back in: the clock must have reset, so another 1.8 s still does not flood it.
    sim.tank_mut(id).unwrap().position = Vec3::new(30.0, 0.0, 30.0);
    tick(&mut sim, &heightmap, id, 108);
    assert!(
        sim.tank(id).unwrap().modules.is_functional(ModuleSlot::Engine),
        "surfacing must reset the submersion clock"
    );

    // But staying under past the deadline floods as usual.
    tick(&mut sim, &heightmap, id, 30);
    assert!(!sim.tank(id).unwrap().modules.is_functional(ModuleSlot::Engine));
}

#[test]
fn a_dry_map_never_drowns_anyone() {
    let heightmap = HeightMap::flat(60, 60, 5.0, 0.0).expect("flat map");
    let mut sim = SimulationState::new(); // water: None
    let id = sim.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(30.0, 0.0, 30.0));
    tick(&mut sim, &heightmap, id, 600);
    let tank = sim.tank(id).unwrap();
    assert!(tank.modules.is_functional(ModuleSlot::Engine));
    assert_eq!(tank.hit_points, tank.spec.hit_points);
}
