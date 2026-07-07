//! The benchmark the FPS complaints were missing: one authoritative tick of a REAL 7v7 —
//! 14 tanks on the full battlefield, 13 bot brains, spotting, shells, ramming, snapshots.
//! The old `sim` bench stepped one tank on a flat void and measured none of the hot paths.
//! Run with `cargo bench -p server` and compare before/after any tick-path change.

use criterion::{Criterion, criterion_group, criterion_main};
use net::ClientInputCommand;
use server::{BattleSeed, LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};
use sim::TankCommand;

fn battle_tick(criterion: &mut Criterion) {
    criterion.bench_function("random_7v7_tick", |bencher| {
        let config =
            RandomBattleConfig::new(BattleSeed::fixed(42), game_core::VehicleKind::T54_1951);
        let mut warmed =
            LocalAuthoritativeServer::new_random_7v7(ServerTickConfig::default(), config);
        let player_tank = warmed.player_tank();
        let drive = |client_tick: u64| ClientInputCommand {
            client_tick,
            tank_id: player_tank,
            command: TankCommand::drive(1.0, 0.0),
        };
        // Warm the battle up: bots pick routes, close distance, and start spotting/firing, so
        // the measured tick carries live combat, not fourteen parked hulls.
        for client_tick in 0..1800 {
            warmed.tick_with_input(drive(client_tick));
        }
        // Measure ticks near the warmed state: criterion would otherwise march one server
        // 50k+ ticks deep, where the battle is decided and every bot idles — a benchmark of
        // nothing. Re-cloning every 120 ticks (clone excluded from timing) keeps every
        // measured tick inside live 14-tank combat.
        bencher.iter_custom(|iterations| {
            let mut total = std::time::Duration::ZERO;
            let mut done = 0_u64;
            while done < iterations {
                let mut server = warmed.clone();
                let chunk = 120.min(iterations - done);
                let start = std::time::Instant::now();
                for offset in 0..chunk {
                    server.tick_with_input(drive(1800 + offset));
                }
                total += start.elapsed();
                done += chunk;
            }
            total
        });
    });
}

criterion_group!(benches, battle_tick);
criterion_main!(benches);
