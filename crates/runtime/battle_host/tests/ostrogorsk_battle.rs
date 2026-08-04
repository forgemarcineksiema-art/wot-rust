//! End-to-end smoke for Ostrogorsk after the road-surface routing (teren A2): streets splat
//! and DRIVE as stone now, so this locks that a full 7v7 still sets up on the city map, the
//! authoritative loop ticks, and the bots march instead of stalling on the new ground rule.

use battle_host::{BattleSeed, LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};
use net::ClientInputCommand;
use sim::TankCommand;
use terrain::MapId;

#[test]
fn a_7v7_sets_up_and_ticks_on_ostrogorsk() {
    let config = RandomBattleConfig::new(BattleSeed::fixed(21), game_core::VehicleKind::T54_1951)
        .on_map(MapId::Ostrogorsk);
    let mut server = LocalAuthoritativeServer::new_random_7v7(ServerTickConfig::default(), config);

    assert_eq!(server.map_id(), MapId::Ostrogorsk);
    let battlefield = map_forge::battlefield(server.map_id());

    let snapshot = server.latest_snapshot();
    assert_eq!(snapshot.tanks.len(), 14);
    for tank in &snapshot.tanks {
        battlefield
            .heightmap
            .sample_height(tank.position[0], tank.position[2])
            .expect("spawn inside the map");
    }

    // One second of authoritative battle without a panic, everyone alive.
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

/// The route brain reads `properties_at`, and A2 changed what streets are made of — this
/// locks that the fleet still MARCHES on the city map: after 30 s most bots have left
/// their spawn apron instead of grinding on the new ground rule.
#[test]
fn the_bots_march_on_the_city_instead_of_stalling_on_stone() {
    let config = RandomBattleConfig::new(BattleSeed::fixed(21), game_core::VehicleKind::T54_1951)
        .on_map(MapId::Ostrogorsk);
    let mut server = LocalAuthoritativeServer::new_random_7v7(ServerTickConfig::default(), config);

    let start: Vec<_> =
        server.latest_snapshot().tanks.iter().map(|tank| (tank.tank_id, tank.position)).collect();

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
