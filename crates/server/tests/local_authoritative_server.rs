use net::ClientInputCommand;
use server::{LocalAuthoritativeServer, ServerTickConfig};
use sim::TankCommand;

#[test]
fn local_server_can_start_with_selected_player_vehicle() {
    let server = LocalAuthoritativeServer::new_with_player_vehicle(
        ServerTickConfig::new(60, 20),
        game_core::VehicleKind::TigerII,
    );
    let snapshot = server.latest_snapshot();
    let player = snapshot
        .tanks
        .iter()
        .find(|tank| tank.tank_id == server.player_tank())
        .expect("player tank");

    assert_eq!(player.vehicle, game_core::VehicleKind::TigerII);
    assert_eq!(player.hit_points, game_core::VehicleKind::TigerII.spec().hit_points);
}

#[test]
fn local_server_runtime_vehicle_change_reassigns_player_tank_id() {
    let mut server = LocalAuthoritativeServer::new(ServerTickConfig::new(60, 20));
    let old_player = server.player_tank();

    let snapshot = server.change_player_vehicle(game_core::VehicleKind::Jagdtiger);
    let new_player = server.player_tank();

    assert_ne!(old_player, new_player);
    assert!(snapshot.tanks.iter().all(|tank| tank.tank_id != old_player));
    let player =
        snapshot.tanks.iter().find(|tank| tank.tank_id == new_player).expect("new player tank");
    assert_eq!(player.vehicle, game_core::VehicleKind::Jagdtiger);
    assert_eq!(player.reload_remaining_s, 0.0);
}

#[test]
fn local_server_accepts_client_commands_and_emits_authoritative_snapshots() {
    let mut server = LocalAuthoritativeServer::new(ServerTickConfig::new(60, 20));
    let player_tank = server.player_tank();

    assert_eq!(server.authoritative_tick(), 0);
    assert_eq!(server.latest_snapshot().server_tick, 0);

    let first = server.tick_with_input(ClientInputCommand {
        client_tick: 0,
        tank_id: player_tank,
        command: TankCommand::drive(1.0, 0.0),
    });
    assert_eq!(first.server_tick, 1);
    assert!(first.snapshot.is_none());

    server.tick_with_input(ClientInputCommand {
        client_tick: 1,
        tank_id: player_tank,
        command: TankCommand::drive(1.0, 0.0),
    });
    let third = server.tick_with_input(ClientInputCommand {
        client_tick: 2,
        tank_id: player_tank,
        command: TankCommand::drive(1.0, 0.0),
    });

    let snapshot = third.snapshot.expect("20 Hz snapshots at 60 Hz server tick emit every 3 ticks");
    assert_eq!(third.server_tick, 3);
    assert_eq!(snapshot.server_tick, 3);
    assert_eq!(snapshot.tanks[0].tank_id, player_tank);
}

#[test]
fn local_server_replication_carries_damage_events_to_next_snapshot() {
    let mut server = LocalAuthoritativeServer::new(ServerTickConfig::new(60, 20));
    let player_tank = server.player_tank();
    let target_tank = server.target_tank();

    server.tick_with_input(ClientInputCommand {
        client_tick: 0,
        tank_id: player_tank,
        command: TankCommand { fire: true, ..TankCommand::idle() },
    });

    let mut damage_snapshot = None;
    for client_tick in 1..=8 {
        let tick = server.tick_with_input(ClientInputCommand {
            client_tick,
            tank_id: player_tank,
            command: TankCommand::idle(),
        });
        if let Some(snapshot) = tick.snapshot
            && !snapshot.damage_events.is_empty()
        {
            damage_snapshot = Some(snapshot);
            break;
        }
    }

    let snapshot = damage_snapshot.expect("damage event should be replicated on a snapshot");
    let event = &snapshot.damage_events[0];
    assert_eq!(event.source, player_tank);
    assert_eq!(event.target, target_tank);
    assert!(event.penetrated);
}

#[test]
fn local_server_replication_carries_absorbed_shell_impacts_to_next_snapshot() {
    let mut server = LocalAuthoritativeServer::new(ServerTickConfig::new(60, 20));
    let player_tank = server.player_tank();

    // Depress the gun fully, then fire into the dirt well short of the target tank.
    for client_tick in 0..30 {
        server.tick_with_input(ClientInputCommand {
            client_tick,
            tank_id: player_tank,
            command: TankCommand { gun_pitch_delta: -1.0, ..TankCommand::idle() },
        });
    }
    server.tick_with_input(ClientInputCommand {
        client_tick: 30,
        tank_id: player_tank,
        command: TankCommand { fire: true, ..TankCommand::idle() },
    });

    let mut impact_snapshot = None;
    for client_tick in 31..=120 {
        let tick = server.tick_with_input(ClientInputCommand {
            client_tick,
            tank_id: player_tank,
            command: TankCommand::idle(),
        });
        if let Some(snapshot) = tick.snapshot
            && !snapshot.shell_impacts.is_empty()
        {
            impact_snapshot = Some(snapshot);
            break;
        }
    }

    // Snapshot cadence must not drop the impact: it is buffered to the next emitted snapshot,
    // so the firing client always learns where its shot died.
    let snapshot = impact_snapshot.expect("an absorbed shell must reach the client in a snapshot");
    let impact = &snapshot.shell_impacts[0];
    assert_eq!(impact.owner, player_tank);
    assert_eq!(impact.surface, game_core::ImpactSurface::Terrain);
}
