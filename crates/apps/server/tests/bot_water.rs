//! The lock that put Bystra into the rotation: a full seeded 7v7 on the valley, minutes of
//! real ticks, and no bot ever drives itself into the drowning channel — while the teams
//! still reach the far bank, so the crossings are used, not avoided by hugging the spawn.

use net::ClientInputCommand;
use server::{BattleSeed, LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};
use sim::{DROWN_DEPTH_M, TankCommand};
use terrain::MapId;

/// Three minutes of battle at 60 Hz.
const SOAK_TICKS: u64 = 10_800;

fn depth_at(map: &terrain::BattlefieldMap, x: f32, z: f32) -> f32 {
    match (map.water, map.heightmap.sample_height(x, z)) {
        (Some(water), Some(ground)) => water.depth_over(ground).max(0.0),
        _ => 0.0,
    }
}

#[test]
fn a_full_bystra_battle_drowns_no_bot_and_still_crosses_the_river() {
    let battlefield = map_forge::battlefield(MapId::BystraValley);
    let river_x = terrain::bystra_river_center_x(500.0);
    let east_bank_x = river_x + terrain::RIVER_CORRIDOR_HALF_WIDTH_M + 20.0;
    for seed in [5_u64, 23] {
        let mut server = LocalAuthoritativeServer::new_random_7v7(
            ServerTickConfig::default(),
            RandomBattleConfig::new(BattleSeed::fixed(seed), game_core::VehicleKind::default()),
        );
        let player = server.player_tank();
        let mut deepest = 0.0_f32;
        let mut crossed_east = false;
        for tick in 0..SOAK_TICKS {
            server.tick_with_input(ClientInputCommand {
                client_tick: tick,
                tank_id: player,
                command: TankCommand::idle(),
            });
            let snapshot = server.current_snapshot();
            for tank in &snapshot.tanks {
                if tank.hit_points == 0 {
                    continue;
                }
                let depth = depth_at(&battlefield, tank.position[0], tank.position[2]);
                deepest = deepest.max(depth);
                assert!(
                    depth < DROWN_DEPTH_M,
                    "seed {seed} tick {tick}: tank {:?} sits {depth:.2} m deep at {:?} — \
                     the route brain drove it into the channel",
                    tank.tank_id,
                    tank.position
                );
                if tank.position[0] > east_bank_x {
                    crossed_east = true;
                }
            }
        }
        assert!(
            crossed_east,
            "seed {seed}: nobody ever reached the east-bank shelf beyond the river corridor"
        );
        // The deepest wade should look like a ford crossing (momentum can carry a hull a
        // shade past the route brain's 1.2 m line before the escape reverse bites), never a
        // near-drowning.
        assert!(
            deepest <= 1.35,
            "seed {seed}: deepest wade {deepest:.2} m — bots flirt with the drowning line"
        );
    }
}
