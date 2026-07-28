//! The contact solver's worst case: fourteen hulls crushed into one point, all driving into each
//! other for 128 ticks.
//!
//! `combat_hot_path` cannot see this. Its two teams spread across 200 m of field and meet a few at
//! a time, so the pair loop's broadphase throws away almost every pair and the solve is nearly
//! free. A chokepoint is the opposite shape — every hull near every other hull, every pair
//! surviving to the full four-axis SAT, four solver iterations deep, twice a tick (the impulse
//! pass and the separation pass). That is the shape a 7v7 actually produces at a bridge, a ford or
//! a city street, so it is the shape the budget has to be proven against.

use criterion::{Criterion, criterion_group, criterion_main};
use game_core::{TeamId, VehicleKind};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};
use terrain::HeightMap;

/// Fourteen hulls inside a 20 m square, every one of them driving at the middle.
fn run_pileup() -> u64 {
    let heightmap = HeightMap::flat(33, 33, 5.0, 0.0).expect("flat battlefield");
    let mut state = SimulationState::new();
    let mut commands = Vec::new();
    let centre = Vec3::new(80.0, 0.0, 80.0);
    for index in 0..14 {
        let angle = index as f32 / 14.0 * std::f32::consts::TAU;
        let position = centre + Vec3::new(angle.sin(), 0.0, angle.cos()) * 10.0;
        // Facing the middle, so everyone converges and nobody can leave.
        let yaw = angle + std::f32::consts::PI;
        let team = if index % 2 == 0 { TeamId(1) } else { TeamId(2) };
        let tank = state.spawn_tank_with_yaw(team, VehicleKind::T54_1951.spec(), position, yaw);
        commands.push((tank, TankCommand::drive(1.0, 0.0)));
    }
    let timestep = FixedTimestep::from_hz(60);
    for _ in 0..128 {
        state.apply_commands_on_battlefield(&commands, timestep, &heightmap, &[]);
    }
    state.tick()
}

fn contact_pileup_benchmark(c: &mut Criterion) {
    c.bench_function("contact_128_ticks_14_tank_pileup", |b| b.iter(run_pileup));
}

criterion_group!(benches, contact_pileup_benchmark);
criterion_main!(benches);
