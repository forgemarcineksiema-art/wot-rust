use criterion::{Criterion, criterion_group, criterion_main};
use game_core::{TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

fn fixed_tick_benchmark(c: &mut Criterion) {
    c.bench_function("sim_128_fixed_ticks_one_tank", |b| {
        b.iter(|| {
            let mut state = SimulationState::new();
            let tank = state.spawn_tank(TeamId(1), TankSpec::medium_test_tank(), Vec3::ZERO);
            let timestep = FixedTimestep::from_hz(20);
            for _ in 0..128 {
                state.apply_commands(
                    &[(tank, TankCommand::drive_with_turret(1.0, 0.1, 0.2))],
                    timestep,
                );
            }
            state.tick()
        });
    });
}

criterion_group!(benches, fixed_tick_benchmark);
criterion_main!(benches);
