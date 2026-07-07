//! The combat hot-path benchmark: a full 14-tank battle with live shells and cover, stepped
//! through the same `apply_commands_on_battlefield` the server runs. This is the measurement
//! behind the destruction-program budget (`docs/destruction-program.md`) — new per-tick combat
//! work (cover damage, detached turrets, richer trace shapes) must show its cost here.

use std::f32::consts::PI;

use criterion::{Criterion, criterion_group, criterion_main};
use game_core::{TeamId, VehicleKind};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};
use terrain::{HeightMap, StaticCoverKind, StaticCoverObject};

/// A farm row between the battle lines, so shell segments and driving hulls both pay the
/// cover-test cost every tick — the shape of a real map, not an empty plain.
fn battle_cover() -> Vec<StaticCoverObject> {
    (0..4)
        .map(|index| StaticCoverObject {
            id: format!("barn_{index}"),
            name: format!("barn {index}"),
            kind: StaticCoverKind::FarmBuilding,
            center: [60.0 + index as f32 * 55.0, 2.5, 160.0],
            half_extents_m: [8.0, 2.5, 5.0],
        })
        .collect()
}

fn combat_hot_path_benchmark(c: &mut Criterion) {
    c.bench_function("combat_128_ticks_14_tank_battle", |b| {
        b.iter(|| {
            let heightmap = HeightMap::flat(65, 65, 5.0, 0.0).expect("flat battlefield");
            let cover = battle_cover();
            let mut state = SimulationState::new();
            let mut commands = Vec::new();
            for index in 0..7 {
                let x = 40.0 + index as f32 * 35.0;
                let attacker = state.spawn_tank_with_yaw(
                    TeamId(1),
                    VehicleKind::T54_1951.spec(),
                    Vec3::new(x, 0.0, 60.0),
                    0.0,
                );
                let defender = state.spawn_tank_with_yaw(
                    TeamId(2),
                    VehicleKind::T55A.spec(),
                    Vec3::new(x, 0.0, 260.0),
                    PI,
                );
                // Everyone drives, slews, and holds the trigger: shells launch on tick 0 (fresh
                // reload) and stay in flight across the run, so the bench pays movement, SAT,
                // shell stepping, armor-volume traces, ramming, and spotting together.
                commands.push((
                    attacker,
                    TankCommand { fire: true, ..TankCommand::drive_with_turret(1.0, 0.05, 0.1) },
                ));
                commands.push((
                    defender,
                    TankCommand { fire: true, ..TankCommand::drive_with_turret(0.6, -0.05, -0.1) },
                ));
            }
            let timestep = FixedTimestep::from_hz(60);
            for _ in 0..128 {
                state.apply_commands_on_battlefield(&commands, timestep, &heightmap, &cover);
            }
            state.tick()
        });
    });
}

criterion_group!(benches, combat_hot_path_benchmark);
criterion_main!(benches);
