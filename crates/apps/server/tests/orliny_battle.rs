//! End-to-end smoke for Orliny Pereval behind its opt-in gate: a full 7v7 sets up on the
//! mountain pass, everyone deploys on real ground on their own side of the wall, and the
//! authoritative loop ticks. (The map plays via `WOT_MAP=orliny-pereval`; no rotation yet.)

use net::ClientInputCommand;
use server::{BattleSeed, LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};
use sim::TankCommand;
use terrain::MapId;

#[test]
fn a_7v7_sets_up_and_ticks_on_orliny_pereval() {
    let config = RandomBattleConfig::new(BattleSeed::fixed(21), game_core::VehicleKind::T54_1951)
        .on_map(MapId::OrlinyPereval);
    let mut server = LocalAuthoritativeServer::new_random_7v7(ServerTickConfig::default(), config);

    assert_eq!(server.map_id(), MapId::OrlinyPereval);
    let battlefield = map_forge::battlefield(server.map_id());
    assert!(battlefield.water.is_none(), "the pass is a dry map by design");

    // Everyone spawned on real ground, on their own side of the wall (the massif crest is
    // the axis at z = 500 — nobody deploys past it).
    let snapshot = server.latest_snapshot();
    assert_eq!(snapshot.tanks.len(), 14);
    for tank in &snapshot.tanks {
        battlefield
            .heightmap
            .sample_height(tank.position[0], tank.position[2])
            .expect("spawn inside the map");
        let south = tank.position[2] < 500.0;
        assert_eq!(
            south,
            tank.team.0 == 1,
            "tank {:?} of team {:?} deployed across the wall at z {:.0}",
            tank.tank_id,
            tank.team,
            tank.position[2]
        );
    }

    // The authoritative loop runs (bots think, shells fly, spotting recomputes) — one second
    // of battle without a panic and with everyone still alive at their spawns.
    let player_tank = server.player_tank();
    for client_tick in 0..60 {
        server.tick_with_input(ClientInputCommand {
            client_tick,
            tank_id: player_tank,
            command: TankCommand::idle(),
        });
    }
    let after = server.latest_snapshot();
    assert_eq!(after.tanks.len(), 14);
    assert!(after.tanks.iter().all(|tank| tank.hit_points > 0));
}
