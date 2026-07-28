//! Throwaway measurement: how often do hulls actually touch in a real 7v7?
use net::ClientInputCommand;
use server::{BattleSeed, LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};
use sim::TankCommand;
use terrain::MapId;

const TICKS: u64 = 10_800; // three minutes at 60 Hz

#[test]
fn census() {
    for (name, map) in [("bystra", MapId::BystraValley), ("ostrogorsk", MapId::Ostrogorsk)] {
        let _ = map_forge::battlefield(map);
        for seed in [5_u64, 23, 71] {
            let mut server = LocalAuthoritativeServer::new_random_7v7(
                ServerTickConfig::default(),
                RandomBattleConfig::new(BattleSeed::fixed(seed), game_core::VehicleKind::default()),
            );
            let player = server.player_tank();
            let mut touching_ticks = 0u64;
            let mut episodes = 0u64;
            let mut was_touching = false;
            let mut hardest: f32 = 0.0;
            let mut prev: Vec<(u64, [f32; 3])> = Vec::new();
            for tick in 0..TICKS {
                server.tick_with_input(ClientInputCommand {
                    client_tick: tick,
                    tank_id: player,
                    command: TankCommand::idle(),
                });
                let snapshot = server.current_snapshot();
                let live: Vec<_> = snapshot.tanks.iter().filter(|t| t.hit_points > 0).collect();
                let mut touching = false;
                for i in 0..live.len() {
                    for j in i + 1..live.len() {
                        let (a, b) = (live[i], live[j]);
                        let dx = a.position[0] - b.position[0];
                        let dz = a.position[2] - b.position[2];
                        // Generous: hull half-length 3.2 each + skin.
                        if (dx * dx + dz * dz).sqrt() < 6.6 {
                            touching = true;
                            // Closing speed from the previous snapshot positions.
                            if let (Some(pa), Some(pb)) = (
                                prev.iter().find(|(id, _)| *id == a.tank_id.0),
                                prev.iter().find(|(id, _)| *id == b.tank_id.0),
                            ) {
                                let va = [
                                    (a.position[0] - pa.1[0]) * 60.0,
                                    (a.position[2] - pa.1[2]) * 60.0,
                                ];
                                let vb = [
                                    (b.position[0] - pb.1[0]) * 60.0,
                                    (b.position[2] - pb.1[2]) * 60.0,
                                ];
                                let len = (dx * dx + dz * dz).sqrt().max(0.01);
                                let closing = ((vb[0] - va[0]) * -dx + (vb[1] - va[1]) * -dz) / len;
                                hardest = hardest.max(closing);
                            }
                        }
                    }
                }
                prev = snapshot.tanks.iter().map(|t| (t.tank_id.0, t.position)).collect();
                if touching {
                    touching_ticks += 1;
                    if !was_touching {
                        episodes += 1;
                    }
                }
                was_touching = touching;
            }
            println!(
                "{name} seed {seed}: contact on {touching_ticks}/{TICKS} ticks ({:.1}%), \
                 {episodes} separate episodes, hardest closing {hardest:.1} m/s",
                touching_ticks as f32 / TICKS as f32 * 100.0
            );
        }
    }
}
