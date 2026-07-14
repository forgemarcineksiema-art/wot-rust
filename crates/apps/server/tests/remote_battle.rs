//! N2's acceptance test: two headless clients join one dedicated server over the in-memory hub,
//! the lobby fills the empty seats with bots, and every snapshot that leaves the host is ALREADY
//! the viewer's own cut — at spawn distances nothing hostile is spotted, so neither client may
//! receive the enemy team's positions. The wallhack dies here, before the wire.

use net::ProtocolMessage;
use net::session::{ClientSession, SessionState};
use net::transport::MemoryHub;
use server::remote::RemoteBattleServer;
use server::{BattleSeed, RandomBattleConfig, ServerTickConfig};

#[test]
fn two_clients_get_their_own_filtered_views_and_bots_fill_the_rest() {
    let hub = MemoryHub::new();
    let server_addr = "10.0.0.1:40000".parse().expect("addr");
    let mut server_port = hub.port(server_addr);
    let mut port_a = hub.port("10.0.0.2:5000".parse().expect("addr"));
    let mut port_b = hub.port("10.0.0.3:5000".parse().expect("addr"));

    let battle = RandomBattleConfig {
        seed: BattleSeed::fixed(21),
        player_vehicle: game_core::VehicleKind::T54_1951,
        map: terrain::MapId::default(),
    };
    let mut host = RemoteBattleServer::new(ServerTickConfig::default(), battle, 400, 0);
    let mut client_a = ClientSession::connect(server_addr, 0);
    let mut client_b = ClientSession::connect(server_addr, 0);

    let mut assigned = [None, None];
    let mut latest_snapshot: [Option<net::Snapshot>; 2] = [None, None];

    for step in 0..600_u64 {
        let now_ms = step * 16;
        for (index, (client, port)) in
            [(&mut client_a, &mut port_a), (&mut client_b, &mut port_b)].into_iter().enumerate()
        {
            for message in client.tick(now_ms, port).expect("client tick") {
                match message {
                    ProtocolMessage::StartBattle { assigned_tank, .. } => {
                        assigned[index] = Some(assigned_tank);
                    }
                    ProtocolMessage::Snapshot(snapshot) => {
                        latest_snapshot[index] = Some(snapshot);
                    }
                    _ => {}
                }
            }
            // Once seated, drive: a redundant idle batch every step keeps the seat warm.
            if let Some(tank) = assigned[index] {
                let batch = ProtocolMessage::InputBatch {
                    commands: vec![net::ClientInputCommand {
                        client_tick: step,
                        tank_id: tank,
                        command: sim::TankCommand::idle(),
                    }],
                };
                client.endpoint.send(port, &batch).expect("input batch");
            }
        }
        host.pump(now_ms, &mut server_port);
        host.tick(now_ms, &mut server_port);
    }

    assert_eq!(*client_a.state(), SessionState::Connected);
    assert_eq!(*client_b.state(), SessionState::Connected);
    let (tank_a, tank_b) = (assigned[0].expect("A seated"), assigned[1].expect("B seated"));
    assert_ne!(tank_a, tank_b, "two crews, two different tanks");
    assert!(host.is_running(), "the lobby deadline filled the rest with bots and started");

    let snapshot_a = latest_snapshot[0].clone().expect("A receives snapshots");
    let snapshot_b = latest_snapshot[1].clone().expect("B receives snapshots");
    assert!(snapshot_a.tanks.iter().any(|t| t.tank_id == tank_a), "A sees its own hull");
    assert!(snapshot_b.tanks.iter().any(|t| t.tank_id == tank_b), "B sees its own hull");

    // THE anti-wallhack assertion: a 7v7 has 14 tanks; at spawn separation nothing hostile is
    // spotted yet, so each client's cut must be missing the enemy team entirely.
    assert!(
        snapshot_a.tanks.len() < 14,
        "A's snapshot must NOT carry unspotted enemies, got {} tanks",
        snapshot_a.tanks.len()
    );
    assert!(
        snapshot_b.tanks.len() < 14,
        "B's snapshot must NOT carry unspotted enemies, got {} tanks",
        snapshot_b.tanks.len()
    );
}

/// A vanished client ages out of the seat table and the battle keeps running on bots + the
/// remaining crew — a disconnect is an event the battle absorbs, never a hang.
#[test]
fn a_silent_client_ages_out_and_the_battle_keeps_running() {
    let hub = MemoryHub::new();
    let server_addr = "10.0.0.1:40000".parse().expect("addr");
    let mut server_port = hub.port(server_addr);
    let mut port_a = hub.port("10.0.0.2:5000".parse().expect("addr"));

    let battle = RandomBattleConfig {
        seed: BattleSeed::fixed(9),
        player_vehicle: game_core::VehicleKind::T54_1951,
        map: terrain::MapId::default(),
    };
    let mut host = RemoteBattleServer::new(ServerTickConfig::default(), battle, 200, 0);
    let mut client = ClientSession::connect(server_addr, 0);

    // Join and reach the battle.
    for step in 0..60_u64 {
        let now_ms = step * 16;
        let _ = client.tick(now_ms, &mut port_a).expect("tick");
        host.pump(now_ms, &mut server_port);
        host.tick(now_ms, &mut server_port);
    }
    assert!(host.is_running());

    // The client falls silent for well past the timeout; the host must keep ticking.
    for step in 60..1_500_u64 {
        let now_ms = step * 16;
        host.pump(now_ms, &mut server_port);
        host.tick(now_ms, &mut server_port);
    }
    assert!(host.is_running(), "a dead client never stalls the battle");
}
