//! End-to-end smoke for the Bystra Valley map behind its opt-in gate: a full 7v7 sets up on
//! the new battlefield, everyone deploys on dry ground, and the authoritative loop ticks.
//! (The map joins the seeded rotation only after the bots learn to respect deep water.)

use net::ClientInputCommand;
use server::{BattleSeed, LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};
use sim::TankCommand;
use terrain::MapId;

#[test]
fn a_7v7_sets_up_and_ticks_on_bystra_valley() {
    let config = RandomBattleConfig::new(BattleSeed::fixed(21), game_core::VehicleKind::T54_1951)
        .on_map(MapId::BystraValley);
    let mut server = LocalAuthoritativeServer::new_random_7v7(ServerTickConfig::default(), config);

    assert_eq!(server.map_id(), MapId::BystraValley);
    let battlefield = map_forge::battlefield(server.map_id());
    let water = battlefield.water.expect("the Bystra is the map");

    // Everyone spawned on dry, real ground.
    let snapshot = server.latest_snapshot();
    assert_eq!(snapshot.tanks.len(), 14);
    for tank in &snapshot.tanks {
        let ground = battlefield
            .heightmap
            .sample_height(tank.position[0], tank.position[2])
            .expect("spawn inside the map");
        assert_eq!(water.depth_over(ground), 0.0, "tank {:?} spawned wet", tank.tank_id);
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
