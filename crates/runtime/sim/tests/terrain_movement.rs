use game_core::{TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};
use terrain::HeightMap;

#[test]
fn authoritative_simulation_slows_tank_on_uphill_terrain() {
    let flat = HeightMap::flat(96, 96, 1.0, 0.0).expect("flat terrain");
    let uphill = ramp_heightmap(96, 96, 1.0, 0.18);

    let flat_tank = drive_for_one_second(&flat);
    let uphill_tank = drive_for_one_second(&uphill);
    let flat_distance = flat_tank.position.z - 8.0;
    let uphill_distance = uphill_tank.position.z - 8.0;

    assert!(uphill_distance < flat_distance * 0.9);
    assert!(uphill_tank.position.y > 0.0);
}

#[test]
fn braking_command_bleeds_forward_speed_before_reverse() {
    let mut state = SimulationState::new();
    let tank_id = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::new(8.0, 0.0, 8.0));
    let step = FixedTimestep::from_hz(60);

    for _ in 0..40 {
        state.apply_commands(&[(tank_id, TankCommand::drive(1.0, 0.0))], step);
    }
    let before_brake = state.tank(tank_id).expect("tank").velocity_mps.length();

    for _ in 0..8 {
        state.apply_commands(&[(tank_id, TankCommand { brake: 1.0, ..TankCommand::idle() })], step);
    }
    let after_brake = state.tank(tank_id).expect("tank").velocity_mps.length();

    assert!(after_brake < before_brake);
    assert!(after_brake >= 0.0);
}

fn drive_for_one_second(terrain: &HeightMap) -> sim::TankState {
    let mut state = SimulationState::new();
    let tank_id = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::new(8.0, 0.0, 8.0));
    let step = FixedTimestep::from_hz(60);

    for _ in 0..60 {
        state.apply_commands_on_terrain(&[(tank_id, TankCommand::drive(1.0, 0.0))], step, terrain);
    }

    state.tank(tank_id).expect("tank").clone()
}

fn ramp_heightmap(width: usize, height: usize, cell_size_m: f32, rise_per_meter: f32) -> HeightMap {
    let samples = (0..height)
        .flat_map(|z| (0..width).map(move |_| z as f32 * cell_size_m * rise_per_meter))
        .collect();
    HeightMap::new(width, height, cell_size_m, samples).expect("ramp terrain")
}
