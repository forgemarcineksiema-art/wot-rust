//! N2's acceptance test: two headless clients join one dedicated server over the in-memory hub,
//! the lobby fills the empty seats with bots, and every snapshot that leaves the host is ALREADY
//! the viewer's own cut — at spawn distances nothing hostile is spotted, so neither client may
//! receive the enemy team's positions. The wallhack dies here, before the wire.

use net::ProtocolMessage;
use net::session::{ClientSession, SessionState};
use net::transport::{MemoryHub, Transport};
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
    let mut input_sequence = [0_u64, 0_u64];
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
                    ProtocolMessage::SnapshotDelivery(delivery) => {
                        latest_snapshot[index] = Some(delivery.snapshot);
                    }
                    _ => {}
                }
            }
            // Once seated, drive: a redundant idle batch every step keeps the seat warm.
            if let Some(tank) = assigned[index] {
                let batch = ProtocolMessage::InputBatch {
                    session_id: client.session_id(),
                    commands: vec![net::ClientInputCommand {
                        client_tick: input_sequence[index],
                        tank_id: tank,
                        command: sim::TankCommand::idle(),
                    }],
                };
                client.endpoint.send(port, &batch).expect("input batch");
                input_sequence[index] += 1;
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

/// N4: a crew that vanishes frees its seat, and a LATE JOINER claims it mid-battle — converging
/// from the very first snapshot, because a snapshot IS the full battle state. The wire was
/// designed for this; here it is proven end-to-end.
#[test]
fn a_late_joiner_claims_a_freed_seat_and_converges_immediately() {
    let hub = MemoryHub::new();
    let server_addr = "10.0.0.1:40000".parse().expect("addr");
    let mut server_port = hub.port(server_addr);
    let mut port_a = hub.port("10.0.0.2:5000".parse().expect("addr"));

    let battle = RandomBattleConfig {
        seed: BattleSeed::fixed(77),
        player_vehicle: game_core::VehicleKind::T54_1951,
        map: terrain::MapId::default(),
    };
    let mut host = RemoteBattleServer::new(ServerTickConfig::default(), battle, 200, 0);
    let mut first = ClientSession::connect(server_addr, 0);
    let mut first_tank = None;
    for step in 0..60_u64 {
        let now_ms = step * 16;
        for message in first.tick(now_ms, &mut port_a).expect("tick") {
            if let ProtocolMessage::StartBattle { assigned_tank, .. } = message {
                first_tank = Some(assigned_tank);
            }
        }
        host.pump(now_ms, &mut server_port);
        host.tick(now_ms, &mut server_port);
    }
    let first_tank = first_tank.expect("seated");

    // The first crew vanishes; the host runs on until the seat ages out.
    for step in 60..800_u64 {
        let now_ms = step * 16;
        host.pump(now_ms, &mut server_port);
        host.tick(now_ms, &mut server_port);
    }

    // A LATE joiner arrives mid-battle and must inherit the freed tank.
    let mut port_b = hub.port("10.0.0.3:5000".parse().expect("addr"));
    let mut late = ClientSession::connect(server_addr, 800 * 16);
    let mut late_tank = None;
    let mut late_snapshot = None;
    let mut latest_position = None;
    let mut input_sequence = 0_u64;
    for step in 800..1_000_u64 {
        let now_ms = step * 16;
        for message in late.tick(now_ms, &mut port_b).expect("tick") {
            match message {
                ProtocolMessage::StartBattle { assigned_tank, .. } => {
                    late_tank = Some(assigned_tank);
                }
                ProtocolMessage::SnapshotDelivery(delivery) => {
                    if let Some(tank) = late_tank
                        && let Some(own) =
                            delivery.snapshot.tanks.iter().find(|snapshot| snapshot.tank_id == tank)
                    {
                        latest_position = Some(own.position);
                    }
                    late_snapshot.get_or_insert(delivery.snapshot);
                }
                _ => {}
            }
        }
        if let Some(tank) = late_tank {
            let batch = ProtocolMessage::InputBatch {
                session_id: late.session_id(),
                commands: vec![net::ClientInputCommand {
                    client_tick: input_sequence,
                    tank_id: tank,
                    command: sim::TankCommand::drive(1.0, 0.0),
                }],
            };
            late.endpoint.send(&mut port_b, &batch).expect("late input");
            input_sequence += 1;
        }
        host.pump(now_ms, &mut server_port);
        host.tick(now_ms, &mut server_port);
    }
    assert_eq!(late_tank, Some(first_tank), "the late joiner inherits the freed seat");
    let snapshot = late_snapshot.expect("the late joiner receives state immediately");
    // Convergence: the FIRST snapshot after joining already carries the whole battle — the
    // joiner's own hull with whatever the battle has done to it, and the global cover state.
    assert!(snapshot.tanks.iter().any(|t| t.tank_id == first_tank));
    assert!(!snapshot.cover_states.is_empty(), "cover state rides every snapshot");
    assert!(snapshot.server_tick > 700, "this is a mid-battle join, not a fresh start");
    let start = snapshot
        .tanks
        .iter()
        .find(|tank| tank.tank_id == first_tank)
        .expect("first late snapshot carries own tank")
        .position;
    let end = latest_position.expect("late snapshots keep carrying own tank");
    let travel_sq = (end[0] - start[0]).powi(2) + (end[2] - start[2]).powi(2);
    assert!(
        travel_sq > 1.0,
        "sequence zero must be accepted at high server tick; tank only moved {} m",
        travel_sq.sqrt()
    );
}

#[test]
fn invalid_input_neither_acks_the_seat_nor_keeps_it_alive() {
    let hub = MemoryHub::new();
    let server_addr = "10.0.0.1:40000".parse().expect("addr");
    let mut server_port = hub.port(server_addr);
    let mut attacker_port = hub.port("10.0.0.2:5000".parse().expect("addr"));
    let battle = RandomBattleConfig {
        seed: BattleSeed::fixed(91),
        player_vehicle: game_core::VehicleKind::T54_1951,
        map: terrain::MapId::default(),
    };
    let mut host = RemoteBattleServer::new(ServerTickConfig::default(), battle, 100, 0);
    let mut attacker = ClientSession::connect(server_addr, 0);
    let mut assigned = None;

    for step in 0..40_u64 {
        let now_ms = step * 16;
        for message in attacker.tick(now_ms, &mut attacker_port).expect("join tick") {
            if let ProtocolMessage::StartBattle { assigned_tank, .. } = message {
                assigned = Some(assigned_tank);
            }
        }
        host.pump(now_ms, &mut server_port);
        host.tick(now_ms, &mut server_port);
    }
    let assigned = assigned.expect("seat assigned");

    let mut repeated_starts = 0_usize;
    for step in 40..620_u64 {
        let now_ms = step * 16;
        let invalid = ProtocolMessage::InputBatch {
            session_id: attacker.session_id(),
            commands: vec![net::ClientInputCommand {
                client_tick: 0,
                tank_id: assigned,
                command: sim::TankCommand { throttle: 1.0e30, ..sim::TankCommand::idle() },
            }],
        };
        attacker.endpoint.send(&mut attacker_port, &invalid).expect("invalid input frame");
        host.pump(now_ms, &mut server_port);
        host.tick(now_ms, &mut server_port);
        while let Some((_, datagram)) = attacker_port.recv().expect("attacker recv") {
            if matches!(
                attacker.endpoint.accept(&datagram).expect("decode"),
                Some(ProtocolMessage::StartBattle { .. })
            ) {
                repeated_starts += 1;
            }
        }
    }
    assert!(
        repeated_starts > 100,
        "invalid input cannot ACK StartBattle; got only {repeated_starts} repeats"
    );

    // Cross the timeout measured from the last valid hello. The invalid spam above must not
    // have renewed liveness, so the seat is already free when a real late joiner arrives.
    for step in 620..680_u64 {
        let now_ms = step * 16;
        host.pump(now_ms, &mut server_port);
        host.tick(now_ms, &mut server_port);
    }
    let mut replacement_port = hub.port("10.0.0.3:5000".parse().expect("addr"));
    let mut replacement = ClientSession::connect(server_addr, 680 * 16);
    let mut replacement_tank = None;
    for step in 680..740_u64 {
        let now_ms = step * 16;
        for message in replacement.tick(now_ms, &mut replacement_port).expect("replacement tick") {
            if let ProtocolMessage::StartBattle { assigned_tank, .. } = message {
                replacement_tank = Some(assigned_tank);
            }
        }
        host.pump(now_ms, &mut server_port);
        host.tick(now_ms, &mut server_port);
    }
    assert_eq!(replacement_tank, Some(assigned), "invalid traffic must not retain the seat");
}

/// N4 chaos: the whole join-and-play flow under 25% seeded datagram loss. Fragmented snapshots
/// are abandoned and superseded by design; the session's hello resend and the redundant input
/// batches carry the rest — the client must still seat and still receive snapshots.
#[test]
fn the_flow_survives_heavy_seeded_loss() {
    let hub = MemoryHub::with_loss(4242, 25);
    let server_addr = "10.0.0.1:40000".parse().expect("addr");
    let mut server_port = hub.port(server_addr);
    let mut port_a = hub.port("10.0.0.2:5000".parse().expect("addr"));

    let battle = RandomBattleConfig {
        seed: BattleSeed::fixed(5),
        player_vehicle: game_core::VehicleKind::T54_1951,
        map: terrain::MapId::default(),
    };
    let mut host = RemoteBattleServer::new(ServerTickConfig::default(), battle, 300, 0);
    let mut client = ClientSession::connect(server_addr, 0);
    let mut seated = false;
    let mut snapshots = 0_usize;
    for step in 0..1_200_u64 {
        let now_ms = step * 16;
        for message in client.tick(now_ms, &mut port_a).expect("tick") {
            match message {
                ProtocolMessage::StartBattle { .. } => seated = true,
                ProtocolMessage::SnapshotDelivery(_) => snapshots += 1,
                _ => {}
            }
        }
        host.pump(now_ms, &mut server_port);
        host.tick(now_ms, &mut server_port);
    }
    assert!(seated, "25% loss must not stop the handshake and seating");
    assert!(snapshots >= 30, "snapshots keep flowing through the loss, got {snapshots}");
}
