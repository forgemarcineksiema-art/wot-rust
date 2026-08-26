//! End-to-end smoke for Mazurski Przesmyk behind its opt-in gate — the first battle test
//! on a standing-sheet map: a full 7v7 sets up between the lakes, everyone deploys on
//! real dry ground on their own side, the bots MARCH, and after a minute of battle nobody
//! is below the drowning line. The lakes deny; the brain refuses; the causeway carries.
//! (The map plays via `WOT_MAP=mazurski-przesmyk`; no rotation yet.)

use battle_host::{BattleSeed, LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};
use net::ClientInputCommand;
use sim::TankCommand;
use terrain::MapId;

fn server_on_mazurski() -> LocalAuthoritativeServer {
    let config = RandomBattleConfig::new(BattleSeed::fixed(21), game_core::VehicleKind::T54_1951)
        .on_map(MapId::MazurskiPrzesmyk);
    LocalAuthoritativeServer::new_random_7v7(ServerTickConfig::default(), config)
}

#[test]
fn a_7v7_sets_up_and_ticks_on_mazurski_przesmyk() {
    let mut server = server_on_mazurski();
    assert_eq!(server.map_id(), MapId::MazurskiPrzesmyk);
    let battlefield = map_forge::battlefield(server.map_id());
    assert!(
        !battlefield.standing_water.is_empty(),
        "the defile is the first map whose water is standing sheets"
    );

    // Everyone spawned on real DRY ground on their own diagonal end (team 1 south-west).
    let field = battlefield.water_field();
    let snapshot = server.latest_snapshot();
    assert_eq!(snapshot.tanks.len(), 14);
    for tank in &snapshot.tanks {
        let ground = battlefield
            .heightmap
            .sample_height(tank.position[0], tank.position[2])
            .expect("spawn inside the map");
        assert_eq!(
            field.depth_at(ground, tank.position[0], tank.position[2]),
            0.0,
            "tank {:?} deployed in water at ({:.0}, {:.0})",
            tank.tank_id,
            tank.position[0],
            tank.position[2]
        );
        let south = tank.position[2] < 500.0;
        assert_eq!(south, tank.team.0 == 1, "tank {:?} deployed across the map", tank.tank_id);
    }

    // One second of authoritative battle without a panic.
    let player_tank = server.player_tank();
    for client_tick in 0..60 {
        server.tick_with_input(ClientInputCommand {
            client_tick,
            tank_id: player_tank,
            command: TankCommand::idle(),
        });
    }
    assert_eq!(server.latest_snapshot().tanks.len(), 14);
}

/// The march AND the water promise in one soak: after a minute of battle most of the
/// fleet has left its spawn apron (the lakes and the pinch do not gridlock the map), and
/// NOBODY — living or wrecked — sits below the drowning line. A drowned hull would lie in
/// its lake forever; this is the assert that catches it, whatever killed it first.
#[test]
fn the_bots_march_between_the_lakes_and_nobody_drowns() {
    let mut server = server_on_mazurski();
    let battlefield = map_forge::battlefield(server.map_id());
    let field = battlefield.water_field();
    let drown = sim::DROWN_DEPTH_M;

    let start: Vec<_> =
        server.latest_snapshot().tanks.iter().map(|tank| (tank.tank_id, tank.position)).collect();

    // 60 s of battle: the player idles, the bots think, drive and fight.
    let player_tank = server.player_tank();
    for client_tick in 0..3600 {
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
        "most bots must be on the move after 60 s (only {marchers}/13 left their spawn apron)"
    );

    for tank in &after.tanks {
        let Some(ground) = battlefield.heightmap.sample_height(tank.position[0], tank.position[2])
        else {
            panic!("tank {:?} left the map at {:?}", tank.tank_id, tank.position);
        };
        let depth = field.depth_at(ground, tank.position[0], tank.position[2]);
        assert!(
            depth < drown,
            "tank {:?} (hp {}) sits in {depth:.2} m of water at ({:.0}, {:.0}) - the lakes \
             must DENY, and the route brain must refuse them",
            tank.tank_id,
            tank.hit_points,
            tank.position[0],
            tank.position[2]
        );
    }
}
