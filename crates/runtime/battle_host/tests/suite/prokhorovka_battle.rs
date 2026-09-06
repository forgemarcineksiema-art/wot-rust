//! End-to-end smoke for Prokhorovka — the roster's oldest map was also the last WITHOUT a
//! battle test (the ROADMAP gap list said so out loud). A full 7v7 sets up on the steppe,
//! everyone deploys grounded on their own side, and the bots MARCH: the open field with
//! the rail embankment and the Psel reaches must not gridlock a fleet.

use battle_host::{BattleSeed, LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};
use net::ClientInputCommand;
use sim::TankCommand;
use terrain::MapId;

#[test]
fn a_7v7_sets_up_ticks_and_marches_on_prokhorovka() {
    let config = RandomBattleConfig::new(BattleSeed::fixed(21), game_core::VehicleKind::T54_1951)
        .on_map(MapId::ProkhorovkaHill252_2);
    let mut server = LocalAuthoritativeServer::new_random_7v7(ServerTickConfig::default(), config);
    assert_eq!(server.map_id(), MapId::ProkhorovkaHill252_2);
    let battlefield = map_forge::battlefield(server.map_id());

    let snapshot = server.latest_snapshot();
    assert_eq!(snapshot.tanks.len(), 14);
    for tank in &snapshot.tanks {
        battlefield
            .heightmap
            .sample_height(tank.position[0], tank.position[2])
            .expect("spawn inside the map");
        let south = tank.position[2] < 500.0;
        assert_eq!(south, tank.team.0 == 1, "tank {:?} deployed across the map", tank.tank_id);
    }

    let start: Vec<_> = snapshot.tanks.iter().map(|tank| (tank.tank_id, tank.position)).collect();

    // 30 s of battle: the player idles, the bots think and drive.
    let player_tank = server.player_tank();
    for client_tick in 0..1800 {
        server.tick_with_input(ClientInputCommand {
            client_tick,
            tank_id: player_tank,
            command: TankCommand::idle(),
        });
    }

    let after = server.latest_snapshot();
    let displacement = |tank_id, position: [f32; 3]| {
        let (_, origin) = start.iter().find(|(id, _)| *id == tank_id).expect("same fleet");
        ((position[0] - origin[0]).powi(2) + (position[2] - origin[2]).powi(2)).sqrt()
    };
    let marchers = after
        .tanks
        .iter()
        .filter(|tank| tank.tank_id != player_tank)
        .filter(|tank| displacement(tank.tank_id, tank.position) > 40.0)
        .count();
    assert!(
        marchers >= 8,
        "most bots must be on the move after 30 s (only {marchers}/13 left their spawn apron)"
    );
}
